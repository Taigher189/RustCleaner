//! UI/UX Pro Max Desktop GUI for RustCleaner built with `eframe` / `egui`.
//!
//! Designed and audited with UI/UX Pro Max Design System rules:
//! - Style: Dark Mode (OLED) + Bento KPI Grid + Technical Developer/System Utility precision
//! - Icons: 100% custom resolution-independent `egui::Painter` vector icons (1.6px consistent stroke,
//!   zero font/emoji dependency, dedicated bounding boxes with zero text overlap)
//! - Typography: Strict type scale + Monospace tabular numbers/versions/paths (`number-tabular`)
//!   + tooltip disclosure for long commands/paths (`truncation-strategy`)
//! - Accessibility: WCAG AA >= 4.5:1 normal text contrast on all dark surfaces (`TEXT_MUTED` #94A3B8),
//!   color-never-alone status indicators (`color-not-only`), pointing-hand cursor on all interactive controls
//! - Action Hierarchy: Distinct Primary Scan, Execute Action, Secondary, and Destructive button variants

use crate::cleaner::{
    analyze_all, default_clean_rules, execute_clean, format_bytes, CleanRule, CleanSummary,
    RuleGroup,
};
use crate::driver_updater::{install_wu_driver_updates, scan_drivers, DriverScanReport};
use crate::monitor::{start_realtime_watcher, RealtimeMonitorState};
use crate::registry::{
    backup_and_fix_registry, export_reg_backup, scan_registry, RegistryCategory, RegistryIssue,
};
use crate::scheduler::{
    create_scheduled_task, delete_scheduled_task, query_scheduled_task, read_scheduled_logs,
    run_silent_clean_and_log, ScheduleFrequency, ScheduledTaskStatus,
};
use crate::software_updater::{
    check_for_updates, upgrade_all_packages, upgrade_single_package, WingetScanReport,
};
use crate::startup_uninstall::{
    delete_startup_entry, launch_uninstaller, list_installed_apps, list_startup_entries,
    set_startup_enabled, InstalledApp, StartupEntry,
};
use crate::uac;
use crate::wiper::{list_drives, wipe_free_space, DriveInfo, WipeAlgorithm, WipeProgress};
use eframe::egui::{self, Color32, CornerRadius, Pos2, Rect, RichText, Sense, Stroke, Vec2};
use std::env;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;

// ============================================================================
// UI/UX PRO MAX — SEMANTIC DESIGN TOKENS (Dark Mode OLED + Developer Utility)
// ============================================================================
const BG_CANVAS: Color32 = Color32::from_rgb(11, 15, 25); // #0B0F19 Deep Slate Canvas
const BG_SIDEBAR: Color32 = Color32::from_rgb(15, 23, 42); // #0F172A Slate 900 Sidebar
const BG_HEADER: Color32 = Color32::from_rgb(13, 19, 34); // #0D1322 Top/Bottom Chrome
const BG_CARD: Color32 = Color32::from_rgb(22, 31, 51); // #161F33 Elevated Card Surface
const BG_CARD_SUBTLE: Color32 = Color32::from_rgb(17, 24, 39); // #111827 Recessed Row Surface
const BG_CARD_HOVER: Color32 = Color32::from_rgb(30, 41, 59); // #1E293B Interactive Hover Surface

const BORDER_SUBTLE: Color32 = Color32::from_rgb(45, 58, 79); // #2D3A4F Visible 1px Separator
const BORDER_STRONG: Color32 = Color32::from_rgb(71, 85, 105); // #475569 Active/Focused Border

// WCAG AA Verified Text Hierarchy (>= 4.5:1 against BG_CANVAS & BG_CARD)
const TEXT_PRIMARY: Color32 = Color32::from_rgb(248, 250, 252); // #F8FAFC (>15:1 contrast)
const TEXT_SECONDARY: Color32 = Color32::from_rgb(203, 213, 225); // #CBD5E1 (>10:1 contrast)
const TEXT_MUTED: Color32 = Color32::from_rgb(148, 163, 184); // #94A3B8 (>5.5:1 WCAG AA pass)

// Semantic Functional Accents
const ACCENT_RUST: Color32 = Color32::from_rgb(249, 115, 22); // #F97316 Brand / Execute Action
const ACCENT_EMERALD: Color32 = Color32::from_rgb(34, 197, 94); // #22C55E Safe / Verified / Active
const ACCENT_CYAN: Color32 = Color32::from_rgb(56, 189, 248); // #38BDF8 Info / Scan / Telemetry-Free
const ACCENT_AMBER: Color32 = Color32::from_rgb(245, 158, 11); // #F59E0B Warning / UAC / Pending
const ACCENT_PURPLE: Color32 = Color32::from_rgb(168, 85, 247); // #A855F7 System / WHQL / Kernel
const ACCENT_RED: Color32 = Color32::from_rgb(239, 68, 68); // #EF4444 Destructive / Stop

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActiveTab {
    Cleaner,
    Registry,
    Startup,
    Uninstaller,
    DriveWiper,
    SoftwareUpdater,
    DriverUpdater,
    AutomationMonitor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum VectorIcon {
    BrandShield,
    CleanerSweep,
    RegistryGrid,
    StartupBolt,
    UninstallerBox,
    DriveCylinder,
    SoftwareCloudUp,
    DriverChip,
    AutomationPulse,
    SearchLens,
    PlayRun,
    CheckShield,
    TrashDelete,
    RefreshSync,
    StopOctagon,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ButtonVariant {
    PrimaryScan,
    ExecuteBrand,
    Secondary,
    Danger,
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

    // 5. Winget Software Updater State
    pub winget_report: Option<WingetScanReport>,
    pub winget_busy: bool,
    pub winget_rx: Arc<Mutex<Option<WingetScanReport>>>,
    pub winget_action_rx: Arc<Mutex<Option<String>>>,
    pub winget_filter: String,

    // 6. Driver Updater (WUApiLib COM + PnP) State
    pub driver_report: Option<DriverScanReport>,
    pub driver_busy: bool,
    pub driver_rx: Arc<Mutex<Option<DriverScanReport>>>,
    pub driver_filter: String,

    // 7. Zero-RAM Scheduler (`schtasks.exe`) & Zero-Polling Kernel Monitor (`notify`)
    pub schedule_status: ScheduledTaskStatus,
    pub schedule_freq: ScheduleFrequency,
    pub schedule_time_hhmm: String,
    pub schedule_highest_uac: bool,
    pub schedule_logs: Vec<String>,
    pub monitor_state: Arc<Mutex<RealtimeMonitorState>>,
    pub monitor_stop_flag: Arc<AtomicBool>,
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
                "Pronto — Privilegi Amministratore (UAC) attivi • Zero Telemetria • 0 MB RAM in background.".to_string()
            } else {
                "Pronto — Modalità Utente Standard • Zero Telemetria. Usa 'Eleva UAC' per pulizia profonda di sistema.".to_string()
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
            winget_report: None,
            winget_busy: false,
            winget_rx: Arc::new(Mutex::new(None)),
            winget_action_rx: Arc::new(Mutex::new(None)),
            winget_filter: String::new(),
            driver_report: None,
            driver_busy: false,
            driver_rx: Arc::new(Mutex::new(None)),
            driver_filter: String::new(),
            schedule_status: query_scheduled_task(),
            schedule_freq: ScheduleFrequency::Weekly,
            schedule_time_hhmm: "18:00".to_string(),
            schedule_highest_uac: false,
            schedule_logs: read_scheduled_logs(),
            monitor_state: Arc::new(Mutex::new(RealtimeMonitorState::default())),
            monitor_stop_flag: Arc::new(AtomicBool::new(false)),
        }
    }
}

fn configure_pro_max_theme(ctx: &egui::Context) {
    let mut style = (*ctx.style()).clone();
    let mut visuals = egui::Visuals::dark();

    visuals.panel_fill = BG_CANVAS;
    visuals.window_fill = BG_CARD;
    visuals.extreme_bg_color = Color32::from_rgb(8, 11, 19);
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

    visuals.widgets.inactive.bg_fill = Color32::from_rgb(30, 41, 59);
    visuals.widgets.inactive.weak_bg_fill = Color32::from_rgb(24, 34, 50);
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, BORDER_SUBTLE);
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, TEXT_PRIMARY);
    visuals.widgets.inactive.corner_radius = radius;

    visuals.widgets.hovered.bg_fill = Color32::from_rgb(41, 55, 78);
    visuals.widgets.hovered.weak_bg_fill = Color32::from_rgb(35, 48, 68);
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.5, ACCENT_CYAN);
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.5, Color32::WHITE);
    visuals.widgets.hovered.corner_radius = radius;

    visuals.widgets.active.bg_fill = ACCENT_RUST;
    visuals.widgets.active.weak_bg_fill = ACCENT_RUST;
    visuals.widgets.active.bg_stroke = Stroke::new(1.5, Color32::WHITE);
    visuals.widgets.active.fg_stroke = Stroke::new(1.5, Color32::WHITE);
    visuals.widgets.active.corner_radius = radius;

    visuals.selection.bg_fill = Color32::from_rgba_unmultiplied(56, 189, 248, 65);
    visuals.selection.stroke = Stroke::new(1.5, ACCENT_CYAN);

    style.visuals = visuals;
    // 8dp / 4dp Spacing Rhythm (UI/UX Pro Max `spacing-scale`)
    style.spacing.item_spacing = egui::vec2(8.0, 8.0);
    style.spacing.button_padding = egui::vec2(12.0, 7.0);
    style.spacing.window_margin = egui::Margin::same(12);
    ctx.set_style(style);
}

