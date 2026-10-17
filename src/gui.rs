//! Modern Dark Desktop GUI for RustCleaner built with `eframe` / `egui`.
//! Features custom Fluent/Obsidian styling, KPI metric cards, visual storage breakdown bars,
//! custom sidebar accent indicators, and non-blocking multi-threaded background workers.

use crate::cleaner::{
    analyze_all, default_clean_rules, execute_clean, format_bytes, CleanRule, CleanSummary,
    RuleGroup,
};
use crate::registry::{
    backup_and_fix_registry, export_reg_backup, scan_registry, RegistryCategory, RegistryIssue,
};
use crate::startup_uninstall::{
    delete_startup_entry, launch_uninstaller, list_installed_apps, list_startup_entries,
    set_startup_enabled, InstalledApp, StartupEntry,
};
use crate::uac;
use crate::wiper::{list_drives, wipe_free_space, DriveInfo, WipeAlgorithm, WipeProgress};
use eframe::egui::{self, Color32, CornerRadius, FontId, Pos2, Rect, RichText, Sense, Stroke, Vec2};
use std::env;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;

// === MODERN OBSIDIAN COLOR PALETTE ===
const BG_CANVAS: Color32 = Color32::from_rgb(13, 17, 23);
const BG_SIDEBAR: Color32 = Color32::from_rgb(18, 23, 33);
const BG_HEADER: Color32 = Color32::from_rgb(15, 20, 29);
const BG_CARD: Color32 = Color32::from_rgb(23, 29, 41);
const BG_CARD_HOVER: Color32 = Color32::from_rgb(30, 38, 54);
const BG_CARD_SUBTLE: Color32 = Color32::from_rgb(19, 24, 34);
const BORDER_SUBTLE: Color32 = Color32::from_rgb(42, 52, 70);

const ACCENT_RUST: Color32 = Color32::from_rgb(245, 85, 50);
const ACCENT_CYAN: Color32 = Color32::from_rgb(56, 189, 248);
const ACCENT_EMERALD: Color32 = Color32::from_rgb(16, 185, 129);
const ACCENT_AMBER: Color32 = Color32::from_rgb(245, 158, 11);
const ACCENT_PURPLE: Color32 = Color32::from_rgb(168, 85, 247);
const ACCENT_RED: Color32 = Color32::from_rgb(239, 68, 68);

const TEXT_PRIMARY: Color32 = Color32::from_rgb(240, 246, 252);
const TEXT_SECONDARY: Color32 = Color32::from_rgb(148, 163, 184);
const TEXT_MUTED: Color32 = Color32::from_rgb(100, 116, 139);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActiveTab {
    Cleaner,
    Registry,
    Startup,
    Uninstaller,
    DriveWiper,
}

pub struct RustCleanerApp {
    pub theme_initialized: bool,
    pub active_tab: ActiveTab,
    pub is_admin: bool,
    pub status_banner: String,

    // 1. Cleaner State
    pub clean_rules: Vec<CleanRule>,
    pub clean_summary: Option<CleanSummary>,
    pub cleaner_busy: bool,
    pub cleaner_result_rx: Arc<Mutex<Option<(CleanSummary, bool)>>>,
    pub selected_rule_group_filter: Option<RuleGroup>,

    // 2. Registry State
    pub reg_categories: Vec<(RegistryCategory, bool)>,
    pub reg_issues: Vec<RegistryIssue>,
    pub reg_backup_dir: PathBuf,
    pub reg_last_backup: Option<PathBuf>,

    // 3. Startup & Uninstaller State
    pub startup_entries: Vec<StartupEntry>,
    pub startup_filter: String,
    pub installed_apps: Vec<InstalledApp>,
    pub uninstall_filter: String,

    // 4. Drive Wiper State
    pub drives: Vec<DriveInfo>,
    pub selected_drive_idx: usize,
    pub wipe_algorithm: WipeAlgorithm,
    pub wipe_mft_slack: bool,
    pub wipe_limit_mb_enabled: bool,
    pub wipe_limit_mb: u64,
    pub wipe_progress: Arc<Mutex<WipeProgress>>,
    pub wipe_cancel: Arc<AtomicBool>,
}

impl Default for RustCleanerApp {
    fn default() -> Self {
        let is_admin = uac::is_elevated();
        let backup_dir = env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join("registry_backups");
        let drives = list_drives();

        Self {
            theme_initialized: false,
            active_tab: ActiveTab::Cleaner,
            is_admin,
            status_banner: if is_admin {
                "Pronto — Esecuzione con privilegi di Amministratore (UAC attivo).".to_string()
            } else {
                "Pronto — Modalità Utente Standard. Clicca su 'Eleva UAC' in alto a destra per accesso completo al sistema.".to_string()
            },
            clean_rules: default_clean_rules(),
            clean_summary: None,
            cleaner_busy: false,
            cleaner_result_rx: Arc::new(Mutex::new(None)),
            selected_rule_group_filter: None,
            reg_categories: RegistryCategory::all()
                .iter()
                .map(|&c| (c, true))
                .collect(),
            reg_issues: Vec::new(),
            reg_backup_dir: backup_dir,
            reg_last_backup: None,
            startup_entries: list_startup_entries(),
            startup_filter: String::new(),
            installed_apps: list_installed_apps(),
            uninstall_filter: String::new(),
            drives,
            selected_drive_idx: 0,
            wipe_algorithm: WipeAlgorithm::SimpleOnePassZero,
            wipe_mft_slack: true,
            wipe_limit_mb_enabled: true,
            wipe_limit_mb: 64,
            wipe_progress: Arc::new(Mutex::new(WipeProgress::default())),
            wipe_cancel: Arc::new(AtomicBool::new(false)),
        }
    }
}

fn configure_modern_theme(ctx: &egui::Context) {
    let mut style = (*ctx.style()).clone();
    let mut visuals = egui::Visuals::dark();

    visuals.panel_fill = BG_CANVAS;
    visuals.window_fill = BG_CARD;
    visuals.extreme_bg_color = Color32::from_rgb(10, 13, 18);
    visuals.faint_bg_color = BG_CARD_SUBTLE;
    visuals.override_text_color = Some(TEXT_PRIMARY);

    let radius = CornerRadius::same(8);
    visuals.window_corner_radius = CornerRadius::same(12);
    visuals.menu_corner_radius = CornerRadius::same(8);

    visuals.widgets.noninteractive.bg_fill = BG_CARD;
    visuals.widgets.noninteractive.weak_bg_fill = BG_CARD;
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, BORDER_SUBTLE);
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, TEXT_PRIMARY);
    visuals.widgets.noninteractive.corner_radius = radius;

    visuals.widgets.inactive.bg_fill = Color32::from_rgb(30, 38, 54);
    visuals.widgets.inactive.weak_bg_fill = Color32::from_rgb(26, 33, 47);
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, Color32::from_rgb(52, 65, 88));
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, TEXT_PRIMARY);
    visuals.widgets.inactive.corner_radius = radius;

    visuals.widgets.hovered.bg_fill = Color32::from_rgb(42, 53, 75);
    visuals.widgets.hovered.weak_bg_fill = Color32::from_rgb(38, 48, 68);
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, ACCENT_RUST);
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.5, Color32::WHITE);
    visuals.widgets.hovered.corner_radius = radius;

    visuals.widgets.active.bg_fill = ACCENT_RUST;
    visuals.widgets.active.weak_bg_fill = ACCENT_RUST;
    visuals.widgets.active.bg_stroke = Stroke::new(1.0, Color32::WHITE);
    visuals.widgets.active.fg_stroke = Stroke::new(1.5, Color32::WHITE);
    visuals.widgets.active.corner_radius = radius;

    visuals.selection.bg_fill = Color32::from_rgba_unmultiplied(245, 85, 50, 75);
    visuals.selection.stroke = Stroke::new(1.5, ACCENT_RUST);

    style.visuals = visuals;
    style.spacing.item_spacing = egui::vec2(10.0, 8.0);
    style.spacing.button_padding = egui::vec2(14.0, 8.0);
    style.spacing.window_margin = egui::Margin::same(14);
    ctx.set_style(style);
}