// ============================================================================
// UI/UX PRO MAX — VECTOR ICON ENGINE (No Emoji Dependency, 1.6px Stroke)
// ============================================================================
fn paint_vector_icon(painter: &egui::Painter, rect: Rect, icon: VectorIcon, color: Color32) {
    let c = rect.center();
    let r = (rect.width().min(rect.height()) * 0.36).max(4.0);
    let s = Stroke::new(1.6, color);

    match icon {
        VectorIcon::BrandShield => {
            let pts = vec![
                Pos2::new(c.x, c.y - r),
                Pos2::new(c.x + r * 0.85, c.y - r * 0.45),
                Pos2::new(c.x + r * 0.65, c.y + r * 0.45),
                Pos2::new(c.x, c.y + r),
                Pos2::new(c.x - r * 0.65, c.y + r * 0.45),
                Pos2::new(c.x - r * 0.85, c.y - r * 0.45),
            ];
            painter.add(egui::Shape::closed_line(pts, s));
            painter.circle_filled(c, r * 0.25, color);
        }
        VectorIcon::CleanerSweep => {
            // Sparkle / Four-point precision star
            painter.line_segment([Pos2::new(c.x, c.y - r), Pos2::new(c.x, c.y + r)], s);
            painter.line_segment([Pos2::new(c.x - r, c.y), Pos2::new(c.x + r, c.y)], s);
            let d = r * 0.55;
            painter.line_segment(
                [Pos2::new(c.x - d, c.y - d), Pos2::new(c.x + d, c.y + d)],
                s,
            );
            painter.line_segment(
                [Pos2::new(c.x + d, c.y - d), Pos2::new(c.x - d, c.y + d)],
                s,
            );
        }
        VectorIcon::RegistryGrid => {
            // 2x2 Modular Registry Hive Cubes
            let g = r * 0.18;
            let b = r * 0.72;
            let tl = Rect::from_min_max(Pos2::new(c.x - b, c.y - b), Pos2::new(c.x - g, c.y - g));
            let tr = Rect::from_min_max(Pos2::new(c.x + g, c.y - b), Pos2::new(c.x + b, c.y - g));
            let bl = Rect::from_min_max(Pos2::new(c.x - b, c.y + g), Pos2::new(c.x - g, c.y + b));
            let br = Rect::from_min_max(Pos2::new(c.x + g, c.y + g), Pos2::new(c.x + b, c.y + b));
            for sq in [tl, tr, bl, br] {
                painter.rect_stroke(sq, CornerRadius::same(1), s, egui::StrokeKind::Inside);
            }
        }
        VectorIcon::StartupBolt => {
            // Lightning / Fast Boot Bolt
            let pts = vec![
                Pos2::new(c.x + r * 0.15, c.y - r),
                Pos2::new(c.x - r * 0.65, c.y + r * 0.1),
                Pos2::new(c.x + r * 0.05, c.y + r * 0.1),
                Pos2::new(c.x - r * 0.15, c.y + r),
                Pos2::new(c.x + r * 0.65, c.y - r * 0.1),
                Pos2::new(c.x - r * 0.05, c.y - r * 0.1),
            ];
            painter.add(egui::Shape::closed_line(pts, s));
        }
        VectorIcon::UninstallerBox => {
            // Package Box + Minus line
            let box_r = Rect::from_center_size(c, Vec2::splat(r * 1.55));
            painter.rect_stroke(box_r, CornerRadius::same(2), s, egui::StrokeKind::Inside);
            painter.line_segment(
                [
                    Pos2::new(box_r.left(), c.y - r * 0.25),
                    Pos2::new(box_r.right(), c.y - r * 0.25),
                ],
                s,
            );
            painter.line_segment(
                [
                    Pos2::new(c.x - r * 0.35, c.y + r * 0.25),
                    Pos2::new(c.x + r * 0.35, c.y + r * 0.25),
                ],
                s,
            );
        }
        VectorIcon::DriveCylinder => {
            // Storage Drive Enclosure + Activity LED
            let drv = Rect::from_center_size(c, Vec2::new(r * 1.75, r * 1.25));
            painter.rect_stroke(drv, CornerRadius::same(3), s, egui::StrokeKind::Inside);
            painter.line_segment(
                [
                    Pos2::new(drv.left() + 3.0, c.y + r * 0.15),
                    Pos2::new(drv.right() - 3.0, c.y + r * 0.15),
                ],
                s,
            );
            painter.circle_filled(Pos2::new(drv.right() - r * 0.4, c.y - r * 0.2), 2.0, color);
        }
        VectorIcon::SoftwareCloudUp => {
            // Upward Arrow inside Circular Package Ring
            painter.circle_stroke(c, r * 0.9, s);
            painter.line_segment(
                [
                    Pos2::new(c.x, c.y + r * 0.45),
                    Pos2::new(c.x, c.y - r * 0.45),
                ],
                s,
            );
            painter.line_segment(
                [
                    Pos2::new(c.x - r * 0.38, c.y - r * 0.08),
                    Pos2::new(c.x, c.y - r * 0.45),
                ],
                s,
            );
            painter.line_segment(
                [
                    Pos2::new(c.x + r * 0.38, c.y - r * 0.08),
                    Pos2::new(c.x, c.y - r * 0.45),
                ],
                s,
            );
        }
        VectorIcon::DriverChip => {
            // Hardware Microchip die + Pins
            let die = Rect::from_center_size(c, Vec2::splat(r * 1.15));
            painter.rect_stroke(die, CornerRadius::same(2), s, egui::StrokeKind::Inside);
            for offset in [-r * 0.3, r * 0.3] {
                painter.line_segment(
                    [
                        Pos2::new(c.x + offset, die.top() - 3.0),
                        Pos2::new(c.x + offset, die.top()),
                    ],
                    s,
                );
                painter.line_segment(
                    [
                        Pos2::new(c.x + offset, die.bottom()),
                        Pos2::new(c.x + offset, die.bottom() + 3.0),
                    ],
                    s,
                );
                painter.line_segment(
                    [
                        Pos2::new(die.left() - 3.0, c.y + offset),
                        Pos2::new(die.left(), c.y + offset),
                    ],
                    s,
                );
                painter.line_segment(
                    [
                        Pos2::new(die.right(), c.y + offset),
                        Pos2::new(die.right() + 3.0, c.y + offset),
                    ],
                    s,
                );
            }
        }
        VectorIcon::AutomationPulse => {
            // Kernel Pulse Wave
            let pts = vec![
                Pos2::new(c.x - r, c.y),
                Pos2::new(c.x - r * 0.45, c.y),
                Pos2::new(c.x - r * 0.2, c.y - r * 0.75),
                Pos2::new(c.x + r * 0.2, c.y + r * 0.75),
                Pos2::new(c.x + r * 0.45, c.y),
                Pos2::new(c.x + r, c.y),
            ];
            painter.add(egui::Shape::line(pts, s));
        }
        VectorIcon::SearchLens => {
            let lc = Pos2::new(c.x - r * 0.15, c.y - r * 0.15);
            painter.circle_stroke(lc, r * 0.62, s);
            painter.line_segment(
                [
                    Pos2::new(lc.x + r * 0.44, lc.y + r * 0.44),
                    Pos2::new(c.x + r * 0.85, c.y + r * 0.85),
                ],
                s,
            );
        }
        VectorIcon::PlayRun => {
            let pts = vec![
                Pos2::new(c.x - r * 0.45, c.y - r * 0.7),
                Pos2::new(c.x + r * 0.65, c.y),
                Pos2::new(c.x - r * 0.45, c.y + r * 0.7),
            ];
            painter.add(egui::Shape::closed_line(pts, s));
        }
        VectorIcon::CheckShield => {
            painter.circle_stroke(c, r * 0.88, s);
            let pts = vec![
                Pos2::new(c.x - r * 0.42, c.y + r * 0.02),
                Pos2::new(c.x - r * 0.1, c.y + r * 0.36),
                Pos2::new(c.x + r * 0.46, c.y - r * 0.32),
            ];
            painter.add(egui::Shape::line(pts, s));
        }
        VectorIcon::TrashDelete => {
            let b = Rect::from_min_max(
                Pos2::new(c.x - r * 0.55, c.y - r * 0.45),
                Pos2::new(c.x + r * 0.55, c.y + r * 0.8),
            );
            painter.rect_stroke(b, CornerRadius::same(2), s, egui::StrokeKind::Inside);
            painter.line_segment(
                [
                    Pos2::new(c.x - r * 0.75, c.y - r * 0.45),
                    Pos2::new(c.x + r * 0.75, c.y - r * 0.45),
                ],
                s,
            );
        }
        VectorIcon::RefreshSync => {
            painter.circle_stroke(c, r * 0.75, s);
            painter.circle_filled(Pos2::new(c.x + r * 0.75, c.y), 2.2, color);
        }
        VectorIcon::StopOctagon => {
            let sq = Rect::from_center_size(c, Vec2::splat(r * 1.1));
            painter.rect_filled(sq, CornerRadius::same(2), color);
        }
    }
}

/// Renders a semantic action button with a vector icon, proper contrast, and pointing-hand cursor.
fn draw_action_button(
    ui: &mut egui::Ui,
    label: &str,
    icon: VectorIcon,
    variant: ButtonVariant,
    enabled: bool,
) -> bool {
    let (bg_fill, border_stroke, text_color) = match variant {
        ButtonVariant::PrimaryScan => (
            Color32::from_rgb(18, 42, 68),
            Stroke::new(1.2, ACCENT_CYAN),
            ACCENT_CYAN,
        ),
        ButtonVariant::ExecuteBrand => (
            ACCENT_RUST,
            Stroke::new(1.0, Color32::from_rgb(255, 150, 90)),
            Color32::WHITE,
        ),
        ButtonVariant::Secondary => (
            Color32::from_rgb(30, 41, 59),
            Stroke::new(1.0, BORDER_STRONG),
            TEXT_PRIMARY,
        ),
        ButtonVariant::Danger => (
            Color32::from_rgb(69, 20, 26),
            Stroke::new(1.2, ACCENT_RED),
            Color32::from_rgb(254, 202, 202),
        ),
    };

    let fill = if enabled {
        bg_fill
    } else {
        Color32::from_rgba_unmultiplied(bg_fill.r(), bg_fill.g(), bg_fill.b(), 85)
    };
    let fg = if enabled { text_color } else { TEXT_MUTED };
    let stroke = if enabled {
        border_stroke
    } else {
        Stroke::new(1.0, BORDER_SUBTLE)
    };

    let mut clicked = false;
    let resp = egui::Frame::new()
        .fill(fill)
        .stroke(stroke)
        .corner_radius(CornerRadius::same(8))
        .inner_margin(egui::Margin::symmetric(12, 7))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                let (icon_rect, _) = ui.allocate_exact_size(egui::vec2(16.0, 16.0), Sense::hover());
                paint_vector_icon(ui.painter(), icon_rect, icon, fg);
                ui.label(RichText::new(label).size(13.0).strong().color(fg));
            });
        });

    if enabled {
        let interact = ui.interact(
            resp.response.rect,
            ui.id().with(label),
            Sense::click(),
        );
        if interact.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            ui.painter().rect_stroke(
                resp.response.rect,
                CornerRadius::same(8),
                Stroke::new(1.5, fg),
                egui::StrokeKind::Inside,
            );
        }
        if interact.clicked() {
            clicked = true;
        }
    }
    clicked
}

/// Renders a Bento-grid KPI metric card with tabular monospace value (`number-tabular`).
fn draw_kpi_card(
    ui: &mut egui::Ui,
    title: &str,
    value: &str,
    subtitle: &str,
    accent: Color32,
    icon: VectorIcon,
) {
    let avail_w = ui.available_width();
    egui::Frame::new()
        .fill(BG_CARD)
        .stroke(Stroke::new(1.0, BORDER_SUBTLE))
        .corner_radius(CornerRadius::same(10))
        .inner_margin(egui::Margin::symmetric(14, 11))
        .show(ui, |ui| {
            ui.set_width((avail_w - 28.0).max(110.0));
            ui.horizontal(|ui| {
                let (icon_rect, _) = ui.allocate_exact_size(egui::vec2(18.0, 18.0), Sense::hover());
                paint_vector_icon(ui.painter(), icon_rect, icon, accent);
                ui.label(
                    RichText::new(title)
                        .size(11.0)
                        .color(TEXT_SECONDARY)
                        .strong(),
                );
            });
            ui.add_space(2.0);
            ui.label(
                RichText::new(value)
                    .size(19.5)
                    .strong()
                    .monospace()
                    .color(TEXT_PRIMARY),
            );
            ui.label(RichText::new(subtitle).size(11.0).color(TEXT_MUTED));
        });
}

/// Single-line non-wrapping status pill badge (`compact-label-overflow`).
fn draw_pill_badge(ui: &mut egui::Ui, text: &str, bg: Color32, fg: Color32) {
    egui::Frame::new()
        .fill(bg)
        .stroke(Stroke::new(
            1.0,
            Color32::from_rgba_unmultiplied(fg.r(), fg.g(), fg.b(), 70),
        ))
        .corner_radius(CornerRadius::same(14))
        .inner_margin(egui::Margin::symmetric(8, 3))
        .show(ui, |ui| {
            ui.add(
                egui::Label::new(RichText::new(text).size(11.0).strong().color(fg))
                    .wrap_mode(egui::TextWrapMode::Extend),
            );
        });
}

fn card_frame() -> egui::Frame {
    egui::Frame::new()
        .fill(BG_CARD)
        .stroke(Stroke::new(1.0, BORDER_SUBTLE))
        .corner_radius(CornerRadius::same(10))
        .inner_margin(egui::Margin::symmetric(14, 12))
}

/// Renders an Empty State illustration + title + description (`empty-states` rule).
fn draw_empty_state(ui: &mut egui::Ui, icon: VectorIcon, accent: Color32, title: &str, desc: &str) {
    ui.vertical_centered(|ui| {
        ui.add_space(44.0);
        egui::Frame::new()
            .fill(Color32::from_rgba_unmultiplied(
                accent.r(),
                accent.g(),
                accent.b(),
                28,
            ))
            .stroke(Stroke::new(
                1.0,
                Color32::from_rgba_unmultiplied(accent.r(), accent.g(), accent.b(), 90),
            ))
            .corner_radius(CornerRadius::same(24))
            .inner_margin(egui::Margin::same(12))
            .show(ui, |ui| {
                let (r, _) = ui.allocate_exact_size(egui::vec2(26.0, 26.0), Sense::hover());
                paint_vector_icon(ui.painter(), r, icon, accent);
            });
        ui.add_space(10.0);
        ui.label(
            RichText::new(title)
                .size(15.0)
                .strong()
                .color(TEXT_PRIMARY),
        );
        ui.add_space(4.0);
        ui.label(RichText::new(desc).size(12.0).color(TEXT_MUTED));
    });
}

/// Renders a sidebar navigation item with a custom vector icon, active left indicator bar,
/// and optional live badge count (`nav-state-active` & `tab-badge`).
fn draw_sidebar_nav_item(
    ui: &mut egui::Ui,
    icon: VectorIcon,
    accent: Color32,
    title: &str,
    subtitle: &str,
    badge_count: Option<usize>,
    is_selected: bool,
) -> bool {
    let mut clicked = false;
    let fill = if is_selected {
        BG_CARD_HOVER
    } else {
        BG_CARD_SUBTLE
    };
    let border = if is_selected {
        Stroke::new(1.5, accent)
    } else {
        Stroke::new(1.0, BORDER_SUBTLE)
    };

    let frame_resp = egui::Frame::new()
        .fill(fill)
        .stroke(border)
        .corner_radius(CornerRadius::same(9))
        .inner_margin(egui::Margin::symmetric(10, 7))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 8.0;

                // Dedicated 28x28 vector icon container (Zero coordinate overlap!)
                egui::Frame::new()
                    .fill(if is_selected {
                        accent
                    } else {
                        Color32::from_rgba_unmultiplied(accent.r(), accent.g(), accent.b(), 32)
                    })
                    .corner_radius(CornerRadius::same(7))
                    .inner_margin(egui::Margin::same(5))
                    .show(ui, |ui| {
                        let (icon_rect, _) =
                            ui.allocate_exact_size(egui::vec2(18.0, 18.0), Sense::hover());
                        paint_vector_icon(
                            ui.painter(),
                            icon_rect,
                            icon,
                            if is_selected {
                                Color32::WHITE
                            } else {
                                accent
                            },
                        );
                    });

                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 1.5;
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(title)
                                .size(13.0)
                                .strong()
                                .color(if is_selected {
                                    Color32::WHITE
                                } else {
                                    TEXT_PRIMARY
                                }),
                        );
                        if let Some(cnt) = badge_count {
                            if cnt > 0 {
                                draw_pill_badge(
                                    ui,
                                    &cnt.to_string(),
                                    Color32::from_rgba_unmultiplied(
                                        accent.r(),
                                        accent.g(),
                                        accent.b(),
                                        45,
                                    ),
                                    accent,
                                );
                            }
                        }
                    });
                    ui.label(
                        RichText::new(subtitle)
                            .size(10.5)
                            .color(if is_selected {
                                TEXT_SECONDARY
                            } else {
                                TEXT_MUTED
                            }),
                    );
                });
            });
        });

    let interact = ui.interact(
        frame_resp.response.rect,
        ui.id().with(title),
        Sense::click(),
    );
    if interact.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    if interact.clicked() {
        clicked = true;
    }
    clicked
}

impl eframe::App for RustCleanerApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if !self.theme_initialized {
            configure_pro_max_theme(ctx);
            self.theme_initialized = true;
        }

        // Poll background cleaner results
        if let Ok(mut guard) = self.cleaner_result_rx.lock() {
            if let Some((summary, was_clean)) = guard.take() {
                self.cleaner_busy = false;
                if was_clean {
                    self.status_banner = format!(
                        "Pulizia completata: {} file eliminati ({}) — {} file in uso saltati in sicurezza.",
                        summary.total_files_deleted,
                        format_bytes(summary.total_bytes_deleted),
                        summary.total_skipped_locked
                    );
                } else {
                    self.status_banner = format!(
                        "Analisi completata: {} file rilevati ({}) — {} file attualmente in uso/bloccati.",
                        summary.total_files_found,
                        format_bytes(summary.total_bytes_found),
                        summary.total_skipped_locked
                    );
                }
                self.clean_summary = Some(summary);
            }
        }

        // Poll background Winget scan results
        if let Ok(mut guard) = self.winget_rx.lock() {
            if let Some(rep) = guard.take() {
                self.winget_busy = false;
                self.status_banner = rep.raw_message.clone();
                self.winget_report = Some(rep);
            }
        }
        let action_msg = self
            .winget_action_rx
            .lock()
            .ok()
            .and_then(|mut g| g.take());
        if let Some(msg) = action_msg {
            self.winget_busy = false;
            self.status_banner = msg;
            self.start_winget_scan();
        }

        // Poll background Driver scan results
        if let Ok(mut guard) = self.driver_rx.lock() {
            if let Some(rep) = guard.take() {
                self.driver_busy = false;
                self.status_banner = rep.status_message.clone();
                self.driver_report = Some(rep);
            }
        }

        let wiper_running = self
            .wipe_progress
            .lock()
            .map(|p| p.running)
            .unwrap_or(false);
        let monitor_active = self
            .monitor_state
            .lock()
            .map(|m| m.active)
            .unwrap_or(false);

        if self.cleaner_busy
            || wiper_running
            || self.winget_busy
            || self.driver_busy
            || monitor_active
        {
            ctx.request_repaint_after(std::time::Duration::from_millis(250));
        }

        // === TOP HEADER BAR ===
        egui::TopBottomPanel::top("top_header")
            .frame(
                egui::Frame::new()
                    .fill(BG_HEADER)
                    .stroke(Stroke::new(1.0, BORDER_SUBTLE))
                    .inner_margin(egui::Margin::symmetric(18, 10)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    // Vector Shield Brand Emblem
                    egui::Frame::new()
                        .fill(Color32::from_rgba_unmultiplied(249, 115, 22, 35))
                        .stroke(Stroke::new(1.5, ACCENT_RUST))
                        .corner_radius(CornerRadius::same(9))
                        .inner_margin(egui::Margin::same(7))
                        .show(ui, |ui| {
                            let (r, _) =
                                ui.allocate_exact_size(egui::vec2(22.0, 22.0), Sense::hover());
                            paint_vector_icon(
                                ui.painter(),
                                r,
                                VectorIcon::BrandShield,
                                ACCENT_RUST,
                            );
                        });

                    ui.add_space(4.0);
                    ui.vertical(|ui| {
                        ui.spacing_mut().item_spacing.y = 2.0;
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new("RustCleaner")
                                    .size(19.0)
                                    .strong()
                                    .color(TEXT_PRIMARY),
                            );
                            draw_pill_badge(
                                ui,
                                "MEGA ALPHA v1.0.0-alpha",
                                Color32::from_rgba_unmultiplied(249, 115, 22, 35),
                                ACCENT_RUST,
                            );
                            draw_pill_badge(
                                ui,
                                "ZERO TELEMETRIA • 0 MB RAM",
                                Color32::from_rgba_unmultiplied(34, 197, 94, 30),
                                ACCENT_EMERALD,
                            );
                        });
                        ui.label(
                            RichText::new(
                                "System Cleaner • Registry Guard • Winget Updater • Windows Update Drivers • Kernel Monitor",
                            )
                            .size(11.5)
                            .color(TEXT_SECONDARY),
                        );
                    });

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if self.is_admin {
                            draw_pill_badge(
                                ui,
                                "UAC: AMMINISTRATORE",
                                Color32::from_rgba_unmultiplied(34, 197, 94, 35),
                                ACCENT_EMERALD,
                            );
                        } else {
                            let uac_btn = egui::Button::new(
                                RichText::new("Eleva UAC (Admin)")
                                    .size(12.0)
                                    .strong()
                                    .color(Color32::from_rgb(15, 23, 42)),
                            )
                            .fill(ACCENT_AMBER)
                            .corner_radius(CornerRadius::same(14));

                            let resp = ui.add(uac_btn);
                            if resp.hovered() {
                                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                            }
                            if resp.clicked() {
                                match uac::relaunch_as_admin(&[]) {
                                    Ok(_) => std::process::exit(0),
                                    Err(e) => self.status_banner = e,
                                }
                            }
                            draw_pill_badge(
                                ui,
                                "MODO UTENTE STANDARD",
                                Color32::from_rgba_unmultiplied(245, 158, 11, 30),
                                ACCENT_AMBER,
                            );
                        }

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
                                                "{} Libero: {}",
                                                d.root_path,
                                                format_bytes(d.free_bytes)
                                            ))
                                            .size(11.0)
                                            .monospace()
                                            .color(TEXT_SECONDARY),
                                        );
                                        let (bar_rect, _) = ui.allocate_exact_size(
                                            egui::vec2(56.0, 6.0),
                                            Sense::hover(),
                                        );
                                        ui.painter().rect_filled(
                                            bar_rect,
                                            CornerRadius::same(3),
                                            Color32::from_rgb(35, 45, 62),
                                        );
                                        let mut fill_rect = bar_rect;
                                        fill_rect.set_width(
                                            bar_rect.width() * used_ratio.clamp(0.0, 1.0),
                                        );
                                        ui.painter().rect_filled(
                                            fill_rect,
                                            CornerRadius::same(3),
                                            if used_ratio > 0.85 {
                                                ACCENT_RED
                                            } else {
                                                ACCENT_CYAN
                                            },
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
                    .inner_margin(egui::Margin::symmetric(18, 7)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    let is_busy = self.cleaner_busy
                        || wiper_running
                        || self.winget_busy
                        || self.driver_busy;
                    let (dot_color, state_label) = if is_busy {
                        (ACCENT_AMBER, "IN ESECUZIONE")
                    } else {
                        (ACCENT_EMERALD, "PRONTO")
                    };
                    let (dot_rect, _) =
                        ui.allocate_exact_size(egui::vec2(8.0, 8.0), Sense::hover());
                    ui.painter()
                        .circle_filled(dot_rect.center(), 4.0, dot_color);
                    draw_pill_badge(
                        ui,
                        state_label,
                        Color32::from_rgba_unmultiplied(
                            dot_color.r(),
                            dot_color.g(),
                            dot_color.b(),
                            30,
                        ),
                        dot_color,
                    );
                    ui.label(
                        RichText::new(&self.status_banner)
                            .size(12.0)
                            .color(TEXT_SECONDARY),
                    );
                });
            });

        // === LEFT NAVIGATION SIDEBAR ===
        egui::SidePanel::left("left_sidebar")
            .resizable(false)
            .exact_width(254.0)
            .frame(
                egui::Frame::new()
                    .fill(BG_SIDEBAR)
                    .stroke(Stroke::new(1.0, BORDER_SUBTLE))
                    .inner_margin(egui::Margin::symmetric(12, 12)),
            )
            .show(ctx, |ui| {
                egui::ScrollArea::vertical()
                    .id_salt("sidebar_nav_scroll")
                    .show(ui, |ui| {
                        ui.label(
                            RichText::new("MANUTENZIONE & PULIZIA")
                                .size(10.5)
                                .strong()
                                .color(TEXT_MUTED),
                        );
                        ui.add_space(4.0);

                        let winget_cnt = self.winget_report.as_ref().map(|r| r.updates.len());
                        let driver_cnt = self
                            .driver_report
                            .as_ref()
                            .map(|r| r.wu_pending_drivers.len());
                        let reg_cnt = if self.reg_issues.is_empty() {
                            None
                        } else {
                            Some(self.reg_issues.len())
                        };

                        let nav_items = [
                            (
                                ActiveTab::Cleaner,
                                VectorIcon::CleanerSweep,
                                ACCENT_RUST,
                                "Pulizia Rapida",
                                "File temp, cache e browser",
                                None,
                            ),
                            (
                                ActiveTab::Registry,
                                VectorIcon::RegistryGrid,
                                ACCENT_AMBER,
                                "Registro Windows",
                                "Ripara chiavi + Backup .reg",
                                reg_cnt,
                            ),
                            (
                                ActiveTab::Startup,
                                VectorIcon::StartupBolt,
                                ACCENT_CYAN,
                                "Avvio Automatico",
                                "Gestisci app all'avvio",
                                Some(self.startup_entries.len()),
                            ),
                            (
                                ActiveTab::Uninstaller,
                                VectorIcon::UninstallerBox,
                                ACCENT_PURPLE,
                                "Disinstallazione",
                                "Rimuovi software installati",
                                Some(self.installed_apps.len()),
                            ),
                            (
                                ActiveTab::DriveWiper,
                                VectorIcon::DriveCylinder,
                                ACCENT_EMERALD,
                                "Bonifica Disco",
                                "Wipe spazio libero & MFT",
                                None,
                            ),
                            (
                                ActiveTab::SoftwareUpdater,
                                VectorIcon::SoftwareCloudUp,
                                ACCENT_CYAN,
                                "Software Updater",
                                "Winget nativo (Zero DB)",
                                winget_cnt,
                            ),
                            (
                                ActiveTab::DriverUpdater,
                                VectorIcon::DriverChip,
                                ACCENT_EMERALD,
                                "Driver Updater",
                                "Windows Update COM & WHQL",
                                driver_cnt,
                            ),
                            (
                                ActiveTab::AutomationMonitor,
                                VectorIcon::AutomationPulse,
                                ACCENT_PURPLE,
                                "Pianifica & Monitor",
                                "schtasks (0 RAM) + notify",
                                None,
                            ),
                        ];

                        for (idx, (tab, icon, color, title, subtitle, badge_cnt)) in
                            nav_items.iter().enumerate()
                        {
                            if idx == 5 {
                                ui.add_space(6.0);
                                ui.label(
                                    RichText::new("FUNZIONI PREMIUM FREE")
                                        .size(10.5)
                                        .strong()
                                        .color(ACCENT_CYAN),
                                );
                                ui.add_space(3.0);
                            }

                            let is_selected = self.active_tab == *tab;
                            if draw_sidebar_nav_item(
                                ui,
                                *icon,
                                *color,
                                title,
                                subtitle,
                                *badge_cnt,
                                is_selected,
                            ) {
                                self.active_tab = *tab;
                                if *tab == ActiveTab::SoftwareUpdater
                                    && self.winget_report.is_none()
                                    && !self.winget_busy
                                {
                                    self.start_winget_scan();
                                } else if *tab == ActiveTab::DriverUpdater
                                    && self.driver_report.is_none()
                                    && !self.driver_busy
                                {
                                    self.start_driver_scan(false);
                                }
                            }
                            ui.add_space(4.0);
                        }

                        ui.add_space(6.0);
                        egui::Frame::new()
                            .fill(BG_CARD)
                            .stroke(Stroke::new(1.0, BORDER_SUBTLE))
                            .corner_radius(CornerRadius::same(9))
                            .inner_margin(egui::Margin::symmetric(10, 8))
                            .show(ui, |ui| {
                                ui.set_width(ui.available_width());
                                ui.horizontal(|ui| {
                                    let (r, _) = ui
                                        .allocate_exact_size(egui::vec2(15.0, 15.0), Sense::hover());
                                    paint_vector_icon(
                                        ui.painter(),
                                        r,
                                        VectorIcon::CheckShield,
                                        ACCENT_EMERALD,
                                    );
                                    ui.label(
                                        RichText::new("100% Privacy & Zero Telemetria")
                                            .size(11.0)
                                            .color(ACCENT_EMERALD)
                                            .strong(),
                                    );
                                });
                                ui.label(
                                    RichText::new(
                                        "Nessun server terzo, nessun demone in RAM, nessun pop-up.",
                                    )
                                    .size(10.5)
                                    .color(TEXT_SECONDARY),
                                );
                            });
                    });
            });

        // === CENTRAL MAIN PANEL ===
        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(BG_CANVAS)
                    .inner_margin(egui::Margin::symmetric(18, 14)),
            )
            .show(ctx, |ui| match self.active_tab {
                ActiveTab::Cleaner => self.render_cleaner_tab(ui),
                ActiveTab::Registry => self.render_registry_tab(ui),
                ActiveTab::Startup => self.render_startup_tab(ui),
                ActiveTab::Uninstaller => self.render_uninstaller_tab(ui),
                ActiveTab::DriveWiper => self.render_wiper_tab(ui),
                ActiveTab::SoftwareUpdater => self.render_software_updater_tab(ui),
                ActiveTab::DriverUpdater => self.render_driver_updater_tab(ui),
                ActiveTab::AutomationMonitor => self.render_automation_monitor_tab(ui),
            });
    }
}