/// Renders a sleek dashboard KPI card with a top colored accent bar, title, big metric value, and subtitle.
fn draw_kpi_card(
    ui: &mut egui::Ui,
    title: &str,
    value: &str,
    subtitle: &str,
    accent: Color32,
) {
    egui::Frame::new()
        .fill(BG_CARD)
        .stroke(Stroke::new(1.0, BORDER_SUBTLE))
        .corner_radius(CornerRadius::same(10))
        .inner_margin(egui::Margin::symmetric(14, 12))
        .show(ui, |ui| {
            ui.set_min_width(170.0);
            // Colored top indicator pill
            let (rect, _) = ui.allocate_exact_size(egui::vec2(36.0, 4.0), Sense::hover());
            ui.painter()
                .rect_filled(rect, CornerRadius::same(2), accent);
            ui.add_space(4.0);
            ui.label(
                RichText::new(title)
                    .size(12.0)
                    .color(TEXT_SECONDARY)
                    .strong(),
            );
            ui.add_space(2.0);
            ui.label(
                RichText::new(value)
                    .size(21.0)
                    .strong()
                    .color(TEXT_PRIMARY),
            );
            ui.add_space(2.0);
            ui.label(RichText::new(subtitle).size(11.0).color(TEXT_MUTED));
        });
}

/// Renders a rounded pill badge with custom background and text color.
fn draw_pill_badge(ui: &mut egui::Ui, text: &str, bg: Color32, fg: Color32) {
    egui::Frame::new()
        .fill(bg)
        .corner_radius(CornerRadius::same(20))
        .inner_margin(egui::Margin::symmetric(10, 4))
        .show(ui, |ui| {
            ui.label(RichText::new(text).size(11.5).strong().color(fg));
        });
}

/// Container card with subtle border and rounded corners.
fn card_frame() -> egui::Frame {
    egui::Frame::new()
        .fill(BG_CARD)
        .stroke(Stroke::new(1.0, BORDER_SUBTLE))
        .corner_radius(CornerRadius::same(10))
        .inner_margin(egui::Margin::symmetric(14, 12))
}