impl RustCleanerApp {
    fn render_cleaner_tab(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.label(
                    RichText::new("Pulizia del File System & Privacy Browser")
                        .size(19.0)
                        .strong()
                        .color(TEXT_PRIMARY),
                );
                ui.label(
                    RichText::new(
                        "Motore multi-thread (rayon + walkdir) con rilevamento automatico dei file bloccati o in uso.",
                    )
                    .size(12.0)
                    .color(TEXT_SECONDARY),
                );
            });

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if draw_action_button(
                    ui,
                    "Avvia Pulizia",
                    VectorIcon::PlayRun,
                    ButtonVariant::ExecuteBrand,
                    !self.cleaner_busy,
                ) {
                    self.start_cleaner_job(true);
                }

                if draw_action_button(
                    ui,
                    "Analizza Sistema",
                    VectorIcon::SearchLens,
                    ButtonVariant::PrimaryScan,
                    !self.cleaner_busy,
                ) {
                    self.start_cleaner_job(false);
                }
            });
        });

        ui.add_space(6.0);

        let (found_bytes, found_files, locked_files, deleted_bytes, deleted_files) =
            if let Some(s) = &self.clean_summary {
                (
                    format_bytes(s.total_bytes_found),
                    format!("{} file rilevati", s.total_files_found),
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
                VectorIcon::SearchLens,
            );
            draw_kpi_card(
                &mut cols[1],
                "FILE IN USO (PROTETTI)",
                &locked_files,
                "Saltati senza errori",
                ACCENT_AMBER,
                VectorIcon::BrandShield,
            );
            draw_kpi_card(
                &mut cols[2],
                "SPAZIO LIBERATO",
                &deleted_bytes,
                &deleted_files,
                ACCENT_EMERALD,
                VectorIcon::CheckShield,
            );
            let enabled_count = self.clean_rules.iter().filter(|r| r.enabled).count();
            draw_kpi_card(
                &mut cols[3],
                "REGOLE ATTIVE",
                &format!("{} / {}", enabled_count, self.clean_rules.len()),
                "Sistema, Explorer e Browser",
                ACCENT_PURPLE,
                VectorIcon::CleanerSweep,
            );
        });

        ui.add_space(6.0);

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
                        egui::vec2(ui.available_width(), 8.0),
                        Sense::hover(),
                    );
                    ui.painter().rect_filled(
                        bar_rect,
                        CornerRadius::same(4),
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
                    ui.horizontal_wrapped(|ui| {
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
                                    .size(11.0)
                                    .monospace()
                                    .color(TEXT_SECONDARY),
                            );
                            ui.add_space(8.0);
                        }
                    });
                });
                ui.add_space(6.0);
            }
        }

        let remaining_h = (ui.available_height() - 10.0).max(240.0);

        ui.columns(2, |cols| {
            card_frame().show(&mut cols[0], |ui| {
                ui.set_height(remaining_h);
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("Categorie di Pulizia")
                            .size(14.5)
                            .strong()
                            .color(TEXT_PRIMARY),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let none_btn = ui.small_button("Nessuna");
                        if none_btn.hovered() {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                        }
                        if none_btn.clicked() {
                            for r in &mut self.clean_rules {
                                r.enabled = false;
                            }
                        }
                        let all_btn = ui.small_button("Tutte");
                        if all_btn.hovered() {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                        }
                        if all_btn.clicked() {
                            for r in &mut self.clean_rules {
                                r.enabled = true;
                            }
                        }
                    });
                });

                ui.horizontal_wrapped(|ui| {
                    let filters = [
                        (None, "Tutte"),
                        (Some(RuleGroup::WindowsSystem), "Sistema"),
                        (Some(RuleGroup::WindowsExplorer), "Explorer"),
                        (Some(RuleGroup::BrowsersPrivacy), "Browser"),
                        (Some(RuleGroup::Applications), "App"),
                    ];
                    for (flt, lbl) in filters {
                        let active = self.selected_rule_group_filter == flt;
                        let btn = egui::Button::new(
                            RichText::new(lbl).size(11.5).strong().color(if active {
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
                        .corner_radius(CornerRadius::same(14));
                        let resp = ui.add(btn);
                        if resp.hovered() {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                        }
                        if resp.clicked() {
                            self.selected_rule_group_filter = flt;
                        }
                    }
                });

                ui.separator();

                egui::ScrollArea::vertical()
                    .id_salt("cleaner_rules_scroll")
                    .max_height((ui.available_height() - 6.0).max(160.0))
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
                                .inner_margin(egui::Margin::symmetric(10, 7))
                                .show(ui, |ui| {
                                    ui.set_width(ui.available_width());
                                    ui.horizontal(|ui| {
                                        let cb = ui.checkbox(&mut rule.enabled, "");
                                        if cb.hovered() {
                                            ui.ctx()
                                                .set_cursor_icon(egui::CursorIcon::PointingHand);
                                        }
                                        ui.label(
                                            RichText::new(&rule.name)
                                                .size(13.0)
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
                                            .size(11.0)
                                            .color(TEXT_MUTED),
                                    );
                                });
                            ui.add_space(4.0);
                        }
                    });
            });

            card_frame().show(&mut cols[1], |ui| {
                ui.set_height(remaining_h);
                ui.label(
                    RichText::new("Dettaglio Analisi e File Rilevati")
                        .size(14.5)
                        .strong()
                        .color(TEXT_PRIMARY),
                );
                ui.separator();

                if self.cleaner_busy {
                    ui.vertical_centered(|ui| {
                        ui.add_space(60.0);
                        ui.spinner();
                        ui.add_space(10.0);
                        ui.label(
                            RichText::new("Scansione parallela multi-thread in corso...")
                                .size(13.5)
                                .color(ACCENT_CYAN),
                        );
                    });
                } else if let Some(summary) = &self.clean_summary {
                    egui::ScrollArea::vertical()
                        .id_salt("cleaner_results_scroll")
                        .max_height((ui.available_height() - 6.0).max(160.0))
                        .show(ui, |ui| {
                            let non_empty: Vec<_> = summary
                                .results
                                .iter()
                                .filter(|r| !r.files.is_empty())
                                .collect();

                            if non_empty.is_empty() {
                                draw_empty_state(
                                    ui,
                                    VectorIcon::CheckShield,
                                    ACCENT_EMERALD,
                                    "Sistema Pulito",
                                    "Nessun file superfluo rilevato nelle categorie selezionate.",
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
                                    .inner_margin(egui::Margin::symmetric(10, 7))
                                    .show(ui, |ui| {
                                        ui.set_width(ui.available_width());
                                        ui.horizontal(|ui| {
                                            ui.label(
                                                RichText::new(&res.rule_name)
                                                    .size(13.0)
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
                                                        .size(11.0)
                                                        .monospace()
                                                        .color(TEXT_SECONDARY),
                                                    );
                                                },
                                            );
                                        });

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
                                                .size(11.0)
                                                .color(TEXT_MUTED),
                                            |ui| {
                                                for f in res.files.iter().take(50) {
                                                    ui.horizontal(|ui| {
                                                        if f.locked_or_denied {
                                                            draw_pill_badge(
                                                                ui,
                                                                "IN USO",
                                                                Color32::from_rgba_unmultiplied(
                                                                    245, 158, 11, 30,
                                                                ),
                                                                ACCENT_AMBER,
                                                            );
                                                        } else {
                                                            draw_pill_badge(
                                                                ui,
                                                                "PRONTO",
                                                                Color32::from_rgba_unmultiplied(
                                                                    34, 197, 94, 25,
                                                                ),
                                                                ACCENT_EMERALD,
                                                            );
                                                        }
                                                        let full = format!(
                                                            "{} ({})",
                                                            f.path.display(),
                                                            format_bytes(f.size_bytes)
                                                        );
                                                        ui.add(
                                                            egui::Label::new(
                                                                RichText::new(&full)
                                                                    .size(10.5)
                                                                    .monospace()
                                                                    .color(TEXT_SECONDARY),
                                                            )
                                                            .truncate(),
                                                        )
                                                        .on_hover_text(full);
                                                    });
                                                }
                                            },
                                        );
                                    });
                                ui.add_space(4.0);
                            }
                        });
                } else {
                    draw_empty_state(
                        ui,
                        VectorIcon::CleanerSweep,
                        ACCENT_CYAN,
                        "Pronto per l'Analisi del Sistema",
                        "Clicca su 'Analizza Sistema' in alto a destra per calcolare lo spazio recuperabile\nsenza eliminare alcun file.",
                    );
                }
            });
        });
    }

    fn start_cleaner_job(&mut self, delete_files: bool) {
        self.cleaner_busy = true;
        self.status_banner = if delete_files {
            "Pulizia parallela del File System in corso...".to_string()
        } else {
            "Analisi parallela del File System in corso...".to_string()
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
                    RichText::new("Manutenzione del Registro di Windows")
                        .size(19.0)
                        .strong()
                        .color(TEXT_PRIMARY),
                );
                ui.label(
                    RichText::new(
                        "Ricerca chiavi orfane con esportazione automatica di sicurezza .reg (Windows Registry Editor 5.00).",
                    )
                    .size(12.0)
                    .color(TEXT_SECONDARY),
                );
            });

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if draw_action_button(
                    ui,
                    "Backup .reg + Ripara",
                    VectorIcon::CheckShield,
                    ButtonVariant::ExecuteBrand,
                    !self.reg_issues.is_empty(),
                ) {
                    match backup_and_fix_registry(&self.reg_issues, &self.reg_backup_dir) {
                        Ok(report) => {
                            self.reg_last_backup = Some(report.backup_file.clone());
                            self.status_banner = format!(
                                "Backup salvato in '{}' — Riparate {} voci ({} errori).",
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

                if draw_action_button(
                    ui,
                    "Esporta .reg",
                    VectorIcon::DriveCylinder,
                    ButtonVariant::Secondary,
                    !self.reg_issues.is_empty(),
                ) {
                    match export_reg_backup(&self.reg_issues, &self.reg_backup_dir) {
                        Ok(path) => {
                            self.status_banner =
                                format!("Backup del Registro esportato in: {}", path.display());
                            self.reg_last_backup = Some(path);
                        }
                        Err(e) => self.status_banner = e,
                    }
                }

                if draw_action_button(
                    ui,
                    "Scansiona Registro",
                    VectorIcon::SearchLens,
                    ButtonVariant::PrimaryScan,
                    true,
                ) {
                    let active_cats: Vec<RegistryCategory> = self
                        .reg_categories
                        .iter()
                        .filter(|(_, en)| *en)
                        .map(|(c, _)| *c)
                        .collect();
                    self.reg_issues = scan_registry(&active_cats);
                    self.status_banner = format!(
                        "Scansione Registro completata: trovate {} incongruenze/chiavi orfane.",
                        self.reg_issues.len()
                    );
                }
            });
        });

        ui.add_space(8.0);

        card_frame().show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal_wrapped(|ui| {
                ui.label(
                    RichText::new("Categorie Hive:")
                        .size(12.0)
                        .strong()
                        .color(TEXT_SECONDARY),
                );
                for (cat, enabled) in &mut self.reg_categories {
                    let cb = ui.checkbox(enabled, cat.code());
                    if cb.hovered() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    }
                }
            });
            if let Some(last_bak) = &self.reg_last_backup {
                ui.add_space(4.0);
                ui.label(
                    RichText::new(format!(
                        "Ultimo file di ripristino .reg generato: {}",
                        last_bak.display()
                    ))
                    .size(11.5)
                    .monospace()
                    .color(ACCENT_EMERALD),
                );
            }
        });

        ui.add_space(8.0);

        let list_h = (ui.available_height() - 10.0).max(220.0);
        card_frame().show(ui, |ui| {
            ui.set_height(list_h);
            egui::ScrollArea::vertical()
                .id_salt("reg_issues_scroll")
                .max_height((ui.available_height() - 6.0).max(180.0))
                .show(ui, |ui| {
                    if self.reg_issues.is_empty() {
                        draw_empty_state(
                            ui,
                            VectorIcon::RegistryGrid,
                            ACCENT_AMBER,
                            "Nessuna Incongruenza Visualizzata",
                            "Clicca su 'Scansiona Registro' in alto a destra per cercare riferimenti orfani in HKCU, HKLM e HKCR.",
                        );
                    } else {
                        for issue in &mut self.reg_issues {
                            egui::Frame::new()
                                .fill(BG_CARD_SUBTLE)
                                .stroke(Stroke::new(1.0, BORDER_SUBTLE))
                                .corner_radius(CornerRadius::same(8))
                                .inner_margin(egui::Margin::symmetric(12, 8))
                                .show(ui, |ui| {
                                    ui.set_width(ui.available_width());
                                    ui.horizontal_wrapped(|ui| {
                                        let cb = ui.checkbox(&mut issue.selected, "");
                                        if cb.hovered() {
                                            ui.ctx()
                                                .set_cursor_icon(egui::CursorIcon::PointingHand);
                                        }
                                        draw_pill_badge(
                                            ui,
                                            issue.category.code(),
                                            Color32::from_rgba_unmultiplied(249, 115, 22, 35),
                                            ACCENT_RUST,
                                        );
                                        ui.label(
                                            RichText::new(&issue.problem_description)
                                                .size(13.0)
                                                .strong()
                                                .color(TEXT_PRIMARY),
                                        );
                                        ui.label(
                                            RichText::new(&issue.data_reference)
                                                .size(12.0)
                                                .monospace()
                                                .color(ACCENT_CYAN),
                                        );
                                    });
                                    let reg_p = issue.full_reg_path();
                                    ui.add(
                                        egui::Label::new(
                                            RichText::new(format!("Percorso: {reg_p}"))
                                                .size(11.0)
                                                .monospace()
                                                .color(TEXT_MUTED),
                                        )
                                        .truncate(),
                                    )
                                    .on_hover_text(reg_p);
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
                    RichText::new("Gestione dell'Avvio Automatico (Startup Manager)")
                        .size(19.0)
                        .strong()
                        .color(TEXT_PRIMARY),
                );
                ui.label(
                    RichText::new(
                        "Controlla le applicazioni che partono con Windows tramite Run, RunOnce e i flag binari StartupApproved.",
                    )
                    .size(12.0)
                    .color(TEXT_SECONDARY),
                );
            });

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if draw_action_button(
                    ui,
                    "Aggiorna Elenco",
                    VectorIcon::RefreshSync,
                    ButtonVariant::Secondary,
                    true,
                ) {
                    self.startup_entries = list_startup_entries();
                }
            });
        });

        ui.add_space(8.0);

        card_frame().show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                draw_pill_badge(
                    ui,
                    &format!("Totale: {}", self.startup_entries.len()),
                    Color32::from_rgba_unmultiplied(56, 189, 248, 30),
                    ACCENT_CYAN,
                );
                draw_pill_badge(
                    ui,
                    &format!("Attive: {active_count}"),
                    Color32::from_rgba_unmultiplied(34, 197, 94, 30),
                    ACCENT_EMERALD,
                );
                draw_pill_badge(
                    ui,
                    &format!("Disabilitate: {disabled_count}"),
                    Color32::from_rgba_unmultiplied(245, 158, 11, 30),
                    ACCENT_AMBER,
                );
                ui.separator();
                ui.label(RichText::new("Filtra:").color(TEXT_SECONDARY));
                ui.add(
                    egui::TextEdit::singleline(&mut self.startup_filter)
                        .hint_text("Cerca per nome o eseguibile...")
                        .desired_width(250.0),
                );
            });
        });

        ui.add_space(8.0);
        let filter_low = self.startup_filter.to_lowercase();
        let mut toggle_req: Option<(StartupEntry, bool)> = None;
        let mut delete_req: Option<StartupEntry> = None;

        let list_h = (ui.available_height() - 10.0).max(220.0);
        card_frame().show(ui, |ui| {
            ui.set_height(list_h);
            egui::ScrollArea::vertical()
                .id_salt("startup_scroll")
                .max_height((ui.available_height() - 6.0).max(180.0))
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
                            .inner_margin(egui::Margin::symmetric(12, 9))
                            .show(ui, |ui| {
                                ui.set_width(ui.available_width());
                                ui.horizontal(|ui| {
                                    if entry.enabled {
                                        draw_pill_badge(
                                            ui,
                                            "ATTIVO",
                                            Color32::from_rgba_unmultiplied(34, 197, 94, 35),
                                            ACCENT_EMERALD,
                                        );
                                    } else {
                                        draw_pill_badge(
                                            ui,
                                            "DISABILITATO",
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
                                            .size(14.0)
                                            .strong()
                                            .color(TEXT_PRIMARY),
                                    );

                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::Center),
                                        |ui| {
                                            if draw_action_button(
                                                ui,
                                                "Elimina",
                                                VectorIcon::TrashDelete,
                                                ButtonVariant::Danger,
                                                true,
                                            ) {
                                                delete_req = Some(entry.clone());
                                            }
                                            let (toggle_label, variant) = if entry.enabled {
                                                ("Disabilita", ButtonVariant::Secondary)
                                            } else {
                                                ("Abilita", ButtonVariant::PrimaryScan)
                                            };
                                            if draw_action_button(
                                                ui,
                                                toggle_label,
                                                VectorIcon::StartupBolt,
                                                variant,
                                                true,
                                            ) {
                                                toggle_req = Some((entry.clone(), !entry.enabled));
                                            }
                                        },
                                    );
                                });
                                ui.add_space(2.0);
                                let cmd_str = format!("Comando: {}", entry.command);
                                ui.add(
                                    egui::Label::new(
                                        RichText::new(&cmd_str)
                                            .size(11.0)
                                            .monospace()
                                            .color(TEXT_MUTED),
                                    )
                                    .truncate(),
                                )
                                .on_hover_text(&entry.command);
                            });
                        ui.add_space(4.0);
                    }
                });
        });

        if let Some((entry, new_state)) = toggle_req {
            match set_startup_enabled(&entry, new_state) {
                Ok(()) => {
                    self.status_banner = format!(
                        "Voce di avvio '{}' impostata su {}.",
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
                    self.status_banner = format!("Voce di avvio '{}' eliminata.", entry.name);
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
                    RichText::new("Disinstallazione Software (Uninstaller Manager)")
                        .size(19.0)
                        .strong()
                        .color(TEXT_PRIMARY),
                );
                ui.label(
                    RichText::new(
                        "Elenco completo delle applicazioni installate (64-bit, 32-bit WOW6432Node e Utente) con avvio diretto dell'uninstaller.",
                    )
                    .size(12.0)
                    .color(TEXT_SECONDARY),
                );
            });

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if draw_action_button(
                    ui,
                    "Aggiorna Lista",
                    VectorIcon::RefreshSync,
                    ButtonVariant::Secondary,
                    true,
                ) {
                    self.installed_apps = list_installed_apps();
                }
            });
        });

        ui.add_space(8.0);

        card_frame().show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                draw_pill_badge(
                    ui,
                    &format!("{} Applicazioni Rilevate", self.installed_apps.len()),
                    Color32::from_rgba_unmultiplied(168, 85, 247, 35),
                    ACCENT_PURPLE,
                );
                ui.separator();
                ui.label(RichText::new("Cerca:").color(TEXT_SECONDARY));
                ui.add(
                    egui::TextEdit::singleline(&mut self.uninstall_filter)
                        .hint_text("Filtra per nome software o produttore...")
                        .desired_width(280.0),
                );
            });
        });

        ui.add_space(8.0);
        let filter_low = self.uninstall_filter.to_lowercase();
        let mut launch_req: Option<InstalledApp> = None;

        let list_h = (ui.available_height() - 10.0).max(220.0);
        card_frame().show(ui, |ui| {
            ui.set_height(list_h);
            egui::ScrollArea::vertical()
                .id_salt("uninstaller_scroll")
                .max_height((ui.available_height() - 6.0).max(180.0))
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
                            .inner_margin(egui::Margin::symmetric(12, 9))
                            .show(ui, |ui| {
                                ui.set_width(ui.available_width());
                                ui.horizontal(|ui| {
                                    ui.vertical(|ui| {
                                        ui.horizontal_wrapped(|ui| {
                                            ui.label(
                                                RichText::new(&app.display_name)
                                                    .size(14.0)
                                                    .strong()
                                                    .color(TEXT_PRIMARY),
                                            );
                                            if !app.display_version.is_empty() {
                                                draw_pill_badge(
                                                    ui,
                                                    &format!("v{}", app.display_version),
                                                    Color32::from_rgba_unmultiplied(
                                                        56, 189, 248, 30,
                                                    ),
                                                    ACCENT_CYAN,
                                                );
                                            }
                                            if let Some(kb) = app.estimated_size_kb {
                                                draw_pill_badge(
                                                    ui,
                                                    &format!("{:.1} MB", kb as f64 / 1024.0),
                                                    Color32::from_rgba_unmultiplied(
                                                        34, 197, 94, 30,
                                                    ),
                                                    ACCENT_EMERALD,
                                                );
                                            }
                                            if !app.publisher.is_empty() {
                                                ui.label(
                                                    RichText::new(format!("• {}", app.publisher))
                                                        .size(11.5)
                                                        .color(TEXT_SECONDARY),
                                                );
                                            }
                                        });
                                        let u_cmd = format!("Comando: {}", app.uninstall_string);
                                        ui.add(
                                            egui::Label::new(
                                                RichText::new(&u_cmd)
                                                    .size(10.5)
                                                    .monospace()
                                                    .color(TEXT_MUTED),
                                            )
                                            .truncate(),
                                        )
                                        .on_hover_text(&app.uninstall_string);
                                    });

                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::Center),
                                        |ui| {
                                            if draw_action_button(
                                                ui,
                                                "Disinstalla",
                                                VectorIcon::UninstallerBox,
                                                ButtonVariant::Danger,
                                                true,
                                            ) {
                                                launch_req = Some(app.clone());
                                            }
                                        },
                                    );
                                });
                            });
                        ui.add_space(4.0);
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
            RichText::new("Bonifica dello Spazio Libero (Drive Wiper)")
                .size(19.0)
                .strong()
                .color(TEXT_PRIMARY),
        );
        ui.label(
            RichText::new(
                "Sovrascrittura sicura dello spazio non allocato con blocchi da 4 MB e flag nativo FILE_FLAG_WRITE_THROUGH (bypass cache OS).",
            )
            .size(12.0)
            .color(TEXT_SECONDARY),
        );
        ui.add_space(10.0);

        ui.columns(2, |cols| {
            card_frame().show(&mut cols[0], |ui| {
                ui.set_width(ui.available_width());
                ui.label(
                    RichText::new("1. Seleziona Volume Disco")
                        .size(14.5)
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
                            BG_CARD_HOVER
                        } else {
                            BG_CARD_SUBTLE
                        })
                        .stroke(Stroke::new(
                            1.2,
                            if is_sel { ACCENT_CYAN } else { BORDER_SUBTLE },
                        ))
                        .corner_radius(CornerRadius::same(8))
                        .inner_margin(egui::Margin::symmetric(12, 9))
                        .show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            ui.horizontal(|ui| {
                                let rb = ui.radio_value(
                                    &mut self.selected_drive_idx,
                                    idx,
                                    RichText::new(format!("Disco {}", d.root_path))
                                        .size(14.0)
                                        .strong(),
                                );
                                if rb.hovered() {
                                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                                }
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        draw_pill_badge(
                                            ui,
                                            &format!("Libero: {}", format_bytes(d.free_bytes)),
                                            Color32::from_rgba_unmultiplied(34, 197, 94, 35),
                                            ACCENT_EMERALD,
                                        );
                                    },
                                );
                            });

                            let (bar_rect, _) = ui.allocate_exact_size(
                                egui::vec2(ui.available_width(), 7.0),
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
                                .size(11.0)
                                .monospace()
                                .color(TEXT_MUTED),
                            );
                        });
                    ui.add_space(5.0);
                }
            });

            card_frame().show(&mut cols[1], |ui| {
                ui.set_width(ui.available_width());
                ui.label(
                    RichText::new("2. Algoritmo di Sicurezza & Opzioni")
                        .size(14.5)
                        .strong()
                        .color(TEXT_PRIMARY),
                );
                ui.separator();

                for &alg in WipeAlgorithm::all() {
                    let is_sel = self.wipe_algorithm == alg;
                    egui::Frame::new()
                        .fill(if is_sel {
                            BG_CARD_HOVER
                        } else {
                            BG_CARD_SUBTLE
                        })
                        .stroke(Stroke::new(
                            1.2,
                            if is_sel { ACCENT_RUST } else { BORDER_SUBTLE },
                        ))
                        .corner_radius(CornerRadius::same(8))
                        .inner_margin(egui::Margin::symmetric(10, 7))
                        .show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            let rb = ui.radio_value(
                                &mut self.wipe_algorithm,
                                alg,
                                RichText::new(alg.label()).size(13.0).strong(),
                            );
                            if rb.hovered() {
                                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                            }
                        });
                    ui.add_space(4.0);
                }

                ui.add_space(4.0);
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

        ui.add_space(10.0);
        let prog_snapshot = self.wipe_progress.lock().unwrap().clone();

        card_frame().show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                if draw_action_button(
                    ui,
                    "Avvia Bonifica Spazio Libero",
                    VectorIcon::DriveCylinder,
                    ButtonVariant::ExecuteBrand,
                    !prog_snapshot.running,
                ) {
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

                if prog_snapshot.running
                    && draw_action_button(
                        ui,
                        "Interrompi Subito e Rilascia Spazio",
                        VectorIcon::StopOctagon,
                        ButtonVariant::Danger,
                        true,
                    )
                {
                    self.wipe_cancel.store(true, Ordering::Relaxed);
                }
            });

            if prog_snapshot.running || prog_snapshot.finished {
                ui.add_space(8.0);
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

    // === 6. SOFTWARE UPDATER TAB (WINGET WRAPPER) ===
    fn start_winget_scan(&mut self) {
        self.winget_busy = true;
        self.status_banner =
            "Ricerca aggiornamenti software tramite Microsoft Winget in corso...".to_string();
        let rx = Arc::clone(&self.winget_rx);
        thread::spawn(move || {
            let rep = check_for_updates();
            if let Ok(mut guard) = rx.lock() {
                *guard = Some(rep);
            }
        });
    }

    fn render_software_updater_tab(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.label(
                    RichText::new("Software Updater (Microsoft Winget Nativo)")
                        .size(19.0)
                        .strong()
                        .color(TEXT_PRIMARY),
                );
                ui.label(
                    RichText::new(
                        "Nessun database proprietario: interroga direttamente il Windows Package Manager ufficiale (winget upgrade).",
                    )
                    .size(12.0)
                    .color(TEXT_SECONDARY),
                );
            });

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let has_updates = self
                    .winget_report
                    .as_ref()
                    .map(|r| !r.updates.is_empty())
                    .unwrap_or(false);

                if draw_action_button(
                    ui,
                    "Aggiorna Tutto (--all --silent)",
                    VectorIcon::SoftwareCloudUp,
                    ButtonVariant::ExecuteBrand,
                    !self.winget_busy && has_updates,
                ) {
                    self.winget_busy = true;
                    self.status_banner =
                        "Aggiornamento silenzioso di tutti i pacchetti Winget in corso (winget upgrade --all --silent)...".to_string();
                    let act_rx = Arc::clone(&self.winget_action_rx);
                    thread::spawn(move || {
                        let res = match upgrade_all_packages() {
                            Ok(m) => m,
                            Err(e) => e,
                        };
                        if let Ok(mut g) = act_rx.lock() {
                            *g = Some(res);
                        }
                    });
                }

                if draw_action_button(
                    ui,
                    "Cerca Aggiornamenti",
                    VectorIcon::SearchLens,
                    ButtonVariant::PrimaryScan,
                    !self.winget_busy,
                ) {
                    self.start_winget_scan();
                }
            });
        });

        ui.add_space(8.0);

        let (count_str, ver_str) = if let Some(rep) = &self.winget_report {
            (
                format!("{} software", rep.updates.len()),
                if rep.winget_version.is_empty() {
                    "Non trovato".to_string()
                } else {
                    rep.winget_version.clone()
                },
            )
        } else {
            ("—".to_string(), "Verifica...".to_string())
        };

        ui.columns(3, |cols| {
            draw_kpi_card(
                &mut cols[0],
                "AGGIORNAMENTI DISPONIBILI",
                &count_str,
                "Rilevati dal repository ufficiale",
                ACCENT_CYAN,
                VectorIcon::SoftwareCloudUp,
            );
            draw_kpi_card(
                &mut cols[1],
                "MOTORE WINGET NATIVO",
                &ver_str,
                "Windows Package Manager",
                ACCENT_EMERALD,
                VectorIcon::CheckShield,
            );
            draw_kpi_card(
                &mut cols[2],
                "SERVER PROPRIETARI",
                "0 (100% Free)",
                "Zero abbonamenti • Zero telemetria",
                ACCENT_PURPLE,
                VectorIcon::BrandShield,
            );
        });

        ui.add_space(8.0);

        let list_h = (ui.available_height() - 10.0).max(220.0);
        card_frame().show(ui, |ui| {
            ui.set_height(list_h);
            ui.horizontal(|ui| {
                ui.label(RichText::new("Filtra pacchetti:").color(TEXT_SECONDARY));
                ui.add(
                    egui::TextEdit::singleline(&mut self.winget_filter)
                        .hint_text("Cerca per nome o ID pacchetto...")
                        .desired_width(280.0),
                );
            });
            ui.separator();

            if self.winget_busy {
                ui.vertical_centered(|ui| {
                    ui.add_space(60.0);
                    ui.spinner();
                    ui.add_space(10.0);
                    ui.label(
                        RichText::new("Interrogazione di Microsoft Winget in background...")
                            .size(14.0)
                            .color(ACCENT_CYAN),
                    );
                });
            } else if let Some(rep) = &self.winget_report {
                let filter_low = self.winget_filter.to_lowercase();
                let mut single_upgrade_id: Option<String> = None;

                egui::ScrollArea::vertical()
                    .id_salt("winget_updates_scroll")
                    .max_height((ui.available_height() - 6.0).max(160.0))
                    .show(ui, |ui| {
                        if rep.updates.is_empty() {
                            draw_empty_state(
                                ui,
                                VectorIcon::CheckShield,
                                ACCENT_EMERALD,
                                "Tutti i Software sono Aggiornati",
                                &rep.raw_message,
                            );
                        } else {
                            for pkg in &rep.updates {
                                if !filter_low.is_empty()
                                    && !pkg.name.to_lowercase().contains(&filter_low)
                                    && !pkg.id.to_lowercase().contains(&filter_low)
                                {
                                    continue;
                                }
                                egui::Frame::new()
                                    .fill(BG_CARD_SUBTLE)
                                    .stroke(Stroke::new(1.0, BORDER_SUBTLE))
                                    .corner_radius(CornerRadius::same(8))
                                    .inner_margin(egui::Margin::symmetric(12, 9))
                                    .show(ui, |ui| {
                                        ui.set_width(ui.available_width());
                                        ui.horizontal(|ui| {
                                            ui.vertical(|ui| {
                                                ui.horizontal_wrapped(|ui| {
                                                    ui.label(
                                                        RichText::new(&pkg.name)
                                                            .size(14.0)
                                                            .strong()
                                                            .color(TEXT_PRIMARY),
                                                    );
                                                    draw_pill_badge(
                                                        ui,
                                                        &format!("Attuale: {}", pkg.current_version),
                                                        Color32::from_rgba_unmultiplied(
                                                            245, 158, 11, 30,
                                                        ),
                                                        ACCENT_AMBER,
                                                    );
                                                    ui.label(
                                                        RichText::new("->")
                                                            .monospace()
                                                            .color(TEXT_MUTED),
                                                    );
                                                    draw_pill_badge(
                                                        ui,
                                                        &format!("Nuova: {}", pkg.available_version),
                                                        Color32::from_rgba_unmultiplied(
                                                            34, 197, 94, 35,
                                                        ),
                                                        ACCENT_EMERALD,
                                                    );
                                                });
                                                ui.label(
                                                    RichText::new(format!(
                                                        "ID Winget: {} • Sorgente: {}",
                                                        pkg.id, pkg.source
                                                    ))
                                                    .size(10.5)
                                                    .monospace()
                                                    .color(TEXT_MUTED),
                                                );
                                            });

                                            ui.with_layout(
                                                egui::Layout::right_to_left(egui::Align::Center),
                                                |ui| {
                                                    if draw_action_button(
                                                        ui,
                                                        "Aggiorna",
                                                        VectorIcon::SoftwareCloudUp,
                                                        ButtonVariant::ExecuteBrand,
                                                        true,
                                                    ) {
                                                        single_upgrade_id = Some(pkg.id.clone());
                                                    }
                                                },
                                            );
                                        });
                                    });
                                ui.add_space(4.0);
                            }
                        }
                    });

                if let Some(pkg_id) = single_upgrade_id {
                    self.winget_busy = true;
                    self.status_banner = format!(
                        "Aggiornamento silenzioso di '{pkg_id}' tramite Winget in corso..."
                    );
                    let act_rx = Arc::clone(&self.winget_action_rx);
                    thread::spawn(move || {
                        let res = match upgrade_single_package(&pkg_id) {
                            Ok(m) => m,
                            Err(e) => e,
                        };
                        if let Ok(mut g) = act_rx.lock() {
                            *g = Some(res);
                        }
                    });
                }
            }
        });
    }

    // === 7. DRIVER UPDATER TAB (WINDOWS UPDATE COM WUApiLib + PnP) ===
    fn start_driver_scan(&mut self, online_wu: bool) {
        self.driver_busy = true;
        self.status_banner = if online_wu {
            "Interrogazione COM Microsoft.Update.Session (IsInstalled=0 and Type='Driver') sui server Windows Update...".to_string()
        } else {
            "Lettura dei driver hardware PnP firmati (WHQL) installati nel sistema...".to_string()
        };
        let rx = Arc::clone(&self.driver_rx);
        thread::spawn(move || {
            let rep = scan_drivers(online_wu);
            if let Ok(mut g) = rx.lock() {
                *g = Some(rep);
            }
        });
    }

    fn render_driver_updater_tab(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.label(
                    RichText::new("Driver Updater Sicuro (Windows Update Agent COM)")
                        .size(19.0)
                        .strong()
                        .color(TEXT_PRIMARY),
                );
                ui.label(
                    RichText::new(
                        "Zero database di terze parti: usa esclusivamente WUApiLib (Microsoft.Update.Session) per driver certificati WHQL.",
                    )
                    .size(12.0)
                    .color(TEXT_SECONDARY),
                );
            });

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let pending_count = self
                    .driver_report
                    .as_ref()
                    .map(|r| r.wu_pending_drivers.len())
                    .unwrap_or(0);

                if draw_action_button(
                    ui,
                    "Installa Driver WU",
                    VectorIcon::DriverChip,
                    ButtonVariant::ExecuteBrand,
                    !self.driver_busy && pending_count > 0,
                ) {
                    match install_wu_driver_updates() {
                        Ok(msg) => {
                            self.status_banner = msg;
                            self.start_driver_scan(true);
                        }
                        Err(e) => self.status_banner = e,
                    }
                }

                if draw_action_button(
                    ui,
                    "Cerca su Windows Update (WUApiLib)",
                    VectorIcon::SearchLens,
                    ButtonVariant::PrimaryScan,
                    !self.driver_busy,
                ) {
                    self.start_driver_scan(true);
                }
            });
        });

        ui.add_space(8.0);

        let (wu_count, pnp_count, signed_count) = if let Some(rep) = &self.driver_report {
            let signed = rep.installed_drivers.iter().filter(|d| d.is_signed).count();
            (
                format!("{} da installare", rep.wu_pending_drivers.len()),
                format!("{} dispositivi", rep.installed_drivers.len()),
                format!("{signed} / {} WHQL", rep.installed_drivers.len()),
            )
        } else {
            ("—".to_string(), "—".to_string(), "—".to_string())
        };

        ui.columns(3, |cols| {
            draw_kpi_card(
                &mut cols[0],
                "DRIVER WINDOWS UPDATE",
                &wu_count,
                "Query: IsInstalled=0 and Type='Driver'",
                ACCENT_CYAN,
                VectorIcon::SoftwareCloudUp,
            );
            draw_kpi_card(
                &mut cols[1],
                "DRIVER HARDWARE RILEVATI",
                &pnp_count,
                "GPU, Rete, Audio, NVMe, USB, Chipset",
                ACCENT_PURPLE,
                VectorIcon::DriverChip,
            );
            draw_kpi_card(
                &mut cols[2],
                "CERTIFICAZIONE MICROSOFT",
                &signed_count,
                "Zero rischio BSOD da DB esterni",
                ACCENT_EMERALD,
                VectorIcon::CheckShield,
            );
        });

        ui.add_space(8.0);

        let list_h = (ui.available_height() - 10.0).max(220.0);
        card_frame().show(ui, |ui| {
            ui.set_height(list_h);
            if self.driver_busy {
                ui.vertical_centered(|ui| {
                    ui.add_space(60.0);
                    ui.spinner();
                    ui.add_space(10.0);
                    ui.label(
                        RichText::new("Scansione COM WUApiLib / Driver Hardware PnP in corso...")
                            .size(14.0)
                            .color(ACCENT_CYAN),
                    );
                });
            } else if let Some(rep) = &self.driver_report {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Filtra driver:").color(TEXT_SECONDARY));
                    ui.add(
                        egui::TextEdit::singleline(&mut self.driver_filter)
                            .hint_text("Cerca per scheda video, rete, produttore...")
                            .desired_width(280.0),
                    );
                });
                ui.separator();

                let filter_low = self.driver_filter.to_lowercase();
                egui::ScrollArea::vertical()
                    .id_salt("drivers_scroll")
                    .max_height((ui.available_height() - 6.0).max(160.0))
                    .show(ui, |ui| {
                        if !rep.wu_pending_drivers.is_empty() {
                            ui.label(
                                RichText::new(
                                    "Aggiornamenti Driver Ufficiali Disponibili su Windows Update:",
                                )
                                .size(13.5)
                                .strong()
                                .color(ACCENT_AMBER),
                            );
                            for wu in &rep.wu_pending_drivers {
                                egui::Frame::new()
                                    .fill(Color32::from_rgb(38, 32, 24))
                                    .stroke(Stroke::new(1.0, ACCENT_AMBER))
                                    .corner_radius(CornerRadius::same(8))
                                    .inner_margin(egui::Margin::symmetric(12, 8))
                                    .show(ui, |ui| {
                                        ui.set_width(ui.available_width());
                                        ui.horizontal_wrapped(|ui| {
                                            draw_pill_badge(
                                                ui,
                                                "NUOVO DRIVER WU",
                                                Color32::from_rgba_unmultiplied(245, 158, 11, 40),
                                                ACCENT_AMBER,
                                            );
                                            ui.label(
                                                RichText::new(&wu.title)
                                                    .size(13.5)
                                                    .strong()
                                                    .color(TEXT_PRIMARY),
                                            );
                                            draw_pill_badge(
                                                ui,
                                                &format!("{:.2} MB", wu.size_mb),
                                                Color32::from_rgba_unmultiplied(56, 189, 248, 30),
                                                ACCENT_CYAN,
                                            );
                                        });
                                        ui.label(
                                            RichText::new(format!(
                                                "Produttore: {} • Classe: {} • Data: {}",
                                                wu.driver_manufacturer,
                                                wu.driver_class,
                                                wu.driver_ver_date
                                            ))
                                            .size(11.0)
                                            .monospace()
                                            .color(TEXT_SECONDARY),
                                        );
                                    });
                                ui.add_space(4.0);
                            }
                            ui.separator();
                        }

                        ui.label(
                            RichText::new("Driver Hardware PnP Installati nel Sistema:")
                                .size(13.0)
                                .strong()
                                .color(TEXT_SECONDARY),
                        );
                        ui.add_space(4.0);

                        for drv in &rep.installed_drivers {
                            if !filter_low.is_empty()
                                && !drv.device_name.to_lowercase().contains(&filter_low)
                                && !drv.manufacturer.to_lowercase().contains(&filter_low)
                                && !drv.device_class.to_lowercase().contains(&filter_low)
                            {
                                continue;
                            }
                            egui::Frame::new()
                                .fill(BG_CARD_SUBTLE)
                                .stroke(Stroke::new(1.0, BORDER_SUBTLE))
                                .corner_radius(CornerRadius::same(8))
                                .inner_margin(egui::Margin::symmetric(12, 8))
                                .show(ui, |ui| {
                                    ui.set_width(ui.available_width());
                                    ui.horizontal_wrapped(|ui| {
                                        draw_pill_badge(
                                            ui,
                                            &drv.device_class,
                                            Color32::from_rgba_unmultiplied(56, 189, 248, 25),
                                            ACCENT_CYAN,
                                        );
                                        ui.label(
                                            RichText::new(&drv.device_name)
                                                .size(13.0)
                                                .strong()
                                                .color(TEXT_PRIMARY),
                                        );
                                        draw_pill_badge(
                                            ui,
                                            &format!("v{}", drv.driver_version),
                                            Color32::from_rgba_unmultiplied(168, 85, 247, 30),
                                            ACCENT_PURPLE,
                                        );
                                        if drv.is_signed {
                                            draw_pill_badge(
                                                ui,
                                                "Firmato WHQL",
                                                Color32::from_rgba_unmultiplied(34, 197, 94, 30),
                                                ACCENT_EMERALD,
                                            );
                                        }
                                    });
                                    ui.label(
                                        RichText::new(format!(
                                            "Produttore: {} • Data Driver: {} • Firmatario: {}",
                                            drv.manufacturer, drv.driver_date, drv.signer
                                        ))
                                        .size(10.5)
                                        .monospace()
                                        .color(TEXT_MUTED),
                                    );
                                });
                            ui.add_space(4.0);
                        }
                    });
            }
        });
    }

    // === 8. AUTOMATION (ZERO-RAM SCHTASKS) & KERNEL MONITOR (NOTIFY) TAB ===
    fn render_automation_monitor_tab(&mut self, ui: &mut egui::Ui) {
        ui.label(
            RichText::new("Pulizia Programmata (0 RAM) & Monitoraggio Kernel (Zero Polling)")
                .size(19.0)
                .strong()
                .color(TEXT_PRIMARY),
        );
        ui.label(
            RichText::new(
                "Usa l'Utilità di Pianificazione di Windows (schtasks.exe --silent-clean) e le API kernel ReadDirectoryChangesW (crate notify).",
            )
            .size(12.0)
            .color(TEXT_SECONDARY),
        );
        ui.add_space(8.0);

        let col_h = (ui.available_height() - 10.0).max(320.0);

        ui.columns(2, |cols| {
            // LEFT CARD: Windows Task Scheduler (`schtasks.exe`)
            card_frame().show(&mut cols[0], |ui| {
                ui.set_height(col_h);
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("1. Pulizia Programmata (schtasks.exe)")
                            .size(14.5)
                            .strong()
                            .color(TEXT_PRIMARY),
                    );
                    draw_pill_badge(
                        ui,
                        "0 MB RAM A RIPOSO",
                        Color32::from_rgba_unmultiplied(34, 197, 94, 35),
                        ACCENT_EMERALD,
                    );
                });
                ui.separator();

                egui::Frame::new()
                    .fill(BG_CARD_SUBTLE)
                    .stroke(Stroke::new(1.0, BORDER_SUBTLE))
                    .corner_radius(CornerRadius::same(8))
                    .inner_margin(egui::Margin::symmetric(12, 8))
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("Stato Task:").color(TEXT_SECONDARY));
                            if self.schedule_status.exists {
                                draw_pill_badge(
                                    ui,
                                    "PIANIFICAZIONE ATTIVA",
                                    Color32::from_rgba_unmultiplied(34, 197, 94, 35),
                                    ACCENT_EMERALD,
                                );
                            } else {
                                draw_pill_badge(
                                    ui,
                                    "NON CONFIGURATA",
                                    Color32::from_rgba_unmultiplied(245, 158, 11, 30),
                                    ACCENT_AMBER,
                                );
                            }
                        });
                        if self.schedule_status.exists {
                            ui.label(
                                RichText::new(format!(
                                    "Prossima esecuzione: {}",
                                    self.schedule_status.next_run_time
                                ))
                                .size(11.5)
                                .monospace()
                                .color(ACCENT_CYAN),
                            );
                        }
                    });

                ui.add_space(6.0);
                ui.label(
                    RichText::new("Frequenza di Esecuzione:")
                        .size(12.5)
                        .strong()
                        .color(TEXT_SECONDARY),
                );
                for freq in [
                    ScheduleFrequency::Daily,
                    ScheduleFrequency::Weekly,
                    ScheduleFrequency::OnLogon,
                ] {
                    let rb = ui.radio_value(&mut self.schedule_freq, freq, freq.label());
                    if rb.hovered() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    }
                }

                if self.schedule_freq != ScheduleFrequency::OnLogon {
                    ui.horizontal(|ui| {
                        ui.label("Orario (HH:MM):");
                        ui.add(
                            egui::TextEdit::singleline(&mut self.schedule_time_hhmm)
                                .desired_width(70.0),
                        );
                    });
                }

                ui.checkbox(
                    &mut self.schedule_highest_uac,
                    "Esegui con privilegi elevati (/RL HIGHEST — richiede UAC)",
                );

                ui.add_space(6.0);
                ui.horizontal_wrapped(|ui| {
                    if draw_action_button(
                        ui,
                        "Salva in Task Scheduler",
                        VectorIcon::CheckShield,
                        ButtonVariant::ExecuteBrand,
                        true,
                    ) {
                        match create_scheduled_task(
                            self.schedule_freq,
                            &self.schedule_time_hhmm,
                            self.schedule_highest_uac,
                        ) {
                            Ok(msg) => {
                                self.status_banner = msg;
                                self.schedule_status = query_scheduled_task();
                            }
                            Err(e) => self.status_banner = e,
                        }
                    }

                    if draw_action_button(
                        ui,
                        "Test --silent-clean",
                        VectorIcon::PlayRun,
                        ButtonVariant::Secondary,
                        true,
                    ) {
                        match run_silent_clean_and_log() {
                            Ok(msg) => {
                                self.status_banner = msg;
                                self.schedule_logs = read_scheduled_logs();
                            }
                            Err(e) => self.status_banner = e,
                        }
                    }

                    if self.schedule_status.exists
                        && draw_action_button(
                            ui,
                            "Rimuovi Task",
                            VectorIcon::TrashDelete,
                            ButtonVariant::Danger,
                            true,
                        )
                    {
                        match delete_scheduled_task() {
                            Ok(msg) => {
                                self.status_banner = msg;
                                self.schedule_status = query_scheduled_task();
                            }
                            Err(e) => self.status_banner = e,
                        }
                    }
                });

                ui.add_space(6.0);
                ui.label(
                    RichText::new("Ultime esecuzioni silenziose (rustcleaner_scheduled.log):")
                        .size(11.5)
                        .strong()
                        .color(TEXT_SECONDARY),
                );
                egui::ScrollArea::vertical()
                    .id_salt("sched_log_scroll")
                    .max_height((ui.available_height() - 6.0).max(80.0))
                    .show(ui, |ui| {
                        if self.schedule_logs.is_empty() {
                            ui.label(
                                RichText::new(
                                    "Nessun log registrato. Clicca su 'Test --silent-clean' per provare.",
                                )
                                .size(11.0)
                                .color(TEXT_MUTED),
                            );
                        } else {
                            for line in &self.schedule_logs {
                                ui.label(
                                    RichText::new(line)
                                        .size(10.5)
                                        .monospace()
                                        .color(ACCENT_EMERALD),
                                );
                            }
                        }
                    });
            });

            // RIGHT CARD: Real-Time Kernel Event Watcher (`notify` / `ReadDirectoryChangesW`)
            card_frame().show(&mut cols[1], |ui| {
                ui.set_height(col_h);
                let mon_snap = self.monitor_state.lock().unwrap().clone();

                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("2. Monitoraggio Kernel (ReadDirectoryChangesW)")
                            .size(14.5)
                            .strong()
                            .color(TEXT_PRIMARY),
                    );
                    draw_pill_badge(
                        ui,
                        "0% CPU POLLING",
                        Color32::from_rgba_unmultiplied(56, 189, 248, 35),
                        ACCENT_CYAN,
                    );
                });
                ui.separator();

                ui.horizontal(|ui| {
                    if !mon_snap.active {
                        if draw_action_button(
                            ui,
                            "Avvia Monitor Kernel",
                            VectorIcon::AutomationPulse,
                            ButtonVariant::PrimaryScan,
                            true,
                        ) {
                            self.monitor_stop_flag.store(false, Ordering::Relaxed);
                            start_realtime_watcher(
                                Arc::clone(&self.monitor_state),
                                Arc::clone(&self.monitor_stop_flag),
                            );
                            self.status_banner = "Monitoraggio Kernel ReadDirectoryChangesW avviato (0% CPU in attesa di eventi dal file system).".to_string();
                        }
                    } else if draw_action_button(
                        ui,
                        "Ferma Monitor Kernel",
                        VectorIcon::StopOctagon,
                        ButtonVariant::Danger,
                        true,
                    ) {
                        self.monitor_stop_flag.store(true, Ordering::Relaxed);
                        self.status_banner =
                            "Monitoraggio Kernel ReadDirectoryChangesW fermato.".to_string();
                    }

                    draw_pill_badge(
                        ui,
                        &format!("Eventi Kernel: {}", mon_snap.total_events_received),
                        Color32::from_rgba_unmultiplied(168, 85, 247, 30),
                        ACCENT_PURPLE,
                    );
                });

                ui.add_space(4.0);
                if let Ok(mut st) = self.monitor_state.lock() {
                    ui.checkbox(
                        &mut st.auto_clean_on_browser_close,
                        "Pulisci automaticamente la cache alla chiusura di Chrome / Edge / Brave",
                    );
                }

                ui.label(
                    RichText::new("Percorsi monitorati a livello Kernel:")
                        .size(11.5)
                        .strong()
                        .color(TEXT_SECONDARY),
                );
                for (lbl, p) in &mon_snap.watched_paths {
                    let wp = format!("  • {lbl}: {}", p.display());
                    ui.add(
                        egui::Label::new(
                            RichText::new(&wp)
                                .size(10.5)
                                .monospace()
                                .color(TEXT_MUTED),
                        )
                        .truncate(),
                    )
                    .on_hover_text(p.display().to_string());
                }

                if let Some(last_auto) = &mon_snap.last_auto_clean_summary {
                    ui.label(
                        RichText::new(last_auto)
                            .size(11.0)
                            .color(ACCENT_EMERALD),
                    );
                }

                ui.separator();
                ui.label(
                    RichText::new("Feed Eventi Kernel in Tempo Reale (ReadDirectoryChangesW):")
                        .size(12.0)
                        .strong()
                        .color(TEXT_PRIMARY),
                );

                egui::ScrollArea::vertical()
                    .id_salt("kernel_events_scroll")
                    .max_height((ui.available_height() - 6.0).max(100.0))
                    .show(ui, |ui| {
                        if mon_snap.events.is_empty() {
                            draw_empty_state(
                                ui,
                                VectorIcon::AutomationPulse,
                                ACCENT_PURPLE,
                                "In Attesa di Eventi I/O dal Kernel",
                                "Avvia il monitor per osservare in diretta le modifiche a %TEMP% e alle sessioni dei browser.",
                            );
                        } else {
                            for ev in &mon_snap.events {
                                ui.horizontal_wrapped(|ui| {
                                    ui.label(
                                        RichText::new(&ev.timestamp)
                                            .size(10.5)
                                            .monospace()
                                            .color(TEXT_MUTED),
                                    );
                                    draw_pill_badge(
                                        ui,
                                        &ev.event_type,
                                        Color32::from_rgba_unmultiplied(56, 189, 248, 25),
                                        ACCENT_CYAN,
                                    );
                                    ui.label(
                                        RichText::new(&ev.source_label)
                                            .size(11.0)
                                            .strong()
                                            .color(TEXT_PRIMARY),
                                    );
                                    ui.add(
                                        egui::Label::new(
                                            RichText::new(&ev.path)
                                                .size(10.5)
                                                .monospace()
                                                .color(TEXT_SECONDARY),
                                        )
                                        .truncate(),
                                    )
                                    .on_hover_text(&ev.path);
                                });
                            }
                        }
                    });
            });
        });
    }
}