impl eframe::App for RustCleanerApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if !self.theme_initialized {
            configure_modern_theme(ctx);
            self.theme_initialized = true;
        }

        // Check background cleaner thread completion
        let mut completed_task = None;
        if let Ok(mut guard) = self.cleaner_result_rx.lock() {
            if let Some(res) = guard.take() {
                completed_task = Some(res);
            }
        }
        if let Some((summary, was_clean)) = completed_task {
            self.cleaner_busy = false;
            if was_clean {
                self.status_banner = format!(
                    "✅ Pulizia completata: {} file eliminati ({}) — {} file in uso saltati in sicurezza.",
                    summary.total_files_deleted,
                    format_bytes(summary.total_bytes_deleted),
                    summary.total_skipped_locked
                );
            } else {
                self.status_banner = format!(
                    "🔍 Analisi completata: {} file rilevati ({}) — {} file attualmente in uso/bloccati.",
                    summary.total_files_found,
                    format_bytes(summary.total_bytes_found),
                    summary.total_skipped_locked
                );
            }
            self.clean_summary = Some(summary);
        }

        let wiper_running = self
            .wipe_progress
            .lock()
            .map(|p| p.running)
            .unwrap_or(false);
        if self.cleaner_busy || wiper_running {
            ctx.request_repaint();
        }

        // === TOP HEADER BAR ===
        egui::TopBottomPanel::top("top_header")
            .frame(
                egui::Frame::new()
                    .fill(BG_HEADER)
                    .stroke(Stroke::new(1.0, BORDER_SUBTLE))
                    .inner_margin(egui::Margin::symmetric(18, 12)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    // Brand Logo Pill
                    egui::Frame::new()
                        .fill(ACCENT_RUST)
                        .corner_radius(CornerRadius::same(8))
                        .inner_margin(egui::Margin::symmetric(9, 6))
                        .show(ui, |ui| {
                            ui.label(
                                RichText::new("🦀")
                                    .size(18.0)
                                    .strong()
                                    .color(Color32::WHITE),
                            );
                        });

                    ui.add_space(4.0);
                    ui.vertical(|ui| {
                        ui.spacing_mut().item_spacing.y = 1.0;
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new("RustCleaner")
                                    .size(20.0)
                                    .strong()
                                    .color(TEXT_PRIMARY),
                            );
                            draw_pill_badge(
                                ui,
                                "MEGA ALPHA v0.1.0",
                                Color32::from_rgba_unmultiplied(245, 85, 50, 45),
                                Color32::from_rgb(255, 140, 110),
                            );
                        });
                        ui.label(
                            RichText::new("Windows System Cleaner, Registry Guard & Disk Wiper")
                                .size(11.5)
                                .color(TEXT_SECONDARY),
                        );
                    });

                    // Right side: UAC Status + Primary Drive Gauge
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if self.is_admin {
                            draw_pill_badge(
                                ui,
                                "🛡️ UAC: AMMINISTRATORE",
                                Color32::from_rgba_unmultiplied(16, 185, 129, 35),
                                ACCENT_EMERALD,
                            );
                        } else {
                            let uac_btn = egui::Button::new(
                                RichText::new("⚡ Eleva come Amministratore (UAC)")
                                    .size(12.5)
                                    .strong()
                                    .color(Color32::from_rgb(20, 20, 20)),
                            )
                            .fill(ACCENT_AMBER)
                            .corner_radius(CornerRadius::same(20));

                            if ui.add(uac_btn).clicked() {
                                match uac::relaunch_as_admin(&[]) {
                                    Ok(_) => std::process::exit(0),
                                    Err(e) => self.status_banner = e,
                                }
                            }
                            draw_pill_badge(
                                ui,
                                "⚠️ Utente Standard",
                                Color32::from_rgba_unmultiplied(245, 158, 11, 30),
                                ACCENT_AMBER,
                            );
                        }

                        ui.add_space(8.0);
                        if let Some(d) = self.drives.first() {
                            let used_ratio = if d.total_bytes > 0 {
                                d.used_bytes as f32 / d.total_bytes as f32
                            } else {
                                0.0
                            };
                            egui::Frame::new()
                                .fill(BG_CARD)
                                .stroke(Stroke::new(1.0, BORDER_SUBTLE))
                                .corner_radius(CornerRadius::same(8))
                                .inner_margin(egui::Margin::symmetric(10, 5))
                                .show(ui, |ui| {
                                    ui.horizontal(|ui| {
                                        ui.label(
                                            RichText::new(format!(
                                                "💽 {} Libero: {}",
                                                d.root_path,
                                                format_bytes(d.free_bytes)
                                            ))
                                            .size(11.5)
                                            .color(TEXT_SECONDARY),
                                        );
                                        let (bar_rect, _) = ui.allocate_exact_size(
                                            egui::vec2(70.0, 7.0),
                                            Sense::hover(),
                                        );
                                        ui.painter().rect_filled(
                                            bar_rect,
                                            CornerRadius::same(4),
                                            Color32::from_rgb(35, 43, 58),
                                        );
                                        let mut fill_rect = bar_rect;
                                        fill_rect.set_width(bar_rect.width() * used_ratio.clamp(0.0, 1.0));
                                        let bar_color = if used_ratio > 0.85 {
                                            ACCENT_RED
                                        } else {
                                            ACCENT_CYAN
                                        };
                                        ui.painter().rect_filled(
                                            fill_rect,
                                            CornerRadius::same(4),
                                            bar_color,
                                        );
                                    });
                                });
                        }
                    });
                });
            });

        // === BOTTOM STATUS BAR ===
        egui::TopBottomPanel::bottom("bottom_status")
            .frame(
                egui::Frame::new()
                    .fill(BG_HEADER)
                    .stroke(Stroke::new(1.0, BORDER_SUBTLE))
                    .inner_margin(egui::Margin::symmetric(18, 8)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    let dot_color = if self.cleaner_busy || wiper_running {
                        ACCENT_AMBER
                    } else {
                        ACCENT_EMERALD
                    };
                    let (dot_rect, _) =
                        ui.allocate_exact_size(egui::vec2(8.0, 8.0), Sense::hover());
                    ui.painter()
                        .circle_filled(dot_rect.center(), 4.0, dot_color);
                    ui.label(
                        RichText::new(&self.status_banner)
                            .size(12.5)
                            .color(TEXT_SECONDARY),
                    );
                });
            });

        // === LEFT NAVIGATION SIDEBAR ===
        egui::SidePanel::left("left_sidebar")
            .resizable(false)
            .exact_width(245.0)
            .frame(
                egui::Frame::new()
                    .fill(BG_SIDEBAR)
                    .stroke(Stroke::new(1.0, BORDER_SUBTLE))
                    .inner_margin(egui::Margin::symmetric(12, 16)),
            )
            .show(ctx, |ui| {
                ui.label(
                    RichText::new("STRUMENTI DI SISTEMA")
                        .size(11.0)
                        .strong()
                        .color(TEXT_MUTED),
                );
                ui.add_space(6.0);

                let nav_items = [
                    (
                        ActiveTab::Cleaner,
                        "🧹",
                        "Pulizia Rapida",
                        "File temp, cache e browser",
                    ),
                    (
                        ActiveTab::Registry,
                        "🧩",
                        "Registro Windows",
                        "Ripara chiavi + Backup .reg",
                    ),
                    (
                        ActiveTab::Startup,
                        "🚀",
                        "Avvio Automatico",
                        "Gestisci app all'avvio",
                    ),
                    (
                        ActiveTab::Uninstaller,
                        "📦",
                        "Disinstallazione",
                        "Rimuovi software installati",
                    ),
                    (
                        ActiveTab::DriveWiper,
                        "💿",
                        "Bonifica Disco",
                        "Wipe spazio libero & MFT",
                    ),
                ];

                for (tab, icon, title, subtitle) in nav_items {
                    let is_selected = self.active_tab == tab;
                    let desired_size = egui::vec2(220.0, 52.0);
                    let (rect, resp) = ui.allocate_exact_size(desired_size, Sense::click());

                    if resp.clicked() {
                        self.active_tab = tab;
                    }

                    let bg = if is_selected {
                        Color32::from_rgb(32, 41, 58)
                    } else if resp.hovered() {
                        BG_CARD_HOVER
                    } else {
                        Color32::TRANSPARENT
                    };

                    ui.painter()
                        .rect_filled(rect, CornerRadius::same(10), bg);

                    if is_selected {
                        ui.painter().rect_stroke(
                            rect,
                            CornerRadius::same(10),
                            Stroke::new(1.0, Color32::from_rgb(65, 80, 110)),
                            egui::StrokeKind::Inside,
                        );
                        // Glowing left accent bar
                        let bar_rect = Rect::from_min_size(
                            Pos2::new(rect.left() + 3.0, rect.top() + 10.0),
                            Vec2::new(4.0, rect.height() - 20.0),
                        );
                        ui.painter()
                            .rect_filled(bar_rect, CornerRadius::same(2), ACCENT_RUST);
                    }

                    // Icon
                    ui.painter().text(
                        Pos2::new(rect.left() + 18.0, rect.center().y),
                        egui::Align2::LEFT_CENTER,
                        icon,
                        FontId::proportional(19.0),
                        TEXT_PRIMARY,
                    );

                    // Title & Subtitle
                    ui.painter().text(
                        Pos2::new(rect.left() + 48.0, rect.top() + 17.0),
                        egui::Align2::LEFT_CENTER,
                        title,
                        FontId::proportional(14.5),
                        if is_selected {
                            Color32::WHITE
                        } else {
                            TEXT_PRIMARY
                        },
                    );
                    ui.painter().text(
                        Pos2::new(rect.left() + 48.0, rect.top() + 35.0),
                        egui::Align2::LEFT_CENTER,
                        subtitle,
                        FontId::proportional(11.0),
                        if is_selected {
                            ACCENT_CYAN
                        } else {
                            TEXT_MUTED
                        },
                    );

                    ui.add_space(4.0);
                }

                // Bottom Sidebar Summary Card
                ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                    egui::Frame::new()
                        .fill(BG_CARD)
                        .stroke(Stroke::new(1.0, BORDER_SUBTLE))
                        .corner_radius(CornerRadius::same(10))
                        .inner_margin(egui::Margin::symmetric(12, 10))
                        .show(ui, |ui| {
                            ui.set_width(195.0);
                            let enabled_rules =
                                self.clean_rules.iter().filter(|r| r.enabled).count();
                            ui.label(
                                RichText::new(format!(
                                    "✔ {} / {} Regole Attive",
                                    enabled_rules,
                                    self.clean_rules.len()
                                ))
                                .size(12.0)
                                .color(ACCENT_EMERALD)
                                .strong(),
                            );
                            ui.label(
                                RichText::new(format!(
                                    "🚀 {} App in Avvio • 📦 {} Installate",
                                    self.startup_entries.len(),
                                    self.installed_apps.len()
                                ))
                                .size(11.0)
                                .color(TEXT_SECONDARY),
                            );
                            ui.label(
                                RichText::new("🛡️ Backup .reg Automatico Attivo")
                                    .size(11.0)
                                    .color(ACCENT_CYAN),
                            );
                        });
                });
            });

        // === CENTRAL MAIN PANEL ===
        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(BG_CANVAS)
                    .inner_margin(egui::Margin::symmetric(20, 16)),
            )
            .show(ctx, |ui| match self.active_tab {
                ActiveTab::Cleaner => self.render_cleaner_tab(ui),
                ActiveTab::Registry => self.render_registry_tab(ui),
                ActiveTab::Startup => self.render_startup_tab(ui),
                ActiveTab::Uninstaller => self.render_uninstaller_tab(ui),
                ActiveTab::DriveWiper => self.render_wiper_tab(ui),
            });
    }
}

impl RustCleanerApp {
    fn render_cleaner_tab(&mut self, ui: &mut egui::Ui) {
        // Section Header + Action Buttons Row
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.label(
                    RichText::new("🧹 Pulizia del File System & Privacy Browser")
                        .size(20.0)
                        .strong()
                        .color(TEXT_PRIMARY),
                );
                ui.label(
                    RichText::new(
                        "Motore multi-thread (rayon + walkdir) con rilevamento automatico dei file bloccati o in uso.",
                    )
                    .size(12.5)
                    .color(TEXT_SECONDARY),
                );
            });

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let clean_btn = egui::Button::new(
                    RichText::new("🧹 Avvia Pulizia")
                        .size(14.5)
                        .strong()
                        .color(Color32::WHITE),
                )
                .fill(ACCENT_RUST)
                .min_size(egui::vec2(150.0, 38.0));

                if ui.add_enabled(!self.cleaner_busy, clean_btn).clicked() {
                    self.start_cleaner_job(true);
                }

                let analyze_btn = egui::Button::new(
                    RichText::new("🔍 Analizza Sistema")
                        .size(14.5)
                        .strong()
                        .color(ACCENT_CYAN),
                )
                .fill(Color32::from_rgb(24, 38, 56))
                .stroke(Stroke::new(1.0, ACCENT_CYAN))
                .min_size(egui::vec2(155.0, 38.0));

                if ui.add_enabled(!self.cleaner_busy, analyze_btn).clicked() {
                    self.start_cleaner_job(false);
                }
            });
        });

        ui.add_space(10.0);

        // === 4 KPI DASHBOARD CARDS ===
        let (found_bytes, found_files, locked_files, deleted_bytes, deleted_files) =
            if let Some(s) = &self.clean_summary {
                (
                    format_bytes(s.total_bytes_found),
                    format!("{} file", s.total_files_found),
                    format!("{} file", s.total_skipped_locked),
                    format_bytes(s.total_bytes_deleted),
                    format!("{} file rimossi", s.total_files_deleted),
                )
            } else {
                (
                    "—".to_string(),
                    "In attesa di analisi".to_string(),
                    "—".to_string(),
                    "0 B".to_string(),
                    "Nessuna pulizia eseguita".to_string(),
                )
            };

        ui.columns(4, |cols| {
            draw_kpi_card(
                &mut cols[0],
                "SPAZIO RILEVATO",
                &found_bytes,
                &found_files,
                ACCENT_CYAN,
            );
            draw_kpi_card(
                &mut cols[1],
                "FILE IN USO (PROTETTI)",
                &locked_files,
                "Saltati senza errori",
                ACCENT_AMBER,
            );
            draw_kpi_card(
                &mut cols[2],
                "SPAZIO LIBERATO",
                &deleted_bytes,
                &deleted_files,
                ACCENT_EMERALD,
            );
            let enabled_count = self.clean_rules.iter().filter(|r| r.enabled).count();
            draw_kpi_card(
                &mut cols[3],
                "REGOLE ATTIVE",
                &format!("{} / {}", enabled_count, self.clean_rules.len()),
                "Sistema, Explorer e Browser",
                ACCENT_PURPLE,
            );
        });

        ui.add_space(10.0);

        // === VISUAL STORAGE BREAKDOWN BAR (if analysis exists) ===
        if let Some(summary) = &self.clean_summary {
            if summary.total_bytes_found > 0 {
                card_frame().show(ui, |ui| {
                    let groups = [
                        (RuleGroup::WindowsSystem, "Sistema Windows", ACCENT_RUST),
                        (RuleGroup::WindowsExplorer, "Esplora File", ACCENT_AMBER),
                        (RuleGroup::BrowsersPrivacy, "Browser & Privacy", ACCENT_CYAN),
                        (RuleGroup::Applications, "Applicazioni", ACCENT_PURPLE),
                    ];

                    let (bar_rect, _) = ui.allocate_exact_size(
                        egui::vec2(ui.available_width(), 10.0),
                        Sense::hover(),
                    );
                    ui.painter().rect_filled(
                        bar_rect,
                        CornerRadius::same(5),
                        Color32::from_rgb(32, 40, 56),
                    );

                    let mut x_offset = bar_rect.left();
                    for (grp, _, color) in groups {
                        let grp_bytes: u64 = summary
                            .results
                            .iter()
                            .filter(|r| r.group == grp)
                            .map(|r| r.total_bytes)
                            .sum();
                        if grp_bytes > 0 {
                            let w = (grp_bytes as f32 / summary.total_bytes_found as f32)
                                * bar_rect.width();
                            let seg_rect = Rect::from_min_size(
                                Pos2::new(x_offset, bar_rect.top()),
                                Vec2::new(w.max(3.0), bar_rect.height()),
                            );
                            ui.painter()
                                .rect_filled(seg_rect, CornerRadius::same(3), color);
                            x_offset += w;
                        }
                    }

                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        for (grp, label, color) in groups {
                            let grp_bytes: u64 = summary
                                .results
                                .iter()
                                .filter(|r| r.group == grp)
                                .map(|r| r.total_bytes)
                                .sum();
                            let (dot, _) =
                                ui.allocate_exact_size(egui::vec2(8.0, 8.0), Sense::hover());
                            ui.painter().circle_filled(dot.center(), 4.0, color);
                            ui.label(
                                RichText::new(format!("{label}: {}", format_bytes(grp_bytes)))
                                    .size(11.5)
                                    .color(TEXT_SECONDARY),
                            );
                            ui.add_space(10.0);
                        }
                    });
                });
                ui.add_space(10.0);
            }
        }

        // === TWO-COLUMN WORKSPACE ===
        ui.columns(2, |cols| {
            // LEFT COLUMN: Category Filter Pills + Rule Cards
            card_frame().show(&mut cols[0], |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("Categorie di Pulizia")
                            .size(15.0)
                            .strong()
                            .color(TEXT_PRIMARY),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.small_button("Deseleziona tutto").clicked() {
                            for r in &mut self.clean_rules {
                                r.enabled = false;
                            }
                        }
                        if ui.small_button("Seleziona tutto").clicked() {
                            for r in &mut self.clean_rules {
                                r.enabled = true;
                            }
                        }
                    });
                });

                ui.add_space(4.0);
                ui.horizontal_wrapped(|ui| {
                    let filters = [
                        (None, "Tutte"),
                        (Some(RuleGroup::WindowsSystem), "🖥 Sistema"),
                        (Some(RuleGroup::WindowsExplorer), "📁 Explorer"),
                        (Some(RuleGroup::BrowsersPrivacy), "🌐 Browser"),
                        (Some(RuleGroup::Applications), "🧰 App"),
                    ];
                    for (flt, lbl) in filters {
                        let active = self.selected_rule_group_filter == flt;
                        let btn = egui::Button::new(
                            RichText::new(lbl).size(12.0).color(if active {
                                Color32::WHITE
                            } else {
                                TEXT_SECONDARY
                            }),
                        )
                        .fill(if active {
                            ACCENT_RUST
                        } else {
                            BG_CARD_SUBTLE
                        })
                        .corner_radius(CornerRadius::same(16));
                        if ui.add(btn).clicked() {
                            self.selected_rule_group_filter = flt;
                        }
                    }
                });

                ui.separator();

                egui::ScrollArea::vertical()
                    .id_salt("cleaner_rules_scroll")
                    .max_height(330.0)
                    .show(ui, |ui| {
                        for rule in &mut self.clean_rules {
                            if let Some(flt) = self.selected_rule_group_filter {
                                if rule.group != flt {
                                    continue;
                                }
                            }
                            egui::Frame::new()
                                .fill(BG_CARD_SUBTLE)
                                .stroke(Stroke::new(1.0, BORDER_SUBTLE))
                                .corner_radius(CornerRadius::same(8))
                                .inner_margin(egui::Margin::symmetric(10, 8))
                                .show(ui, |ui| {
                                    ui.horizontal(|ui| {
                                        ui.checkbox(&mut rule.enabled, "");
                                        ui.label(
                                            RichText::new(&rule.name)
                                                .size(13.5)
                                                .strong()
                                                .color(TEXT_PRIMARY),
                                        );
                                        if rule.requires_admin {
                                            draw_pill_badge(
                                                ui,
                                                "UAC",
                                                Color32::from_rgba_unmultiplied(245, 158, 11, 35),
                                                ACCENT_AMBER,
                                            );
                                        }
                                    });
                                    ui.label(
                                        RichText::new(&rule.description)
                                            .size(11.5)
                                            .color(TEXT_MUTED),
                                    );
                                });
                            ui.add_space(4.0);
                        }
                    });
            });

            // RIGHT COLUMN: Detailed Scan Results
            card_frame().show(&mut cols[1], |ui| {
                ui.label(
                    RichText::new("Dettaglio Analisi e File Rilevati")
                        .size(15.0)
                        .strong()
                        .color(TEXT_PRIMARY),
                );
                ui.separator();

                if self.cleaner_busy {
                    ui.vertical_centered(|ui| {
                        ui.add_space(80.0);
                        ui.spinner();
                        ui.add_space(10.0);
                        ui.label(
                            RichText::new("Scansione parallela multi-thread in corso...")
                                .size(14.0)
                                .color(ACCENT_CYAN),
                        );
                        ui.add_space(80.0);
                    });
                } else if let Some(summary) = &self.clean_summary {
                    egui::ScrollArea::vertical()
                        .id_salt("cleaner_results_scroll")
                        .max_height(365.0)
                        .show(ui, |ui| {
                            let non_empty: Vec<_> = summary
                                .results
                                .iter()
                                .filter(|r| !r.files.is_empty())
                                .collect();

                            if non_empty.is_empty() {
                                ui.label(
                                    RichText::new("✨ Nessun file spazzatura rilevato nelle categorie selezionate.")
                                        .color(ACCENT_EMERALD),
                                );
                            }

                            for res in non_empty {
                                let ratio = if summary.total_bytes_found > 0 {
                                    (res.total_bytes as f32 / summary.total_bytes_found as f32)
                                        .clamp(0.0, 1.0)
                                } else {
                                    0.0
                                };

                                egui::Frame::new()
                                    .fill(BG_CARD_SUBTLE)
                                    .stroke(Stroke::new(1.0, BORDER_SUBTLE))
                                    .corner_radius(CornerRadius::same(8))
                                    .inner_margin(egui::Margin::symmetric(10, 8))
                                    .show(ui, |ui| {
                                        ui.horizontal(|ui| {
                                            ui.label(
                                                RichText::new(&res.rule_name)
                                                    .size(13.5)
                                                    .strong()
                                                    .color(TEXT_PRIMARY),
                                            );
                                            ui.with_layout(
                                                egui::Layout::right_to_left(egui::Align::Center),
                                                |ui| {
                                                    draw_pill_badge(
                                                        ui,
                                                        &format_bytes(res.total_bytes),
                                                        Color32::from_rgba_unmultiplied(
                                                            56, 189, 248, 35,
                                                        ),
                                                        ACCENT_CYAN,
                                                    );
                                                    ui.label(
                                                        RichText::new(format!(
                                                            "{} file ({} in uso)",
                                                            res.files.len(),
                                                            res.locked_count
                                                        ))
                                                        .size(11.5)
                                                        .color(TEXT_SECONDARY),
                                                    );
                                                },
                                            );
                                        });

                                        // Mini progress bar
                                        let (p_rect, _) = ui.allocate_exact_size(
                                            egui::vec2(ui.available_width(), 4.0),
                                            Sense::hover(),
                                        );
                                        ui.painter().rect_filled(
                                            p_rect,
                                            CornerRadius::same(2),
                                            Color32::from_rgb(32, 40, 56),
                                        );
                                        let mut fill_r = p_rect;
                                        fill_r.set_width(p_rect.width() * ratio);
                                        ui.painter().rect_filled(
                                            fill_r,
                                            CornerRadius::same(2),
                                            ACCENT_CYAN,
                                        );

                                        ui.collapsing(
                                            RichText::new("Mostra elenco file")
                                                .size(11.5)
                                                .color(TEXT_MUTED),
                                            |ui| {
                                                for f in res.files.iter().take(50) {
                                                    ui.horizontal(|ui| {
                                                        if f.locked_or_denied {
                                                            ui.label(
                                                                RichText::new("🔒 IN USO")
                                                                    .size(10.5)
                                                                    .color(ACCENT_AMBER),
                                                            );
                                                        } else {
                                                            ui.label(
                                                                RichText::new("✔")
                                                                    .size(11.0)
                                                                    .color(ACCENT_EMERALD),
                                                            );
                                                        }
                                                        ui.label(
                                                            RichText::new(format!(
                                                                "{} ({})",
                                                                f.path.display(),
                                                                format_bytes(f.size_bytes)
                                                            ))
                                                            .size(11.0)
                                                            .color(TEXT_SECONDARY),
                                                        );
                                                    });
                                                }
                                            },
                                        );
                                    });
                                ui.add_space(4.0);
                            }
                        });
                } else {
                    ui.vertical_centered(|ui| {
                        ui.add_space(70.0);
                        ui.label(RichText::new("🔍").size(38.0));
                        ui.add_space(6.0);
                        ui.label(
                            RichText::new("Pronto per l'Analisi del Sistema")
                                .size(15.5)
                                .strong()
                                .color(TEXT_PRIMARY),
                        );
                        ui.label(
                            RichText::new(
                                "Clicca su 'Analizza Sistema' in alto a destra per calcolare lo spazio recuperabile\nsenza eliminare alcun file.",
                            )
                            .size(12.5)
                            .color(TEXT_MUTED),
                        );
                        ui.add_space(70.0);
                    });
                }
            });
        });
    }

    fn start_cleaner_job(&mut self, delete_files: bool) {
        self.cleaner_busy = true;
        self.status_banner = if delete_files {
            "🧹 Pulizia parallela del File System in corso...".to_string()
        } else {
            "🔍 Analisi parallela del File System in corso...".to_string()
        };
        let rules_clone = self.clean_rules.clone();
        let rx_slot = Arc::clone(&self.cleaner_result_rx);
        thread::spawn(move || {
            let summary = if delete_files {
                execute_clean(&rules_clone)
            } else {
                analyze_all(&rules_clone)
            };
            if let Ok(mut guard) = rx_slot.lock() {
                *guard = Some((summary, delete_files));
            }
        });
    }

    fn render_registry_tab(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.label(
                    RichText::new("🧩 Manutenzione del Registro di Windows")
                        .size(20.0)
                        .strong()
                        .color(TEXT_PRIMARY),
                );
                ui.label(
                    RichText::new(
                        "Ricerca chiavi orfane con esportazione automatica di sicurezza .reg (Windows Registry Editor 5.00).",
                    )
                    .size(12.5)
                    .color(TEXT_SECONDARY),
                );
            });

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let fix_btn = egui::Button::new(
                    RichText::new("🛠️ Backup .reg + Ripara")
                        .size(14.0)
                        .strong()
                        .color(Color32::WHITE),
                )
                .fill(ACCENT_RUST)
                .min_size(egui::vec2(175.0, 38.0));

                if ui.add_enabled(!self.reg_issues.is_empty(), fix_btn).clicked() {
                    match backup_and_fix_registry(&self.reg_issues, &self.reg_backup_dir) {
                        Ok(report) => {
                            self.reg_last_backup = Some(report.backup_file.clone());
                            self.status_banner = format!(
                                "✅ Backup salvato in '{}' — Riparate {} voci ({} errori).",
                                report.backup_file.display(),
                                report.fixed_count,
                                report.failed_count
                            );
                            let active_cats: Vec<RegistryCategory> = self
                                .reg_categories
                                .iter()
                                .filter(|(_, en)| *en)
                                .map(|(c, _)| *c)
                                .collect();
                            self.reg_issues = scan_registry(&active_cats);
                        }
                        Err(e) => self.status_banner = e,
                    }
                }

                let export_btn = egui::Button::new(
                    RichText::new("💾 Esporta Backup .reg").size(13.5),
                )
                .min_size(egui::vec2(155.0, 38.0));

                if ui.add_enabled(!self.reg_issues.is_empty(), export_btn).clicked() {
                    match export_reg_backup(&self.reg_issues, &self.reg_backup_dir) {
                        Ok(path) => {
                            self.status_banner =
                                format!("💾 Backup del Registro esportato in: {}", path.display());
                            self.reg_last_backup = Some(path);
                        }
                        Err(e) => self.status_banner = e,
                    }
                }

                let scan_btn = egui::Button::new(
                    RichText::new("🔍 Scansiona Registro")
                        .size(14.0)
                        .strong()
                        .color(ACCENT_CYAN),
                )
                .fill(Color32::from_rgb(24, 38, 56))
                .stroke(Stroke::new(1.0, ACCENT_CYAN))
                .min_size(egui::vec2(165.0, 38.0));

                if ui.add(scan_btn).clicked() {
                    let active_cats: Vec<RegistryCategory> = self
                        .reg_categories
                        .iter()
                        .filter(|(_, en)| *en)
                        .map(|(c, _)| *c)
                        .collect();
                    self.reg_issues = scan_registry(&active_cats);
                    self.status_banner = format!(
                        "🔍 Scansione Registro completata: trovate {} incongruenze/chiavi orfane.",
                        self.reg_issues.len()
                    );
                }
            });
        });

        ui.add_space(10.0);

        // Category Filter Card
        card_frame().show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label(
                    RichText::new("Categorie Hive:")
                        .size(12.5)
                        .strong()
                        .color(TEXT_SECONDARY),
                );
                for (cat, enabled) in &mut self.reg_categories {
                    ui.checkbox(enabled, cat.code());
                }
            });
            if let Some(last_bak) = &self.reg_last_backup {
                ui.add_space(4.0);
                ui.label(
                    RichText::new(format!(
                        "🛡️ Ultimo file di ripristino .reg generato: {}",
                        last_bak.display()
                    ))
                    .size(12.0)
                    .color(ACCENT_EMERALD),
                );
            }
        });

        ui.add_space(10.0);

        // Issues List Card
        card_frame().show(ui, |ui| {
            egui::ScrollArea::vertical()
                .id_salt("reg_issues_scroll")
                .max_height(410.0)
                .show(ui, |ui| {
                    if self.reg_issues.is_empty() {
                        ui.vertical_centered(|ui| {
                            ui.add_space(80.0);
                            ui.label(RichText::new("🧩").size(38.0));
                            ui.label(
                                RichText::new("Nessuna Incongruenza Visualizzata")
                                    .size(15.5)
                                    .strong()
                                    .color(TEXT_PRIMARY),
                            );
                            ui.label(
                                RichText::new(
                                    "Clicca su 'Scansiona Registro' in alto a destra per cercare riferimenti orfani in HKCU, HKLM e HKCR.",
                                )
                                .size(12.5)
                                .color(TEXT_MUTED),
                            );
                            ui.add_space(80.0);
                        });
                    } else {
                        for issue in &mut self.reg_issues {
                            egui::Frame::new()
                                .fill(BG_CARD_SUBTLE)
                                .stroke(Stroke::new(1.0, BORDER_SUBTLE))
                                .corner_radius(CornerRadius::same(8))
                                .inner_margin(egui::Margin::symmetric(12, 8))
                                .show(ui, |ui| {
                                    ui.horizontal(|ui| {
                                        ui.checkbox(&mut issue.selected, "");
                                        draw_pill_badge(
                                            ui,
                                            issue.category.code(),
                                            Color32::from_rgba_unmultiplied(245, 85, 50, 35),
                                            Color32::from_rgb(255, 135, 100),
                                        );
                                        ui.label(
                                            RichText::new(&issue.problem_description)
                                                .size(13.5)
                                                .strong()
                                                .color(TEXT_PRIMARY),
                                        );
                                        ui.label(
                                            RichText::new(&issue.data_reference)
                                                .size(12.5)
                                                .color(ACCENT_CYAN),
                                        );
                                    });
                                    ui.label(
                                        RichText::new(format!("   Percorso: {}", issue.full_reg_path()))
                                            .size(11.5)
                                            .color(TEXT_MUTED),
                                    );
                                });
                            ui.add_space(4.0);
                        }
                    }
                });
        });
    }

    fn render_startup_tab(&mut self, ui: &mut egui::Ui) {
        let active_count = self.startup_entries.iter().filter(|e| e.enabled).count();
        let disabled_count = self.startup_entries.len().saturating_sub(active_count);

        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.label(
                    RichText::new("🚀 Gestione dell'Avvio Automatico (Startup Manager)")
                        .size(20.0)
                        .strong()
                        .color(TEXT_PRIMARY),
                );
                ui.label(
                    RichText::new(
                        "Controlla le applicazioni che partono con Windows tramite Run, RunOnce e i flag binari StartupApproved.",
                    )
                    .size(12.5)
                    .color(TEXT_SECONDARY),
                );
            });

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add(
                        egui::Button::new(RichText::new("🔄 Aggiorna Elenco").size(13.5).strong())
                            .min_size(egui::vec2(145.0, 36.0)),
                    )
                    .clicked()
                {
                    self.startup_entries = list_startup_entries();
                }
            });
        });

        ui.add_space(10.0);

        card_frame().show(ui, |ui| {
            ui.horizontal(|ui| {
                draw_pill_badge(
                    ui,
                    &format!("Totale: {}", self.startup_entries.len()),
                    Color32::from_rgba_unmultiplied(56, 189, 248, 30),
                    ACCENT_CYAN,
                );
                draw_pill_badge(
                    ui,
                    &format!("🟢 Attive: {active_count}"),
                    Color32::from_rgba_unmultiplied(16, 185, 129, 30),
                    ACCENT_EMERALD,
                );
                draw_pill_badge(
                    ui,
                    &format!("⚪ Disabilitate: {disabled_count}"),
                    Color32::from_rgba_unmultiplied(245, 158, 11, 30),
                    ACCENT_AMBER,
                );
                ui.separator();
                ui.label(RichText::new("🔍 Filtra:").color(TEXT_SECONDARY));
                ui.add(
                    egui::TextEdit::singleline(&mut self.startup_filter)
                        .hint_text("Cerca per nome o eseguibile...")
                        .desired_width(260.0),
                );
            });
        });

        ui.add_space(10.0);
        let filter_low = self.startup_filter.to_lowercase();
        let mut toggle_req: Option<(StartupEntry, bool)> = None;
        let mut delete_req: Option<StartupEntry> = None;

        card_frame().show(ui, |ui| {
            egui::ScrollArea::vertical()
                .id_salt("startup_scroll")
                .max_height(420.0)
                .show(ui, |ui| {
                    for entry in &self.startup_entries {
                        if !filter_low.is_empty()
                            && !entry.name.to_lowercase().contains(&filter_low)
                            && !entry.command.to_lowercase().contains(&filter_low)
                        {
                            continue;
                        }
                        egui::Frame::new()
                            .fill(BG_CARD_SUBTLE)
                            .stroke(Stroke::new(1.0, BORDER_SUBTLE))
                            .corner_radius(CornerRadius::same(8))
                            .inner_margin(egui::Margin::symmetric(12, 10))
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    if entry.enabled {
                                        draw_pill_badge(
                                            ui,
                                            "🟢 ATTIVO",
                                            Color32::from_rgba_unmultiplied(16, 185, 129, 35),
                                            ACCENT_EMERALD,
                                        );
                                    } else {
                                        draw_pill_badge(
                                            ui,
                                            "⚪ DISABILITATO",
                                            Color32::from_rgba_unmultiplied(245, 158, 11, 35),
                                            ACCENT_AMBER,
                                        );
                                    }
                                    draw_pill_badge(
                                        ui,
                                        entry.source.key_label(),
                                        Color32::from_rgba_unmultiplied(56, 189, 248, 25),
                                        ACCENT_CYAN,
                                    );
                                    ui.label(
                                        RichText::new(&entry.name)
                                            .size(14.5)
                                            .strong()
                                            .color(TEXT_PRIMARY),
                                    );

                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::Center),
                                        |ui| {
                                            if ui.button("🗑️ Elimina").clicked() {
                                                delete_req = Some(entry.clone());
                                            }
                                            let (toggle_label, btn_color) = if entry.enabled {
                                                ("⏸ Disabilita", Color32::from_rgb(45, 55, 72))
                                            } else {
                                                ("▶ Abilita", ACCENT_EMERALD)
                                            };
                                            if ui
                                                .add(
                                                    egui::Button::new(
                                                        RichText::new(toggle_label)
                                                            .strong()
                                                            .color(Color32::WHITE),
                                                    )
                                                    .fill(btn_color),
                                                )
                                                .clicked()
                                            {
                                                toggle_req = Some((entry.clone(), !entry.enabled));
                                            }
                                        },
                                    );
                                });
                                ui.add_space(2.0);
                                ui.label(
                                    RichText::new(format!("Comando: {}", entry.command))
                                        .size(11.5)
                                        .color(TEXT_MUTED),
                                );
                            });
                        ui.add_space(5.0);
                    }
                });
        });

        if let Some((entry, new_state)) = toggle_req {
            match set_startup_enabled(&entry, new_state) {
                Ok(()) => {
                    self.status_banner = format!(
                        "✅ Voce di avvio '{}' impostata su {}.",
                        entry.name,
                        if new_state { "ATTIVA" } else { "DISABILITATA" }
                    );
                    self.startup_entries = list_startup_entries();
                }
                Err(e) => self.status_banner = e,
            }
        }
        if let Some(entry) = delete_req {
            match delete_startup_entry(&entry) {
                Ok(()) => {
                    self.status_banner = format!("🗑️ Voce di avvio '{}' eliminata.", entry.name);
                    self.startup_entries = list_startup_entries();
                }
                Err(e) => self.status_banner = e,
            }
        }
    }

    fn render_uninstaller_tab(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.label(
                    RichText::new("📦 Disinstallazione Software (Uninstaller Manager)")
                        .size(20.0)
                        .strong()
                        .color(TEXT_PRIMARY),
                );
                ui.label(
                    RichText::new(
                        "Elenco completo delle applicazioni installate (64-bit, 32-bit WOW6432Node e Utente) con avvio diretto dell'uninstaller.",
                    )
                    .size(12.5)
                    .color(TEXT_SECONDARY),
                );
            });

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add(
                        egui::Button::new(RichText::new("🔄 Aggiorna Lista").size(13.5).strong())
                            .min_size(egui::vec2(145.0, 36.0)),
                    )
                    .clicked()
                {
                    self.installed_apps = list_installed_apps();
                }
            });
        });

        ui.add_space(10.0);

        card_frame().show(ui, |ui| {
            ui.horizontal(|ui| {
                draw_pill_badge(
                    ui,
                    &format!("📦 {} Applicazioni Rilevate", self.installed_apps.len()),
                    Color32::from_rgba_unmultiplied(168, 85, 247, 35),
                    ACCENT_PURPLE,
                );
                ui.separator();
                ui.label(RichText::new("🔍 Cerca:").color(TEXT_SECONDARY));
                ui.add(
                    egui::TextEdit::singleline(&mut self.uninstall_filter)
                        .hint_text("Filtra per nome software o produttore...")
                        .desired_width(300.0),
                );
            });
        });

        ui.add_space(10.0);
        let filter_low = self.uninstall_filter.to_lowercase();
        let mut launch_req: Option<InstalledApp> = None;

        card_frame().show(ui, |ui| {
            egui::ScrollArea::vertical()
                .id_salt("uninstaller_scroll")
                .max_height(420.0)
                .show(ui, |ui| {
                    for app in &self.installed_apps {
                        if !filter_low.is_empty()
                            && !app.display_name.to_lowercase().contains(&filter_low)
                            && !app.publisher.to_lowercase().contains(&filter_low)
                        {
                            continue;
                        }
                        egui::Frame::new()
                            .fill(BG_CARD_SUBTLE)
                            .stroke(Stroke::new(1.0, BORDER_SUBTLE))
                            .corner_radius(CornerRadius::same(8))
                            .inner_margin(egui::Margin::symmetric(12, 10))
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    ui.label(
                                        RichText::new(&app.display_name)
                                            .size(14.5)
                                            .strong()
                                            .color(TEXT_PRIMARY),
                                    );
                                    if !app.display_version.is_empty() {
                                        draw_pill_badge(
                                            ui,
                                            &format!("v{}", app.display_version),
                                            Color32::from_rgba_unmultiplied(56, 189, 248, 30),
                                            ACCENT_CYAN,
                                        );
                                    }
                                    if let Some(kb) = app.estimated_size_kb {
                                        draw_pill_badge(
                                            ui,
                                            &format!("{:.1} MB", kb as f64 / 1024.0),
                                            Color32::from_rgba_unmultiplied(16, 185, 129, 30),
                                            ACCENT_EMERALD,
                                        );
                                    }
                                    if !app.publisher.is_empty() {
                                        ui.label(
                                            RichText::new(format!("• {}", app.publisher))
                                                .size(12.0)
                                                .color(TEXT_SECONDARY),
                                        );
                                    }

                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::Center),
                                        |ui| {
                                            let uninst_btn = egui::Button::new(
                                                RichText::new("🚀 Disinstalla")
                                                    .size(12.5)
                                                    .strong()
                                                    .color(Color32::WHITE),
                                            )
                                            .fill(ACCENT_RUST);
                                            if ui.add(uninst_btn).clicked() {
                                                launch_req = Some(app.clone());
                                            }
                                        },
                                    );
                                });
                                ui.add_space(2.0);
                                ui.label(
                                    RichText::new(format!("Comando: {}", app.uninstall_string))
                                        .size(11.0)
                                        .color(TEXT_MUTED),
                                );
                            });
                        ui.add_space(5.0);
                    }
                });
        });

        if let Some(app) = launch_req {
            match launch_uninstaller(&app) {
                Ok(msg) => self.status_banner = msg,
                Err(e) => self.status_banner = e,
            }
        }
    }

    fn render_wiper_tab(&mut self, ui: &mut egui::Ui) {
        ui.label(
            RichText::new("💿 Bonifica dello Spazio Libero (Drive Wiper)")
                .size(20.0)
                .strong()
                .color(TEXT_PRIMARY),
        );
        ui.label(
            RichText::new(
                "Sovrascrittura sicura dello spazio non allocato con blocchi da 4 MB e flag nativo FILE_FLAG_WRITE_THROUGH (bypass cache OS).",
            )
            .size(12.5)
            .color(TEXT_SECONDARY),
        );
        ui.add_space(12.0);

        ui.columns(2, |cols| {
            // Left Card: Drive Selection with Visual Gauges
            card_frame().show(&mut cols[0], |ui| {
                ui.label(
                    RichText::new("1. Seleziona Volume Disco")
                        .size(15.0)
                        .strong()
                        .color(TEXT_PRIMARY),
                );
                ui.separator();

                for (idx, d) in self.drives.iter().enumerate() {
                    let is_sel = self.selected_drive_idx == idx;
                    let used_ratio = if d.total_bytes > 0 {
                        (d.used_bytes as f32 / d.total_bytes as f32).clamp(0.0, 1.0)
                    } else {
                        0.0
                    };

                    egui::Frame::new()
                        .fill(if is_sel {
                            Color32::from_rgb(30, 41, 60)
                        } else {
                            BG_CARD_SUBTLE
                        })
                        .stroke(Stroke::new(
                            1.0,
                            if is_sel { ACCENT_CYAN } else { BORDER_SUBTLE },
                        ))
                        .corner_radius(CornerRadius::same(8))
                        .inner_margin(egui::Margin::symmetric(12, 10))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.radio_value(
                                    &mut self.selected_drive_idx,
                                    idx,
                                    RichText::new(format!("💽 Disco {}", d.root_path))
                                        .size(14.5)
                                        .strong(),
                                );
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        draw_pill_badge(
                                            ui,
                                            &format!("Libero: {}", format_bytes(d.free_bytes)),
                                            Color32::from_rgba_unmultiplied(16, 185, 129, 35),
                                            ACCENT_EMERALD,
                                        );
                                    },
                                );
                            });

                            let (bar_rect, _) = ui.allocate_exact_size(
                                egui::vec2(ui.available_width(), 8.0),
                                Sense::hover(),
                            );
                            ui.painter().rect_filled(
                                bar_rect,
                                CornerRadius::same(4),
                                Color32::from_rgb(35, 45, 62),
                            );
                            let mut used_rect = bar_rect;
                            used_rect.set_width(bar_rect.width() * used_ratio);
                            ui.painter().rect_filled(
                                used_rect,
                                CornerRadius::same(4),
                                ACCENT_CYAN,
                            );

                            ui.label(
                                RichText::new(format!(
                                    "Usato: {} ({:.0}%) su {} totali",
                                    format_bytes(d.used_bytes),
                                    used_ratio * 100.0,
                                    format_bytes(d.total_bytes)
                                ))
                                .size(11.5)
                                .color(TEXT_MUTED),
                            );
                        });
                    ui.add_space(6.0);
                }
            });

            // Right Card: Security Algorithm & Options
            card_frame().show(&mut cols[1], |ui| {
                ui.label(
                    RichText::new("2. Algoritmo di Sicurezza & Opzioni")
                        .size(15.0)
                        .strong()
                        .color(TEXT_PRIMARY),
                );
                ui.separator();

                for &alg in WipeAlgorithm::all() {
                    let is_sel = self.wipe_algorithm == alg;
                    egui::Frame::new()
                        .fill(if is_sel {
                            Color32::from_rgb(36, 30, 46)
                        } else {
                            BG_CARD_SUBTLE
                        })
                        .stroke(Stroke::new(
                            1.0,
                            if is_sel { ACCENT_RUST } else { BORDER_SUBTLE },
                        ))
                        .corner_radius(CornerRadius::same(8))
                        .inner_margin(egui::Margin::symmetric(10, 8))
                        .show(ui, |ui| {
                            ui.radio_value(
                                &mut self.wipe_algorithm,
                                alg,
                                RichText::new(alg.label()).size(13.5).strong(),
                            );
                        });
                    ui.add_space(4.0);
                }

                ui.add_space(6.0);
                ui.checkbox(
                    &mut self.wipe_mft_slack,
                    "Bonifica anche i record liberi della Master File Table (MFT Slack)",
                );

                ui.horizontal(|ui| {
                    ui.checkbox(
                        &mut self.wipe_limit_mb_enabled,
                        "Limite di sicurezza (MB):",
                    );
                    if self.wipe_limit_mb_enabled {
                        ui.add(
                            egui::DragValue::new(&mut self.wipe_limit_mb)
                                .range(16..=65536)
                                .suffix(" MB"),
                        );
                    } else {
                        draw_pill_badge(
                            ui,
                            "100% Spazio Libero (-512 MB margine OS)",
                            Color32::from_rgba_unmultiplied(245, 158, 11, 35),
                            ACCENT_AMBER,
                        );
                    }
                });
            });
        });

        ui.add_space(12.0);
        let prog_snapshot = self.wipe_progress.lock().unwrap().clone();

        card_frame().show(ui, |ui| {
            ui.horizontal(|ui| {
                let start_btn = egui::Button::new(
                    RichText::new("💿 Avvia Bonifica Spazio Libero")
                        .size(15.0)
                        .strong()
                        .color(Color32::WHITE),
                )
                .fill(ACCENT_RUST)
                .min_size(egui::vec2(240.0, 40.0));

                if ui.add_enabled(!prog_snapshot.running, start_btn).clicked() {
                    if let Some(drive) = self.drives.get(self.selected_drive_idx) {
                        let drive_root = drive.root_path.clone();
                        let alg = self.wipe_algorithm;
                        let mft = self.wipe_mft_slack;
                        let limit = if self.wipe_limit_mb_enabled {
                            Some(self.wipe_limit_mb)
                        } else {
                            None
                        };
                        self.wipe_cancel.store(false, Ordering::Relaxed);
                        let prog_arc = Arc::clone(&self.wipe_progress);
                        let cancel_arc = Arc::clone(&self.wipe_cancel);

                        thread::spawn(move || {
                            if let Err(e) = wipe_free_space(
                                &drive_root,
                                alg,
                                mft,
                                limit,
                                prog_arc.clone(),
                                cancel_arc,
                            ) {
                                if let Ok(mut p) = prog_arc.lock() {
                                    p.running = false;
                                    p.finished = true;
                                    p.error = Some(e);
                                }
                            }
                        });
                    }
                }

                if prog_snapshot.running {
                    let stop_btn = egui::Button::new(
                        RichText::new("⏹ Interrompi Subito e Rilascia Spazio")
                            .size(14.0)
                            .strong()
                            .color(Color32::WHITE),
                    )
                    .fill(ACCENT_RED)
                    .min_size(egui::vec2(240.0, 40.0));

                    if ui.add(stop_btn).clicked() {
                        self.wipe_cancel.store(true, Ordering::Relaxed);
                    }
                }
            });

            if prog_snapshot.running || prog_snapshot.finished {
                ui.add_space(10.0);
                let fraction = if prog_snapshot.target_bytes_per_pass > 0 {
                    (prog_snapshot.bytes_written_in_pass as f32)
                        / (prog_snapshot.target_bytes_per_pass as f32)
                } else {
                    0.0
                };
                ui.add(
                    egui::ProgressBar::new(fraction.clamp(0.0, 1.0))
                        .show_percentage()
                        .text(&prog_snapshot.status_message),
                );
                if let Some(err) = &prog_snapshot.error {
                    ui.label(RichText::new(err).color(ACCENT_RED));
                }
            }
        });
    }
}
