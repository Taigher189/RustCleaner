//! UI/UX Pro Max Desktop GUI for RustCleaner built with `eframe` / `egui`.
//!
//! Features:
//! - Live **Dark / Light Mode** Switcher (`ThemeMode::Dark` / `ThemeMode::Light`) with semantic
//!   token-driven theming (`ThemePalette`) and WCAG AA/AAA contrast in both modes.
//! - Live **7-Language Switcher** (`Italiano`, `English`, `Français`, `Español`, `Deutsch`, `中文`, `Português`)
//!   with native Windows CJK font fallback (`msyh.ttc`) so Chinese and European glyphs render crisply.
//! - 100% custom resolution-independent `egui::Painter` vector icons (`1.6px` stroke, zero emoji dependency,
//!   dedicated bounding boxes with zero text overlap).

use crate::cleaner::{
    analyze_all, default_clean_rules, execute_clean, format_bytes, CleanRule, CleanSummary,
    RuleGroup, ScannedFileItem,
};
use crate::disk_analyzer::{
    delete_analyzed_file, resolve_preset_paths, reveal_in_explorer, run_disk_analysis,
    DiskAnalysisReport, FileCategory, ScanTargetPreset,
};
use crate::driver_updater::{install_wu_driver_updates, scan_drivers, DriverScanReport};
use crate::i18n::Language;
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
// UI/UX LAYOUT CONSTANTS & RESPONSIVE BREAKPOINTS
// ============================================================================
/// Minimum height for Bento KPI summary cards to guarantee visual parity across all 7 languages.
pub const MIN_KPI_CARD_HEIGHT: f32 = 78.0;

/// Screen width breakpoint below which the header subtitle collapses to preserve horizontal space.
pub const HEADER_SUBTITLE_BREAKPOINT: f32 = 1150.0;

/// Screen width breakpoint below which the zero-telemetry badge collapses to prevent clipping.
pub const HEADER_BADGE_BREAKPOINT: f32 = 1050.0;

/// Minimum supported inner window width for RustCleaner desktop GUI (1020px).
pub const MIN_WINDOW_WIDTH: f32 = 1020.0;

/// Minimum supported inner window height for RustCleaner desktop GUI (640px).
pub const MIN_WINDOW_HEIGHT: f32 = 640.0;

/// Default launch inner window width (1220px).
pub const DEFAULT_WINDOW_WIDTH: f32 = 1220.0;

/// Default launch inner window height (750px).
pub const DEFAULT_WINDOW_HEIGHT: f32 = 750.0;

/// Returns true if the screen width triggers the compact header mode (collapsing subtitle).
#[inline]
pub fn is_compact_header(screen_w: f32) -> bool {
    screen_w < HEADER_SUBTITLE_BREAKPOINT
}

/// Returns true if the screen width triggers the ultra-compact header mode (collapsing telemetry badge).
#[inline]
pub fn is_ultra_compact(screen_w: f32) -> bool {
    screen_w < HEADER_BADGE_BREAKPOINT
}

// ============================================================================
// UI/UX PRO MAX — DUAL-THEME SEMANTIC PALETTE (Dark OLED & Crisp Light Mode)
// ============================================================================
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeMode {
    Dark,
    Light,
}

#[derive(Debug, Clone, Copy)]
pub struct ThemePalette {
    pub is_dark: bool,
    pub bg_canvas: Color32,
    pub bg_sidebar: Color32,
    pub bg_header: Color32,
    pub bg_card: Color32,
    pub bg_card_subtle: Color32,
    pub bg_card_hover: Color32,
    pub bg_track: Color32,
    pub border_subtle: Color32,
    pub border_strong: Color32,
    pub text_primary: Color32,
    pub text_secondary: Color32,
    pub text_muted: Color32,
    pub accent_rust: Color32,
    pub accent_emerald: Color32,
    pub accent_cyan: Color32,
    pub accent_amber: Color32,
    pub accent_purple: Color32,
    pub accent_red: Color32,
}

impl ThemePalette {
    pub fn for_mode(mode: ThemeMode) -> Self {
        match mode {
            ThemeMode::Dark => Self {
                is_dark: true,
                bg_canvas: Color32::from_rgb(11, 15, 25),       // #0B0F19 Deep Slate Canvas
                bg_sidebar: Color32::from_rgb(15, 23, 42),      // #0F172A Slate 900 Sidebar
                bg_header: Color32::from_rgb(13, 19, 34),       // #0D1322 Top/Bottom Chrome
                bg_card: Color32::from_rgb(22, 31, 51),         // #161F33 Elevated Card Surface
                bg_card_subtle: Color32::from_rgb(17, 24, 39),  // #111827 Recessed Row Surface
                bg_card_hover: Color32::from_rgb(30, 41, 59),   // #1E293B Interactive Hover
                bg_track: Color32::from_rgb(35, 45, 62),        // Progress Track
                border_subtle: Color32::from_rgb(45, 58, 79),   // #2D3A4F 1px Separator
                border_strong: Color32::from_rgb(71, 85, 105),  // #475569 Active Border
                text_primary: Color32::from_rgb(248, 250, 252), // #F8FAFC (>15:1 contrast)
                text_secondary: Color32::from_rgb(203, 213, 225), // #CBD5E1 (>10:1 contrast)
                text_muted: Color32::from_rgb(148, 163, 184),   // #94A3B8 (>5.5:1 WCAG AA)
                accent_rust: Color32::from_rgb(249, 115, 22),   // #F97316
                accent_emerald: Color32::from_rgb(34, 197, 94), // #22C55E
                accent_cyan: Color32::from_rgb(56, 189, 248),   // #38BDF8
                accent_amber: Color32::from_rgb(245, 158, 11),  // #F59E0B
                accent_purple: Color32::from_rgb(168, 85, 247), // #A855F7
                accent_red: Color32::from_rgb(239, 68, 68),     // #EF4444
            },
            ThemeMode::Light => Self {
                is_dark: false,
                bg_canvas: Color32::from_rgb(241, 245, 249),    // #F1F5F9 Slate 100 Canvas
                bg_sidebar: Color32::from_rgb(255, 255, 255),   // #FFFFFF Crisp White Sidebar
                bg_header: Color32::from_rgb(255, 255, 255),    // #FFFFFF Crisp White Header
                bg_card: Color32::from_rgb(255, 255, 255),      // #FFFFFF Elevated White Card
                bg_card_subtle: Color32::from_rgb(248, 250, 252), // #F8FAFC Slate 50 Row Surface
                bg_card_hover: Color32::from_rgb(226, 232, 240), // #E2E8F0 Slate 200 Hover
                bg_track: Color32::from_rgb(203, 213, 225),     // #CBD5E1 Progress Track
                border_subtle: Color32::from_rgb(203, 213, 225), // #CBD5E1 Visible Light Border
                border_strong: Color32::from_rgb(148, 163, 184), // #94A3B8 Active Light Border
                text_primary: Color32::from_rgb(15, 23, 42),    // #0F172A Slate 900 (>15:1)
                text_secondary: Color32::from_rgb(51, 65, 85),  // #334155 Slate 700 (>9:1)
                text_muted: Color32::from_rgb(71, 85, 105),     // #475569 Slate 600 (>7:1 AAA)
                accent_rust: Color32::from_rgb(234, 88, 12),    // #EA580C Orange 600
                accent_emerald: Color32::from_rgb(21, 128, 61), // #15803D Green 700
                accent_cyan: Color32::from_rgb(2, 132, 199),    // #0284C7 Sky 600
                accent_amber: Color32::from_rgb(180, 83, 9),    // #B45309 Amber 700
                accent_purple: Color32::from_rgb(126, 34, 206), // #7E22CE Purple 700
                accent_red: Color32::from_rgb(220, 38, 38),     // #DC2626 Red 600
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActiveTab {
    Cleaner,
    DiskAnalyzer,
    Registry,
    Startup,
    Uninstaller,
    Decrapifier,
    DriveWiper,
    SoftwareUpdater,
    DriverUpdater,
    AutomationMonitor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecrapSubtab {
    Tweaks,
    Appx,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiskSubView {
    Duplicates,
    LargeFiles,
    Categories,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum VectorIcon {
    BrandShield,
    CleanerSweep,
    DiskRadar,
    FolderOpen,
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
    SunLight,
    MoonDark,
    GlobeLang,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ButtonVariant {
    PrimaryScan,
    ExecuteBrand,
    Secondary,
    Danger,
}

pub type RegFixResult = (
    Result<crate::registry::RegistryFixReport, String>,
    Option<Vec<RegistryIssue>>,
);

pub struct RustCleanerApp {
    pub fonts_initialized: bool,
    pub applied_theme: Option<ThemeMode>,
    pub theme_mode: ThemeMode,
    pub language: Language,
    pub active_tab: ActiveTab,
    pub is_admin: bool,
    pub status_banner: String,

    // 1. Cleaner State
    pub clean_rules: Vec<CleanRule>,
    pub clean_summary: Option<CleanSummary>,
    pub cleaner_busy: bool,
    pub cleaner_result_rx: Arc<Mutex<Option<(CleanSummary, bool)>>>,
    pub selected_rule_group_filter: Option<RuleGroup>,
    pub cleaner_search_query: String,

    // 2. Registry State
    pub reg_categories: Vec<(RegistryCategory, bool)>,
    pub reg_issues: Vec<RegistryIssue>,
    pub reg_filter: String,
    pub reg_backup_dir: PathBuf,
    pub reg_last_backup: Option<PathBuf>,
    pub reg_busy: bool,
    pub reg_scan_rx: Arc<Mutex<Option<Vec<RegistryIssue>>>>,
    pub reg_fix_rx: Arc<Mutex<Option<RegFixResult>>>,

    // 3. Startup & Uninstaller State
    pub startup_entries: Vec<StartupEntry>,
    pub startup_filter: String,
    pub installed_apps: Vec<InstalledApp>,
    pub uninstall_filter: String,

    // 4. Drive Wiper State
    pub drives: Vec<DriveInfo>,
    pub selected_drive_idx: usize,
    pub wipe_algorithm: WipeAlgorithm,
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
    pub driver_action_rx: Arc<Mutex<Option<Result<String, String>>>>,
    pub driver_filter: String,

    // 7. Zero-RAM Scheduler (`schtasks.exe`) & Zero-Polling Kernel Monitor (`notify`)
    pub schedule_status: ScheduledTaskStatus,
    pub schedule_freq: ScheduleFrequency,
    pub schedule_time_hhmm: String,
    pub schedule_highest_uac: bool,
    pub schedule_logs: Vec<String>,
    pub silent_clean_busy: bool,
    pub silent_clean_rx: Arc<Mutex<Option<Result<String, String>>>>,
    pub monitor_state: Arc<Mutex<RealtimeMonitorState>>,
    pub monitor_stop_flag: Arc<AtomicBool>,

    // 8. Disk Analyzer & SHA-256 Duplicate Finder State
    pub disk_preset: ScanTargetPreset,
    pub disk_custom_path: String,
    pub disk_min_large_mb: u64,
    pub disk_min_dup_kb: u64,
    pub disk_subview: DiskSubView,
    pub disk_search_filter: String,
    pub disk_busy: bool,
    pub disk_cancel: Arc<AtomicBool>,
    pub disk_report: Option<DiskAnalysisReport>,
    pub disk_rx: Arc<Mutex<Option<DiskAnalysisReport>>>,

    // 9. Deep Residual Hunter (Leftover Hunter) State
    pub residual_summary: Option<crate::residual_hunter::ResidualScanSummary>,
    pub residual_filter: String,
    pub residual_status_msg: Option<String>,
    pub is_global_orphan_mode: bool,

    // 10. Windows 11 Privacy & Decrapifier State
    pub decrap_tweaks_state: Vec<(String, bool)>,
    pub decrap_appx_packages: Vec<crate::decrapifier::AppxPackageInfo>,
    pub decrap_appx_scanned: bool,
    pub decrap_appx_busy: bool,
    pub decrap_appx_rx: Arc<Mutex<Option<Vec<crate::decrapifier::AppxPackageInfo>>>>,
    pub decrap_subtab: DecrapSubtab,
    pub decrap_status_msg: Option<String>,
    pub decrap_filter: String,
}

impl Default for RustCleanerApp {
    fn default() -> Self {
        let is_admin = uac::is_elevated();
        let backup_dir = env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join("registry_backups");
        let drives = list_drives();
        let language = Language::Italian;
        let s = language.strings();

        Self {
            fonts_initialized: false,
            applied_theme: None,
            theme_mode: ThemeMode::Dark,
            language,
            active_tab: ActiveTab::Cleaner,
            is_admin,
            status_banner: if is_admin {
                s.default_banner_admin.to_string()
            } else {
                s.default_banner_user.to_string()
            },
            clean_rules: default_clean_rules(),
            clean_summary: None,
            cleaner_busy: false,
            cleaner_result_rx: Arc::new(Mutex::new(None)),
            selected_rule_group_filter: None,
            cleaner_search_query: String::new(),
            reg_categories: RegistryCategory::all()
                .iter()
                .map(|&c| (c, true))
                .collect(),
            reg_issues: Vec::new(),
            reg_filter: String::new(),
            reg_backup_dir: backup_dir,
            reg_last_backup: None,
            reg_busy: false,
            reg_scan_rx: Arc::new(Mutex::new(None)),
            reg_fix_rx: Arc::new(Mutex::new(None)),
            startup_entries: list_startup_entries(),
            startup_filter: String::new(),
            installed_apps: list_installed_apps(),
            uninstall_filter: String::new(),
            drives,
            selected_drive_idx: 0,
            wipe_algorithm: WipeAlgorithm::SimpleOnePassZero,
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
            driver_action_rx: Arc::new(Mutex::new(None)),
            driver_filter: String::new(),
            schedule_status: query_scheduled_task(),
            schedule_freq: ScheduleFrequency::Weekly,
            schedule_time_hhmm: "18:00".to_string(),
            schedule_highest_uac: false,
            schedule_logs: read_scheduled_logs(),
            silent_clean_busy: false,
            silent_clean_rx: Arc::new(Mutex::new(None)),
            monitor_state: Arc::new(Mutex::new(RealtimeMonitorState::default())),
            monitor_stop_flag: Arc::new(AtomicBool::new(false)),
            disk_preset: ScanTargetPreset::UserProfileMediaAndDocs,
            disk_custom_path: env::var("USERPROFILE").unwrap_or_else(|_| r"C:\".to_string()),
            disk_min_large_mb: 50,
            disk_min_dup_kb: 256,
            disk_subview: DiskSubView::Duplicates,
            disk_search_filter: String::new(),
            disk_busy: false,
            disk_cancel: Arc::new(AtomicBool::new(false)),
            disk_report: None,
            disk_rx: Arc::new(Mutex::new(None)),
            residual_summary: None,
            residual_filter: String::new(),
            residual_status_msg: None,
            is_global_orphan_mode: false,
            decrap_tweaks_state: crate::decrapifier::REGISTRY_TWEAKS
                .iter()
                .map(|t| (t.id.to_string(), crate::decrapifier::is_tweak_active(t)))
                .collect(),
            decrap_appx_packages: Vec::new(),
            decrap_appx_scanned: false,
            decrap_appx_busy: false,
            decrap_appx_rx: Arc::new(Mutex::new(None)),
            decrap_subtab: DecrapSubtab::Tweaks,
            decrap_status_msg: None,
            decrap_filter: String::new(),
        }
    }
}

/// Loads Windows native CJK fallback font (`msyh.ttc` / `simsun.ttc`) so Chinese (`中文`)
/// and all European accented glyphs render cleanly without external dependencies.
fn configure_cjk_fonts_once(ctx: &egui::Context) {
    let candidates = [
        r"C:\Windows\Fonts\msyh.ttc",
        r"C:\Windows\Fonts\simsun.ttc",
        r"C:\Windows\Fonts\segoeui.ttf",
    ];
    for path in candidates {
        if let Ok(bytes) = std::fs::read(path) {
            let mut fonts = egui::FontDefinitions::default();
            fonts.font_data.insert(
                "win_cjk_fallback".to_owned(),
                egui::FontData::from_owned(bytes).into(),
            );
            if let Some(fam) = fonts.families.get_mut(&egui::FontFamily::Proportional) {
                fam.push("win_cjk_fallback".to_owned());
            }
            if let Some(fam) = fonts.families.get_mut(&egui::FontFamily::Monospace) {
                fam.push("win_cjk_fallback".to_owned());
            }
            ctx.set_fonts(fonts);
            break;
        }
    }
}

fn apply_pro_max_theme(ctx: &egui::Context, mode: ThemeMode) {
    let pal = ThemePalette::for_mode(mode);
    let mut style = (*ctx.style()).clone();
    let mut visuals = if pal.is_dark {
        egui::Visuals::dark()
    } else {
        egui::Visuals::light()
    };

    visuals.panel_fill = pal.bg_canvas;
    visuals.window_fill = pal.bg_card;
    visuals.extreme_bg_color = if pal.is_dark {
        Color32::from_rgb(8, 11, 19)
    } else {
        Color32::from_rgb(255, 255, 255)
    };
    visuals.faint_bg_color = pal.bg_card_subtle;
    visuals.override_text_color = Some(pal.text_primary);

    let radius = CornerRadius::same(8);
    visuals.window_corner_radius = CornerRadius::same(12);
    visuals.menu_corner_radius = CornerRadius::same(8);

    visuals.widgets.noninteractive.bg_fill = pal.bg_card;
    visuals.widgets.noninteractive.weak_bg_fill = pal.bg_card;
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, pal.border_subtle);
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, pal.text_primary);
    visuals.widgets.noninteractive.corner_radius = radius;

    visuals.widgets.inactive.bg_fill = pal.bg_card_subtle;
    visuals.widgets.inactive.weak_bg_fill = pal.bg_card_subtle;
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0_f32, pal.border_subtle);
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0_f32, pal.text_primary);
    visuals.widgets.inactive.corner_radius = radius;

    visuals.widgets.hovered.bg_fill = pal.bg_card_hover;
    visuals.widgets.hovered.weak_bg_fill = pal.bg_card_hover;
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.5_f32, pal.accent_cyan);
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.5_f32, pal.text_primary);
    visuals.widgets.hovered.corner_radius = radius;

    visuals.widgets.active.bg_fill = pal.accent_rust;
    visuals.widgets.active.weak_bg_fill = pal.accent_rust;
    visuals.widgets.active.bg_stroke = Stroke::new(1.5_f32, Color32::WHITE);
    visuals.widgets.active.fg_stroke = Stroke::new(1.5_f32, Color32::WHITE);
    visuals.widgets.active.corner_radius = radius;

    visuals.selection.bg_fill = Color32::from_rgba_unmultiplied(
        pal.accent_cyan.r(),
        pal.accent_cyan.g(),
        pal.accent_cyan.b(),
        55,
    );
    visuals.selection.stroke = Stroke::new(1.5_f32, pal.accent_cyan);

    style.visuals = visuals;
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
    let s = Stroke::new(1.6_f32, color);

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
        VectorIcon::SunLight => {
            painter.circle_stroke(c, r * 0.48, s);
            for angle_deg in (0..360).step_by(45) {
                let rad = (angle_deg as f32).to_radians();
                let p1 = Pos2::new(c.x + rad.cos() * r * 0.68, c.y + rad.sin() * r * 0.68);
                let p2 = Pos2::new(c.x + rad.cos() * r * 0.98, c.y + rad.sin() * r * 0.98);
                painter.line_segment([p1, p2], s);
            }
        }
        VectorIcon::MoonDark => {
            painter.circle_stroke(c, r * 0.75, s);
            painter.circle_filled(Pos2::new(c.x + r * 0.28, c.y - r * 0.22), r * 0.48, color);
        }
        VectorIcon::GlobeLang => {
            painter.circle_stroke(c, r * 0.88, s);
            painter.line_segment(
                [Pos2::new(c.x - r * 0.88, c.y), Pos2::new(c.x + r * 0.88, c.y)],
                s,
            );
            painter.line_segment(
                [Pos2::new(c.x, c.y - r * 0.88), Pos2::new(c.x, c.y + r * 0.88)],
                s,
            );
        }
        VectorIcon::DiskRadar => {
            painter.circle_stroke(c, r * 0.9, s);
            painter.circle_stroke(c, r * 0.45, s);
            painter.line_segment(
                [c, Pos2::new(c.x + r * 0.64, c.y - r * 0.64)],
                s,
            );
            painter.circle_filled(c, 1.8, color);
        }
        VectorIcon::FolderOpen => {
            let folder = Rect::from_min_max(
                Pos2::new(c.x - r * 0.85, c.y - r * 0.45),
                Pos2::new(c.x + r * 0.85, c.y + r * 0.72),
            );
            painter.rect_stroke(folder, CornerRadius::same(2), s, egui::StrokeKind::Inside);
            painter.line_segment(
                [
                    Pos2::new(folder.left(), folder.top() - r * 0.25),
                    Pos2::new(c.x - r * 0.1, folder.top() - r * 0.25),
                ],
                s,
            );
        }
    }
}

/// Renders a semantic action button with a vector icon, proper contrast, and pointing-hand cursor.
fn draw_action_button(
    ui: &mut egui::Ui,
    pal: &ThemePalette,
    label: &str,
    icon: VectorIcon,
    variant: ButtonVariant,
    enabled: bool,
) -> bool {
    let (bg_fill, border_stroke, text_color) = match variant {
        ButtonVariant::PrimaryScan => (
            if pal.is_dark {
                Color32::from_rgb(18, 42, 68) // #122A44
            } else {
                Color32::from_rgb(224, 242, 254) // #E0F2FE Sky 100
            },
            Stroke::new(1.2_f32, pal.accent_cyan),
            if pal.is_dark {
                pal.accent_cyan // #38BDF8 -> 6.86:1 AA Pass
            } else {
                Color32::from_rgb(7, 89, 133) // #075985 Sky 800 -> 6.55:1 AA Pass
            },
        ),
        ButtonVariant::ExecuteBrand => (
            Color32::from_rgb(194, 65, 12), // #C2410C Orange 700 -> 5.19:1 AA Pass in both modes
            Stroke::new(
                1.0_f32,
                if pal.is_dark {
                    Color32::from_rgb(234, 88, 12) // #EA580C Orange 600
                } else {
                    Color32::from_rgb(194, 65, 12) // #C2410C Orange 700
                },
            ),
            Color32::WHITE, // #FFFFFF
        ),
        ButtonVariant::Secondary => (
            pal.bg_card_subtle,
            Stroke::new(1.0_f32, pal.border_strong),
            pal.text_primary, // > 15:1 AAA Pass
        ),
        ButtonVariant::Danger => (
            if pal.is_dark {
                Color32::from_rgb(69, 20, 26) // #45141A Deep Red
            } else {
                Color32::from_rgb(254, 226, 226) // #FEE2E2 Red 100
            },
            Stroke::new(1.2_f32, pal.accent_red),
            if pal.is_dark {
                Color32::from_rgb(248, 113, 113) // #F87171 Red 400 -> 5.54:1 AA Pass
            } else {
                Color32::from_rgb(185, 28, 28) // #B91C1C Red 700 -> 5.24:1 AA Pass
            },
        ),
    };

    let fill = if enabled {
        bg_fill
    } else {
        Color32::from_rgba_unmultiplied(bg_fill.r(), bg_fill.g(), bg_fill.b(), 85)
    };
    let fg = if enabled { text_color } else { pal.text_muted };
    let stroke = if enabled {
        border_stroke
    } else {
        Stroke::new(1.0_f32, pal.border_subtle)
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

    let btn_id = ui.next_auto_id().with(label);
    if enabled {
        let interact = ui.interact(
            resp.response.rect,
            btn_id,
            Sense::click(),
        );
        if interact.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            ui.painter().rect_stroke(
                resp.response.rect,
                CornerRadius::same(8),
                Stroke::new(1.5_f32, fg),
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
    pal: &ThemePalette,
    title: &str,
    value: &str,
    subtitle: &str,
    accent: Color32,
    icon: VectorIcon,
) {
    let avail_w = ui.available_width();
    egui::Frame::new()
        .fill(pal.bg_card)
        .stroke(Stroke::new(1.0_f32, pal.border_subtle))
        .corner_radius(CornerRadius::same(10))
        .inner_margin(egui::Margin::symmetric(14, 11))
        .show(ui, |ui| {
            ui.set_width((avail_w - 28.0).max(110.0));
            ui.set_min_height(MIN_KPI_CARD_HEIGHT); // Guarantees row height parity across all languages
            ui.horizontal(|ui| {
                let (icon_rect, _) = ui.allocate_exact_size(egui::vec2(18.0, 18.0), Sense::hover());
                paint_vector_icon(ui.painter(), icon_rect, icon, accent);
                ui.label(
                    RichText::new(title)
                        .size(11.0)
                        .color(pal.text_secondary)
                        .strong(),
                );
            });
            ui.add_space(2.0);
            ui.label(
                RichText::new(value)
                    .size(19.5)
                    .strong()
                    .monospace()
                    .color(pal.text_primary),
            );
            ui.label(RichText::new(subtitle).size(11.0).color(pal.text_muted));
        });
}

/// Single-line non-wrapping status pill badge (`compact-label-overflow`).
fn draw_pill_badge(ui: &mut egui::Ui, text: &str, bg: Color32, fg: Color32) {
    egui::Frame::new()
        .fill(bg)
        .stroke(Stroke::new(
            1.0_f32,
            Color32::from_rgba_unmultiplied(fg.r(), fg.g(), fg.b(), 75),
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

fn card_frame(pal: &ThemePalette) -> egui::Frame {
    egui::Frame::new()
        .fill(pal.bg_card)
        .stroke(Stroke::new(1.0_f32, pal.border_subtle))
        .corner_radius(CornerRadius::same(10))
        .inner_margin(egui::Margin::symmetric(14, 12))
}

/// Renders an Empty State illustration + title + description (`empty-states` rule).
fn draw_empty_state(
    ui: &mut egui::Ui,
    pal: &ThemePalette,
    icon: VectorIcon,
    accent: Color32,
    title: &str,
    desc: &str,
) {
    ui.vertical_centered(|ui| {
        ui.add_space(44.0);
        egui::Frame::new()
            .fill(Color32::from_rgba_unmultiplied(
                accent.r(),
                accent.g(),
                accent.b(),
                26,
            ))
            .stroke(Stroke::new(
                1.0_f32,
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
                .color(pal.text_primary),
        );
        ui.add_space(4.0);
        ui.label(RichText::new(desc).size(12.0).color(pal.text_muted));
    });
}

/// Renders a sidebar navigation item with a custom vector icon, active indicator border,
/// and optional live badge count (`nav-state-active` & `tab-badge`).
#[allow(clippy::too_many_arguments)]
fn draw_sidebar_nav_item(
    ui: &mut egui::Ui,
    pal: &ThemePalette,
    icon: VectorIcon,
    accent: Color32,
    title: &str,
    subtitle: &str,
    badge_count: Option<usize>,
    is_selected: bool,
) -> bool {
    let mut clicked = false;
    let fill = if is_selected {
        pal.bg_card_hover
    } else {
        pal.bg_card_subtle
    };
    let border = if is_selected {
        Stroke::new(1.5_f32, accent)
    } else {
        Stroke::new(1.0_f32, pal.border_subtle)
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

                egui::Frame::new()
                    .fill(if is_selected {
                        accent
                    } else {
                        Color32::from_rgba_unmultiplied(accent.r(), accent.g(), accent.b(), 30)
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
                                .color(pal.text_primary),
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
                                        38,
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
                                pal.text_secondary
                            } else {
                                pal.text_muted
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
        if !self.fonts_initialized {
            configure_cjk_fonts_once(ctx);
            self.fonts_initialized = true;
        }
        if self.applied_theme != Some(self.theme_mode) {
            apply_pro_max_theme(ctx, self.theme_mode);
            self.applied_theme = Some(self.theme_mode);
        }

        let pal = ThemePalette::for_mode(self.theme_mode);
        let s = self.language.strings();

        // Poll background cleaner results
        if let Ok(mut guard) = self.cleaner_result_rx.lock() {
            if let Some((summary, was_clean)) = guard.take() {
                self.cleaner_busy = false;
                if was_clean {
                    self.status_banner = format!(
                        "{}: {} ({}) — {} {}.",
                        s.kpi_space_freed,
                        summary.total_files_deleted,
                        format_bytes(summary.total_bytes_deleted),
                        summary.total_skipped_locked,
                        s.banner_skipped_suffix
                    );
                } else {
                    self.status_banner = format!(
                        "{}: {} ({}) — {} {}.",
                        s.kpi_space_found,
                        summary.total_files_found,
                        format_bytes(summary.total_bytes_found),
                        summary.total_skipped_locked,
                        s.banner_locked_suffix
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

        // Poll background Disk Analyzer & Duplicate Finder results
        if let Ok(mut guard) = self.disk_rx.lock() {
            if let Some(rep) = guard.take() {
                self.disk_busy = false;
                self.status_banner = rep.status_message.clone();
                self.disk_report = Some(rep);
            }
        }

        // Poll background Decrapifier AppX scan results
        if let Ok(mut guard) = self.decrap_appx_rx.lock() {
            if let Some(pkgs) = guard.take() {
                self.decrap_appx_busy = false;
                self.decrap_appx_scanned = true;
                self.decrap_appx_packages = pkgs;
            }
        }

        // Poll background Registry scan results
        if let Ok(mut guard) = self.reg_scan_rx.lock() {
            if let Some(issues) = guard.take() {
                self.reg_busy = false;
                let count = issues.len();
                self.reg_issues = issues;
                self.status_banner = format!("{} ({})", s.reg_title, count);
            }
        }

        // Poll background Registry fix results
        if let Ok(mut guard) = self.reg_fix_rx.lock() {
            if let Some((fix_res, post_scan)) = guard.take() {
                self.reg_busy = false;
                match fix_res {
                    Ok(report) => {
                        self.reg_last_backup = Some(report.backup_file.clone());
                        self.status_banner = format!(
                            "{} '{}' ({} {}).",
                            s.reg_last_backup_prefix,
                            report.backup_file.display(),
                            report.fixed_count,
                            s.banner_fixed_suffix,
                        );
                        if let Some(issues) = post_scan {
                            self.reg_issues = issues;
                        }
                    }
                    Err(e) => self.status_banner = e,
                }
            }
        }

        // Poll background Driver install action results
        let drv_action_msg = self
            .driver_action_rx
            .lock()
            .ok()
            .and_then(|mut g| g.take());
        if let Some(res) = drv_action_msg {
            self.driver_busy = false;
            match res {
                Ok(msg) => {
                    self.status_banner = msg;
                    self.start_driver_scan(true);
                }
                Err(e) => {
                    self.status_banner = e;
                }
            }
        }

        // Poll background Silent Clean test results
        if let Ok(mut guard) = self.silent_clean_rx.lock() {
            if let Some(res) = guard.take() {
                self.silent_clean_busy = false;
                match res {
                    Ok(msg) => {
                        self.status_banner = msg;
                        self.schedule_logs = read_scheduled_logs();
                    }
                    Err(e) => self.status_banner = e,
                }
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
            || self.disk_busy
            || wiper_running
            || self.winget_busy
            || self.driver_busy
            || self.reg_busy
            || self.silent_clean_busy
            || monitor_active
        {
            ctx.request_repaint_after(std::time::Duration::from_millis(250));
        }

        // === TOP HEADER BAR (Brand + Language Switcher + Dark/Light Toggle + UAC + Drive) ===
        egui::TopBottomPanel::top("top_header")
            .frame(
                egui::Frame::new()
                    .fill(pal.bg_header)
                    .stroke(Stroke::new(1.0_f32, pal.border_subtle))
                    .inner_margin(egui::Margin::symmetric(18, 10)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    // Vector Shield Brand Emblem
                    egui::Frame::new()
                        .fill(Color32::from_rgba_unmultiplied(
                            pal.accent_rust.r(),
                            pal.accent_rust.g(),
                            pal.accent_rust.b(),
                            30,
                        ))
                        .stroke(Stroke::new(1.5_f32, pal.accent_rust))
                        .corner_radius(CornerRadius::same(9))
                        .inner_margin(egui::Margin::same(7))
                        .show(ui, |ui| {
                            let (r, _) =
                                ui.allocate_exact_size(egui::vec2(22.0, 22.0), Sense::hover());
                            paint_vector_icon(
                                ui.painter(),
                                r,
                                VectorIcon::BrandShield,
                                pal.accent_rust,
                            );
                        });

                    let screen_w = ctx.screen_rect().width();
                    let is_compact_header = is_compact_header(screen_w);
                    let is_ultra_compact = is_ultra_compact(screen_w);

                    ui.add_space(4.0);
                    ui.vertical(|ui| {
                        ui.spacing_mut().item_spacing.y = 2.0;
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new("RustCleaner")
                                    .size(19.0)
                                    .strong()
                                    .color(pal.text_primary),
                            );
                            draw_pill_badge(
                                ui,
                                "v1.4.0-alpha",
                                Color32::from_rgba_unmultiplied(
                                    pal.accent_rust.r(),
                                    pal.accent_rust.g(),
                                    pal.accent_rust.b(),
                                    30,
                                ),
                                pal.accent_rust,
                            );
                            if !is_ultra_compact {
                                draw_pill_badge(
                                    ui,
                                    s.badge_zero_telemetry,
                                    Color32::from_rgba_unmultiplied(
                                        pal.accent_emerald.r(),
                                        pal.accent_emerald.g(),
                                        pal.accent_emerald.b(),
                                        28,
                                    ),
                                    pal.accent_emerald,
                                );
                            }
                        });
                        if !is_compact_header {
                            ui.label(
                                RichText::new(s.header_subtitle)
                                    .size(11.5)
                                    .color(pal.text_secondary),
                            );
                        }
                    });

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        // 1. UAC Status / Elevate Button
                        if self.is_admin {
                            draw_pill_badge(
                                ui,
                                s.uac_admin_active,
                                Color32::from_rgba_unmultiplied(
                                    pal.accent_emerald.r(),
                                    pal.accent_emerald.g(),
                                    pal.accent_emerald.b(),
                                    32,
                                ),
                                pal.accent_emerald,
                            );
                        } else {
                            let uac_text_color = if pal.is_dark {
                                Color32::from_rgb(15, 23, 42) // #0F172A Slate 900 -> 8.28:1 AAA Pass on #F59E0B
                            } else {
                                Color32::WHITE // #FFFFFF -> 5.12:1 AA Pass on #B45309
                            };

                            let uac_btn = egui::Button::new(
                                RichText::new(s.btn_elevate_uac)
                                    .size(11.5)
                                    .strong()
                                    .color(uac_text_color),
                            )
                            .fill(pal.accent_amber)
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
                                s.uac_standard_mode,
                                Color32::from_rgba_unmultiplied(
                                    pal.accent_amber.r(),
                                    pal.accent_amber.g(),
                                    pal.accent_amber.b(),
                                    28,
                                ),
                                pal.accent_amber,
                            );
                        }

                        // 2. Dark / Light Theme Switcher Button
                        let (theme_icon, theme_lbl) = match self.theme_mode {
                            ThemeMode::Dark => (VectorIcon::MoonDark, s.theme_dark_label),
                            ThemeMode::Light => (VectorIcon::SunLight, s.theme_light_label),
                        };
                        if draw_action_button(
                            ui,
                            &pal,
                            theme_lbl,
                            theme_icon,
                            ButtonVariant::Secondary,
                            true,
                        ) {
                            self.theme_mode = match self.theme_mode {
                                ThemeMode::Dark => ThemeMode::Light,
                                ThemeMode::Light => ThemeMode::Dark,
                            };
                        }

                        // 3. 7-Language Selector (IT, EN, FR, ES, DE, ZH, PT)
                        egui::Frame::new()
                            .fill(pal.bg_card_subtle)
                            .stroke(Stroke::new(1.0_f32, pal.border_strong))
                            .corner_radius(CornerRadius::same(8))
                            .inner_margin(egui::Margin::symmetric(8, 4))
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    ui.spacing_mut().item_spacing.x = 5.0;
                                    let (g_rect, _) = ui.allocate_exact_size(
                                        egui::vec2(15.0, 15.0),
                                        Sense::hover(),
                                    );
                                    paint_vector_icon(
                                        ui.painter(),
                                        g_rect,
                                        VectorIcon::GlobeLang,
                                        pal.accent_cyan,
                                    );
                                    let prev_lang = self.language;
                                    egui::ComboBox::from_id_salt("header_lang_selector")
                                        .selected_text(
                                            RichText::new(self.language.display_name())
                                                .size(12.0)
                                                .strong()
                                                .color(pal.text_primary),
                                        )
                                        .width(135.0)
                                        .show_ui(ui, |ui| {
                                            for &lang in Language::all() {
                                                let resp = ui.selectable_value(
                                                    &mut self.language,
                                                    lang,
                                                    lang.display_name(),
                                                );
                                                if resp.hovered() {
                                                    ui.ctx().set_cursor_icon(
                                                        egui::CursorIcon::PointingHand,
                                                    );
                                                }
                                            }
                                        });
                                    if self.language != prev_lang {
                                        let new_s = self.language.strings();
                                        self.status_banner = if self.is_admin {
                                            new_s.default_banner_admin.to_string()
                                        } else {
                                            new_s.default_banner_user.to_string()
                                        };
                                    }
                                });
                            });

                        // 4. Primary Drive Free Space Indicator
                        if let Some(d) = self.drives.first() {
                            let used_ratio = if d.total_bytes > 0 {
                                d.used_bytes as f32 / d.total_bytes as f32
                            } else {
                                0.0
                            };
                            egui::Frame::new()
                                .fill(pal.bg_card)
                                .stroke(Stroke::new(1.0_f32, pal.border_subtle))
                                .corner_radius(CornerRadius::same(8))
                                .inner_margin(egui::Margin::symmetric(10, 5))
                                .show(ui, |ui| {
                                    ui.horizontal(|ui| {
                                        ui.label(
                                            RichText::new(format!(
                                                "{} {}: {}",
                                                d.root_path,
                                                s.drive_free_label,
                                                format_bytes(d.free_bytes)
                                            ))
                                            .size(11.0)
                                            .monospace()
                                            .color(pal.text_secondary),
                                        );
                                        let (bar_rect, _) = ui.allocate_exact_size(
                                            egui::vec2(50.0, 6.0),
                                            Sense::hover(),
                                        );
                                        ui.painter().rect_filled(
                                            bar_rect,
                                            CornerRadius::same(3),
                                            pal.bg_track,
                                        );
                                        let mut fill_rect = bar_rect;
                                        fill_rect.set_width(
                                            bar_rect.width() * used_ratio.clamp(0.0, 1.0),
                                        );
                                        ui.painter().rect_filled(
                                            fill_rect,
                                            CornerRadius::same(3),
                                            if used_ratio > 0.85 {
                                                pal.accent_red
                                            } else {
                                                pal.accent_cyan
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
                    .fill(pal.bg_header)
                    .stroke(Stroke::new(1.0_f32, pal.border_subtle))
                    .inner_margin(egui::Margin::symmetric(18, 7)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    let is_busy = self.cleaner_busy
                        || self.disk_busy
                        || wiper_running
                        || self.winget_busy
                        || self.driver_busy;
                    let (dot_color, state_label) = if is_busy {
                        (pal.accent_amber, s.status_running)
                    } else {
                        (pal.accent_emerald, s.status_ready)
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
                            .color(pal.text_secondary),
                    );
                });
            });

        // === LEFT NAVIGATION SIDEBAR ===
        egui::SidePanel::left("left_sidebar")
            .resizable(false)
            .exact_width(256.0)
            .frame(
                egui::Frame::new()
                    .fill(pal.bg_sidebar)
                    .stroke(Stroke::new(1.0_f32, pal.border_subtle))
                    .inner_margin(egui::Margin::symmetric(12, 12)),
            )
            .show(ctx, |ui| {
                egui::ScrollArea::vertical()
                    .id_salt("sidebar_nav_scroll")
                    .show(ui, |ui| {
                        ui.label(
                            RichText::new(s.sidebar_sec_maintenance)
                                .size(10.5)
                                .strong()
                                .color(pal.text_muted),
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

                        let ds = self.language.disk_analyzer();
                        let dec = self.language.decrapifier();
                        let decrap_active_cnt = {
                            let count = self.decrap_tweaks_state.iter().filter(|(_, active)| *active).count();
                            if count > 0 { Some(count) } else { None }
                        };
                        let disk_dup_cnt = self
                            .disk_report
                            .as_ref()
                            .map(|r| r.duplicate_groups.len())
                            .filter(|&c| c > 0);

                        let nav_items = [
                            (
                                ActiveTab::Cleaner,
                                VectorIcon::CleanerSweep,
                                pal.accent_rust,
                                s.nav_cleaner_title,
                                s.nav_cleaner_sub,
                                None,
                            ),
                            (
                                ActiveTab::DiskAnalyzer,
                                VectorIcon::DiskRadar,
                                pal.accent_cyan,
                                ds.nav_title,
                                ds.nav_sub,
                                disk_dup_cnt,
                            ),
                            (
                                ActiveTab::Registry,
                                VectorIcon::RegistryGrid,
                                pal.accent_amber,
                                s.nav_registry_title,
                                s.nav_registry_sub,
                                reg_cnt,
                            ),
                            (
                                ActiveTab::Startup,
                                VectorIcon::StartupBolt,
                                pal.accent_cyan,
                                s.nav_startup_title,
                                s.nav_startup_sub,
                                Some(self.startup_entries.len()),
                            ),
                            (
                                ActiveTab::Uninstaller,
                                VectorIcon::UninstallerBox,
                                pal.accent_purple,
                                s.nav_uninstall_title,
                                s.nav_uninstall_sub,
                                Some(self.installed_apps.len()),
                            ),
                            (
                                ActiveTab::Decrapifier,
                                VectorIcon::CheckShield,
                                pal.accent_emerald,
                                dec.nav_title,
                                dec.nav_sub,
                                decrap_active_cnt,
                            ),
                            (
                                ActiveTab::DriveWiper,
                                VectorIcon::DriveCylinder,
                                pal.accent_emerald,
                                s.nav_wiper_title,
                                s.nav_wiper_sub,
                                None,
                            ),
                            (
                                ActiveTab::SoftwareUpdater,
                                VectorIcon::SoftwareCloudUp,
                                pal.accent_cyan,
                                s.nav_soft_title,
                                s.nav_soft_sub,
                                winget_cnt,
                            ),
                            (
                                ActiveTab::DriverUpdater,
                                VectorIcon::DriverChip,
                                pal.accent_emerald,
                                s.nav_driver_title,
                                s.nav_driver_sub,
                                driver_cnt,
                            ),
                            (
                                ActiveTab::AutomationMonitor,
                                VectorIcon::AutomationPulse,
                                pal.accent_purple,
                                s.nav_auto_title,
                                s.nav_auto_sub,
                                None,
                            ),
                        ];

                        for (idx, (tab, icon, color, title, subtitle, badge_cnt)) in
                            nav_items.iter().enumerate()
                        {
                            if idx == 6 {
                                ui.add_space(6.0);
                                ui.label(
                                    RichText::new(s.sidebar_sec_premium)
                                        .size(10.5)
                                        .strong()
                                        .color(pal.accent_cyan),
                                );
                                ui.add_space(3.0);
                            }

                            let is_selected = self.active_tab == *tab;
                            if draw_sidebar_nav_item(
                                ui,
                                &pal,
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
                            .fill(pal.bg_card)
                            .stroke(Stroke::new(1.0_f32, pal.border_subtle))
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
                                        pal.accent_emerald,
                                    );
                                    ui.label(
                                        RichText::new(s.privacy_card_title)
                                            .size(11.0)
                                            .color(pal.accent_emerald)
                                            .strong(),
                                    );
                                });
                                ui.label(
                                    RichText::new(s.privacy_card_desc)
                                        .size(10.5)
                                        .color(pal.text_secondary),
                                );
                            });
                    });
            });

        // === CENTRAL MAIN PANEL ===
        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(pal.bg_canvas)
                    .inner_margin(egui::Margin::symmetric(18, 14)),
            )
            .show(ctx, |ui| match self.active_tab {
                ActiveTab::Cleaner => self.render_cleaner_tab(ui, &pal),
                ActiveTab::DiskAnalyzer => self.render_disk_analyzer_tab(ui, &pal),
                ActiveTab::Registry => self.render_registry_tab(ui, &pal),
                ActiveTab::Startup => self.render_startup_tab(ui, &pal),
                ActiveTab::Uninstaller => self.render_uninstaller_tab(ui, &pal),
                ActiveTab::Decrapifier => self.render_decrapifier_tab(ui, &pal),
                ActiveTab::DriveWiper => self.render_wiper_tab(ui, &pal),
                ActiveTab::SoftwareUpdater => self.render_software_updater_tab(ui, &pal),
                ActiveTab::DriverUpdater => self.render_driver_updater_tab(ui, &pal),
                ActiveTab::AutomationMonitor => self.render_automation_monitor_tab(ui, &pal),
            });
    }
}

impl RustCleanerApp {
    fn render_cleaner_tab(&mut self, ui: &mut egui::Ui, pal: &ThemePalette) {
        let s = self.language.strings();

        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.label(
                    RichText::new(s.cleaner_title)
                        .size(19.0)
                        .strong()
                        .color(pal.text_primary),
                );
                ui.label(
                    RichText::new(s.cleaner_subtitle)
                        .size(12.0)
                        .color(pal.text_secondary),
                );
            });

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if draw_action_button(
                    ui,
                    pal,
                    s.btn_start_clean,
                    VectorIcon::PlayRun,
                    ButtonVariant::ExecuteBrand,
                    !self.cleaner_busy,
                ) {
                    self.start_cleaner_job(true);
                }

                if draw_action_button(
                    ui,
                    pal,
                    s.btn_analyze_system,
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
            if let Some(sum) = &self.clean_summary {
                (
                    format_bytes(sum.total_bytes_found),
                    format!("{} {}", sum.total_files_found, s.kpi_files_unit),
                    format!("{}", sum.total_skipped_locked),
                    format_bytes(sum.total_bytes_deleted),
                    format!("{} {}", sum.total_files_deleted, s.kpi_files_unit),
                )
            } else {
                (
                    "—".to_string(),
                    s.kpi_waiting_scan.to_string(),
                    "—".to_string(),
                    "0 B".to_string(),
                    s.kpi_no_clean_yet.to_string(),
                )
            };

        ui.columns(4, |cols| {
            draw_kpi_card(
                &mut cols[0],
                pal,
                s.kpi_space_found,
                &found_bytes,
                &found_files,
                pal.accent_cyan,
                VectorIcon::SearchLens,
            );
            draw_kpi_card(
                &mut cols[1],
                pal,
                s.kpi_locked_files,
                &locked_files,
                s.kpi_locked_sub,
                pal.accent_amber,
                VectorIcon::BrandShield,
            );
            draw_kpi_card(
                &mut cols[2],
                pal,
                s.kpi_space_freed,
                &deleted_bytes,
                &deleted_files,
                pal.accent_emerald,
                VectorIcon::CheckShield,
            );
            let enabled_count = self.clean_rules.iter().filter(|r| r.enabled).count();
            draw_kpi_card(
                &mut cols[3],
                pal,
                s.kpi_active_rules,
                &format!("{} / {}", enabled_count, self.clean_rules.len()),
                s.kpi_active_rules_sub,
                pal.accent_purple,
                VectorIcon::CleanerSweep,
            );
        });

        ui.add_space(6.0);

        if let Some(summary) = &self.clean_summary {
            if summary.total_bytes_found > 0 {
                card_frame(pal).show(ui, |ui| {
                    let groups = [
                        (RuleGroup::WindowsSystem, s.grp_win_sys, pal.accent_rust),
                        (RuleGroup::WindowsExplorer, s.grp_explorer, pal.accent_amber),
                        (RuleGroup::BrowsersPrivacy, s.grp_browsers, pal.accent_cyan),
                        (RuleGroup::Applications, s.grp_apps, pal.accent_purple),
                    ];

                    let (bar_rect, _) = ui.allocate_exact_size(
                        egui::vec2(ui.available_width(), 8.0),
                        Sense::hover(),
                    );
                    ui.painter()
                        .rect_filled(bar_rect, CornerRadius::same(4), pal.bg_track);

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
                                    .color(pal.text_secondary),
                            );
                            ui.add_space(8.0);
                        }
                    });
                });
                ui.add_space(6.0);
            }
        }

        let remaining_h = (ui.available_height() - 10.0).max(240.0);
        let current_lang = self.language;

        ui.columns(2, |cols| {
            card_frame(pal).show(&mut cols[0], |ui| {
                ui.set_height(remaining_h);
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(s.cleaner_categories_hdr)
                            .size(14.5)
                            .strong()
                            .color(pal.text_primary),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let none_btn = egui::Button::new(
                            RichText::new(s.btn_none).size(11.0).color(pal.text_secondary),
                        )
                        .fill(pal.bg_card_subtle)
                        .stroke(Stroke::new(1.0_f32, pal.border_subtle))
                        .corner_radius(CornerRadius::same(12));
                        let resp_none = ui.add(none_btn);
                        if resp_none.hovered() {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                        }
                        if resp_none.clicked() {
                            for r in &mut self.clean_rules {
                                r.enabled = false;
                            }
                        }

                        let all_btn = egui::Button::new(
                            RichText::new(s.btn_all).size(11.0).color(pal.text_secondary),
                        )
                        .fill(pal.bg_card_subtle)
                        .stroke(Stroke::new(1.0_f32, pal.border_subtle))
                        .corner_radius(CornerRadius::same(12));
                        let resp_all = ui.add(all_btn);
                        if resp_all.hovered() {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                        }
                        if resp_all.clicked() {
                            for r in &mut self.clean_rules {
                                r.enabled = true;
                            }
                        }
                    });
                });

                ui.horizontal_wrapped(|ui| {
                    let filters = [
                        (None, s.flt_all),
                        (Some(RuleGroup::WindowsSystem), s.flt_system),
                        (Some(RuleGroup::WindowsExplorer), s.flt_explorer),
                        (Some(RuleGroup::BrowsersPrivacy), s.flt_browser),
                        (Some(RuleGroup::Applications), s.flt_app),
                    ];
                    for (flt, lbl) in filters {
                        let active = self.selected_rule_group_filter == flt;
                        let btn = egui::Button::new(
                            RichText::new(lbl).size(11.5).strong().color(if active {
                                Color32::WHITE
                            } else {
                                pal.text_secondary
                            }),
                        )
                        .fill(if active {
                            pal.accent_rust
                        } else {
                            pal.bg_card_subtle
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
                            let (r_name, r_desc) = current_lang
                                .localize_rule(rule.id)
                                .unwrap_or((&rule.name, &rule.description));

                            egui::Frame::new()
                                .fill(pal.bg_card_subtle)
                                .stroke(Stroke::new(1.0_f32, pal.border_subtle))
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
                                            RichText::new(r_name)
                                                .size(13.0)
                                                .strong()
                                                .color(pal.text_primary),
                                        );
                                        if rule.requires_admin {
                                            draw_pill_badge(
                                                ui,
                                                "UAC",
                                                Color32::from_rgba_unmultiplied(
                                                    pal.accent_amber.r(),
                                                    pal.accent_amber.g(),
                                                    pal.accent_amber.b(),
                                                    32,
                                                ),
                                                pal.accent_amber,
                                            );
                                        }
                                    });
                                    ui.label(
                                        RichText::new(r_desc).size(11.0).color(pal.text_muted),
                                    );
                                });
                            ui.add_space(4.0);
                        }
                    });
            });

            card_frame(pal).show(&mut cols[1], |ui| {
                ui.set_height(remaining_h);
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(s.cleaner_details_hdr)
                            .size(14.5)
                            .strong()
                            .color(pal.text_primary),
                    );
                    if self.clean_summary.is_some() {
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.add(
                                egui::TextEdit::singleline(&mut self.cleaner_search_query)
                                    .hint_text(s.lbl_search_filter)
                                    .desired_width(170.0),
                            );
                        });
                    }
                });
                ui.separator();

                if self.cleaner_busy {
                    ui.vertical_centered(|ui| {
                        ui.add_space(60.0);
                        ui.spinner();
                        ui.add_space(10.0);
                        ui.label(
                            RichText::new(s.cleaner_scanning_msg)
                                .size(13.5)
                                .color(pal.accent_cyan),
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
                                    pal,
                                    VectorIcon::CheckShield,
                                    pal.accent_emerald,
                                    s.cleaner_empty_clean_title,
                                    s.cleaner_empty_clean_desc,
                                );
                            }

                            let q = self.cleaner_search_query.trim().to_lowercase();

                            for res in non_empty {
                                let matching_files: Vec<&ScannedFileItem> = if q.is_empty() {
                                    res.files.iter().collect()
                                } else {
                                    res.files
                                        .iter()
                                        .filter(|f| {
                                            f.path.to_string_lossy().to_lowercase().contains(&q)
                                        })
                                        .collect()
                                };

                                if !q.is_empty() && matching_files.is_empty() {
                                    continue;
                                }

                                let ratio = if summary.total_bytes_found > 0 {
                                    (res.total_bytes as f32 / summary.total_bytes_found as f32)
                                        .clamp(0.0, 1.0)
                                } else {
                                    0.0
                                };
                                let disp_name = current_lang
                                    .localize_rule(res.rule_id)
                                    .map(|(n, _)| n)
                                    .unwrap_or(&res.rule_name);

                                egui::Frame::new()
                                    .fill(pal.bg_card_subtle)
                                    .stroke(Stroke::new(1.0_f32, pal.border_subtle))
                                    .corner_radius(CornerRadius::same(8))
                                    .inner_margin(egui::Margin::symmetric(10, 7))
                                    .show(ui, |ui| {
                                        ui.set_width(ui.available_width());
                                        ui.horizontal(|ui| {
                                            ui.label(
                                                RichText::new(disp_name)
                                                    .size(13.0)
                                                    .strong()
                                                    .color(pal.text_primary),
                                            );
                                            ui.with_layout(
                                                egui::Layout::right_to_left(egui::Align::Center),
                                                |ui| {
                                                    draw_pill_badge(
                                                        ui,
                                                        &format_bytes(res.total_bytes),
                                                        Color32::from_rgba_unmultiplied(
                                                            pal.accent_cyan.r(),
                                                            pal.accent_cyan.g(),
                                                            pal.accent_cyan.b(),
                                                            32,
                                                        ),
                                                        pal.accent_cyan,
                                                    );
                                                    let count_label = if q.is_empty() {
                                                        format!("{} ({})", res.files.len(), res.locked_count)
                                                    } else {
                                                        format!("{}/{} ({})", matching_files.len(), res.files.len(), res.locked_count)
                                                    };
                                                    ui.label(
                                                        RichText::new(count_label)
                                                            .size(11.0)
                                                            .monospace()
                                                            .color(pal.text_secondary),
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
                                            pal.bg_track,
                                        );
                                        let mut fill_r = p_rect;
                                        fill_r.set_width(p_rect.width() * ratio);
                                        ui.painter().rect_filled(
                                            fill_r,
                                            CornerRadius::same(2),
                                            pal.accent_cyan,
                                        );

                                        ui.collapsing(
                                            RichText::new(s.cleaner_show_files)
                                                .size(11.0)
                                                .color(pal.text_muted),
                                            |ui| {
                                                for f in matching_files.iter().take(50) {
                                                    ui.horizontal(|ui| {
                                                        if f.locked_or_denied {
                                                            draw_pill_badge(
                                                                ui,
                                                                s.badge_in_use,
                                                                Color32::from_rgba_unmultiplied(
                                                                    pal.accent_amber.r(),
                                                                    pal.accent_amber.g(),
                                                                    pal.accent_amber.b(),
                                                                    28,
                                                                ),
                                                                pal.accent_amber,
                                                            );
                                                        } else {
                                                            draw_pill_badge(
                                                                ui,
                                                                s.badge_ready_ok,
                                                                Color32::from_rgba_unmultiplied(
                                                                    pal.accent_emerald.r(),
                                                                    pal.accent_emerald.g(),
                                                                    pal.accent_emerald.b(),
                                                                    25,
                                                                ),
                                                                pal.accent_emerald,
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
                                                                    .color(pal.text_secondary),
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
                        pal,
                        VectorIcon::CleanerSweep,
                        pal.accent_cyan,
                        s.cleaner_ready_title,
                        s.cleaner_ready_desc,
                    );
                }
            });
        });
    }

    fn start_cleaner_job(&mut self, delete_files: bool) {
        let s = self.language.strings();
        self.cleaner_busy = true;
        self.status_banner = s.cleaner_scanning_msg.to_string();
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

    fn start_reg_scan(&mut self) {
        let s = self.language.strings();
        self.reg_busy = true;
        self.status_banner = s.reg_scanning_msg.to_string();
        let active_cats: Vec<RegistryCategory> = self
            .reg_categories
            .iter()
            .filter(|(_, en)| *en)
            .map(|(c, _)| *c)
            .collect();
        let rx = Arc::clone(&self.reg_scan_rx);
        thread::spawn(move || {
            let issues = scan_registry(&active_cats);
            if let Ok(mut g) = rx.lock() {
                *g = Some(issues);
            }
        });
    }

    fn start_reg_fix(&mut self) {
        let s = self.language.strings();
        self.reg_busy = true;
        self.status_banner = s.reg_fixing_msg.to_string();
        let issues_clone = self.reg_issues.clone();
        let backup_dir = self.reg_backup_dir.clone();
        let active_cats: Vec<RegistryCategory> = self
            .reg_categories
            .iter()
            .filter(|(_, en)| *en)
            .map(|(c, _)| *c)
            .collect();
        let rx = Arc::clone(&self.reg_fix_rx);
        thread::spawn(move || {
            let fix_res = backup_and_fix_registry(&issues_clone, &backup_dir);
            let post_scan = if fix_res.is_ok() {
                Some(scan_registry(&active_cats))
            } else {
                None
            };
            if let Ok(mut g) = rx.lock() {
                *g = Some((fix_res, post_scan));
            }
        });
    }

    fn render_registry_tab(&mut self, ui: &mut egui::Ui, pal: &ThemePalette) {
        let s = self.language.strings();

        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.label(
                    RichText::new(s.reg_title)
                        .size(19.0)
                        .strong()
                        .color(pal.text_primary),
                );
                ui.label(
                    RichText::new(s.reg_subtitle)
                        .size(12.0)
                        .color(pal.text_secondary),
                );
            });

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if draw_action_button(
                    ui,
                    pal,
                    s.btn_backup_fix_reg,
                    VectorIcon::CheckShield,
                    ButtonVariant::ExecuteBrand,
                    !self.reg_busy && !self.reg_issues.is_empty(),
                ) {
                    self.start_reg_fix();
                }

                if draw_action_button(
                    ui,
                    pal,
                    s.btn_export_reg,
                    VectorIcon::DriveCylinder,
                    ButtonVariant::Secondary,
                    !self.reg_busy && !self.reg_issues.is_empty(),
                ) {
                    match export_reg_backup(&self.reg_issues, &self.reg_backup_dir) {
                        Ok(path) => {
                            self.status_banner =
                                format!("{} {}", s.reg_last_backup_prefix, path.display());
                            self.reg_last_backup = Some(path);
                        }
                        Err(e) => self.status_banner = e,
                    }
                }

                if draw_action_button(
                    ui,
                    pal,
                    s.btn_scan_reg,
                    VectorIcon::SearchLens,
                    ButtonVariant::PrimaryScan,
                    !self.reg_busy,
                ) {
                    self.start_reg_scan();
                }
            });
        });

        ui.add_space(8.0);

        card_frame(pal).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal_wrapped(|ui| {
                ui.label(
                    RichText::new(s.reg_hive_categories)
                        .size(12.0)
                        .strong()
                        .color(pal.text_secondary),
                );
                for (cat, enabled) in &mut self.reg_categories {
                    let (cat_title, cat_desc) = self.language.localize_reg_category(*cat);
                    let cb = ui.checkbox(enabled, cat_title);
                    if cb.hovered() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    }
                    cb.on_hover_text(format!("{} [{}]", cat_desc, cat.code()));
                }
            });
            if let Some(last_bak) = &self.reg_last_backup {
                ui.add_space(4.0);
                ui.label(
                    RichText::new(format!(
                        "{} {}",
                        s.reg_last_backup_prefix,
                        last_bak.display()
                    ))
                    .size(11.5)
                    .monospace()
                    .color(pal.accent_emerald),
                );
            }
        });

        ui.add_space(6.0);

        if !self.reg_issues.is_empty() {
            card_frame(pal).show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    let selected_count = self.reg_issues.iter().filter(|i| i.selected).count();
                    draw_pill_badge(
                        ui,
                        &format!("{}/{}", selected_count, self.reg_issues.len()),
                        Color32::from_rgba_unmultiplied(
                            pal.accent_amber.r(),
                            pal.accent_amber.g(),
                            pal.accent_amber.b(),
                            32,
                        ),
                        pal.accent_amber,
                    );

                    let all_btn = egui::Button::new(
                        RichText::new(s.btn_select_all).size(11.0).color(pal.text_secondary),
                    )
                    .fill(pal.bg_card_subtle)
                    .stroke(Stroke::new(1.0_f32, pal.border_subtle))
                    .corner_radius(CornerRadius::same(12));
                    let resp_all = ui.add(all_btn);
                    if resp_all.hovered() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    }
                    if resp_all.clicked() {
                        for issue in &mut self.reg_issues {
                            issue.selected = true;
                        }
                    }

                    let none_btn = egui::Button::new(
                        RichText::new(s.btn_deselect_all).size(11.0).color(pal.text_secondary),
                    )
                    .fill(pal.bg_card_subtle)
                    .stroke(Stroke::new(1.0_f32, pal.border_subtle))
                    .corner_radius(CornerRadius::same(12));
                    let resp_none = ui.add(none_btn);
                    if resp_none.hovered() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    }
                    if resp_none.clicked() {
                        for issue in &mut self.reg_issues {
                            issue.selected = false;
                        }
                    }

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.add(
                            egui::TextEdit::singleline(&mut self.reg_filter)
                                .hint_text(s.lbl_search_filter)
                                .desired_width(220.0),
                        );
                    });
                });
            });
            ui.add_space(6.0);
        }

        let list_h = (ui.available_height() - 10.0).max(220.0);
        let q_reg = self.reg_filter.trim().to_lowercase();

        card_frame(pal).show(ui, |ui| {
            ui.set_height(list_h);
            egui::ScrollArea::vertical()
                .id_salt("reg_issues_scroll")
                .max_height((ui.available_height() - 6.0).max(180.0))
                .show(ui, |ui| {
                    if self.reg_busy {
                        ui.vertical_centered(|ui| {
                            ui.add_space(40.0);
                            ui.spinner();
                            ui.add_space(10.0);
                            ui.label(
                                RichText::new(s.reg_scanning_msg)
                                    .size(13.5)
                                    .color(pal.text_secondary),
                            );
                        });
                    } else if self.reg_issues.is_empty() {
                        draw_empty_state(
                            ui,
                            pal,
                            VectorIcon::RegistryGrid,
                            pal.accent_amber,
                            s.reg_empty_title,
                            s.reg_empty_desc,
                        );
                    } else {
                        for issue in &mut self.reg_issues {
                            if !q_reg.is_empty()
                                && !issue.problem_description.to_lowercase().contains(&q_reg)
                                && !issue.data_reference.to_lowercase().contains(&q_reg)
                                && !issue.key_path.to_lowercase().contains(&q_reg)
                                && !issue.category.code().to_lowercase().contains(&q_reg)
                            {
                                continue;
                            }

                            let (cat_title, _) = self.language.localize_reg_category(issue.category);

                            egui::Frame::new()
                                .fill(pal.bg_card_subtle)
                                .stroke(Stroke::new(1.0_f32, pal.border_subtle))
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
                                            Color32::from_rgba_unmultiplied(
                                                pal.accent_rust.r(),
                                                pal.accent_rust.g(),
                                                pal.accent_rust.b(),
                                                32,
                                            ),
                                            pal.accent_rust,
                                        );
                                        draw_pill_badge(
                                            ui,
                                            cat_title,
                                            Color32::from_rgba_unmultiplied(
                                                pal.accent_cyan.r(),
                                                pal.accent_cyan.g(),
                                                pal.accent_cyan.b(),
                                                24,
                                            ),
                                            pal.accent_cyan,
                                        );
                                        ui.label(
                                            RichText::new(&issue.problem_description)
                                                .size(13.0)
                                                .strong()
                                                .color(pal.text_primary),
                                        );
                                        ui.label(
                                            RichText::new(&issue.data_reference)
                                                .size(12.0)
                                                .monospace()
                                                .color(pal.accent_cyan),
                                        );
                                    });
                                    let reg_p = issue.full_reg_path();
                                    ui.add(
                                        egui::Label::new(
                                            RichText::new(format!("{} {reg_p}", s.reg_path_prefix))
                                                .size(11.0)
                                                .monospace()
                                                .color(pal.text_muted),
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

    fn render_startup_tab(&mut self, ui: &mut egui::Ui, pal: &ThemePalette) {
        let s = self.language.strings();
        let active_count = self.startup_entries.iter().filter(|e| e.enabled).count();
        let disabled_count = self.startup_entries.len().saturating_sub(active_count);

        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.label(
                    RichText::new(s.startup_title)
                        .size(19.0)
                        .strong()
                        .color(pal.text_primary),
                );
                ui.label(
                    RichText::new(s.startup_subtitle)
                        .size(12.0)
                        .color(pal.text_secondary),
                );
            });

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if draw_action_button(
                    ui,
                    pal,
                    s.btn_refresh_list,
                    VectorIcon::RefreshSync,
                    ButtonVariant::Secondary,
                    true,
                ) {
                    self.startup_entries = list_startup_entries();
                }
            });
        });

        ui.add_space(8.0);

        card_frame(pal).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                draw_pill_badge(
                    ui,
                    &format!("{}: {}", s.badge_total, self.startup_entries.len()),
                    Color32::from_rgba_unmultiplied(
                        pal.accent_cyan.r(),
                        pal.accent_cyan.g(),
                        pal.accent_cyan.b(),
                        28,
                    ),
                    pal.accent_cyan,
                );
                draw_pill_badge(
                    ui,
                    &format!("{}: {active_count}", s.badge_active),
                    Color32::from_rgba_unmultiplied(
                        pal.accent_emerald.r(),
                        pal.accent_emerald.g(),
                        pal.accent_emerald.b(),
                        28,
                    ),
                    pal.accent_emerald,
                );
                draw_pill_badge(
                    ui,
                    &format!("{}: {disabled_count}", s.badge_disabled),
                    Color32::from_rgba_unmultiplied(
                        pal.accent_amber.r(),
                        pal.accent_amber.g(),
                        pal.accent_amber.b(),
                        28,
                    ),
                    pal.accent_amber,
                );
                ui.separator();
                ui.label(RichText::new(s.lbl_filter).color(pal.text_secondary));
                ui.add(
                    egui::TextEdit::singleline(&mut self.startup_filter)
                        .hint_text(s.startup_filter_hint)
                        .desired_width(250.0),
                );
            });
        });

        ui.add_space(8.0);
        let filter_low = self.startup_filter.to_lowercase();
        let mut toggle_req: Option<(StartupEntry, bool)> = None;
        let mut delete_req: Option<StartupEntry> = None;

        let list_h = (ui.available_height() - 10.0).max(220.0);
        card_frame(pal).show(ui, |ui| {
            ui.set_height(list_h);
            egui::ScrollArea::vertical()
                .id_salt("startup_scroll")
                .max_height((ui.available_height() - 6.0).max(180.0))
                .show(ui, |ui| {
                    for (row_idx, entry) in self.startup_entries.iter().enumerate() {
                        if !filter_low.is_empty()
                            && !entry.name.to_lowercase().contains(&filter_low)
                            && !entry.command.to_lowercase().contains(&filter_low)
                        {
                            continue;
                        }
                        ui.push_id(row_idx, |ui| {
                            egui::Frame::new()
                                .fill(pal.bg_card_subtle)
                                .stroke(Stroke::new(1.0_f32, pal.border_subtle))
                                .corner_radius(CornerRadius::same(8))
                                .inner_margin(egui::Margin::symmetric(12, 9))
                                .show(ui, |ui| {
                                    ui.set_width(ui.available_width());
                                    ui.horizontal(|ui| {
                                        if entry.enabled {
                                            draw_pill_badge(
                                                ui,
                                                s.pill_enabled,
                                                Color32::from_rgba_unmultiplied(
                                                    pal.accent_emerald.r(),
                                                    pal.accent_emerald.g(),
                                                    pal.accent_emerald.b(),
                                                    32,
                                                ),
                                                pal.accent_emerald,
                                            );
                                        } else {
                                            draw_pill_badge(
                                                ui,
                                                s.pill_disabled,
                                                Color32::from_rgba_unmultiplied(
                                                    pal.accent_amber.r(),
                                                    pal.accent_amber.g(),
                                                    pal.accent_amber.b(),
                                                    32,
                                                ),
                                                pal.accent_amber,
                                            );
                                        }
                                        let req_admin = entry.source.requires_admin() && !self.is_admin;
                                        if req_admin {
                                            draw_pill_badge(
                                                ui,
                                                s.badge_admin_required,
                                                Color32::from_rgba_unmultiplied(
                                                    pal.accent_amber.r(),
                                                    pal.accent_amber.g(),
                                                    pal.accent_amber.b(),
                                                    32,
                                                ),
                                                pal.accent_amber,
                                            );
                                        }

                                        draw_pill_badge(
                                            ui,
                                            entry.source.key_label(),
                                            Color32::from_rgba_unmultiplied(
                                                pal.accent_cyan.r(),
                                                pal.accent_cyan.g(),
                                                pal.accent_cyan.b(),
                                                25,
                                            ),
                                            pal.accent_cyan,
                                        );
                                        ui.label(
                                            RichText::new(&entry.name)
                                                .size(14.0)
                                                .strong()
                                                .color(pal.text_primary),
                                        );

                                        ui.with_layout(
                                            egui::Layout::right_to_left(egui::Align::Center),
                                            |ui| {
                                                if draw_action_button(
                                                    ui,
                                                    pal,
                                                    s.btn_delete,
                                                    VectorIcon::TrashDelete,
                                                    ButtonVariant::Danger,
                                                    !req_admin,
                                                ) {
                                                    delete_req = Some(entry.clone());
                                                }
                                                let (toggle_label, variant) = if entry.enabled {
                                                    (s.btn_disable, ButtonVariant::Secondary)
                                                } else {
                                                    (s.btn_enable, ButtonVariant::PrimaryScan)
                                                };
                                                if draw_action_button(
                                                    ui,
                                                    pal,
                                                    toggle_label,
                                                    VectorIcon::StartupBolt,
                                                    variant,
                                                    !req_admin,
                                                ) {
                                                    toggle_req =
                                                        Some((entry.clone(), !entry.enabled));
                                                }
                                            },
                                        );
                                    });
                                    ui.add_space(2.0);
                                    let cmd_str = format!("{} {}", s.lbl_command, entry.command);
                                    ui.add(
                                        egui::Label::new(
                                            RichText::new(&cmd_str)
                                                .size(11.0)
                                                .monospace()
                                                .color(pal.text_muted),
                                        )
                                        .truncate(),
                                    )
                                    .on_hover_text(&entry.command);
                                });
                        });
                        ui.add_space(4.0);
                    }
                });
        });

        if let Some((entry, new_state)) = toggle_req {
            match set_startup_enabled(&entry, new_state) {
                Ok(()) => {
                    self.status_banner = format!(
                        "'{}' -> {}",
                        entry.name,
                        if new_state {
                            s.pill_enabled
                        } else {
                            s.pill_disabled
                        }
                    );
                    self.startup_entries = list_startup_entries();
                }
                Err(e) => self.status_banner = e,
            }
        }
        if let Some(entry) = delete_req {
            match delete_startup_entry(&entry, Some(&self.reg_backup_dir)) {
                Ok(backup_path) => {
                    self.status_banner = format!(
                        "'{}' {} ({}: {}).",
                        entry.name,
                        s.banner_startup_deleted,
                        s.lbl_backup,
                        backup_path.display()
                    );
                    self.startup_entries = list_startup_entries();
                }
                Err(e) => self.status_banner = e,
            }
        }
    }

    fn render_uninstaller_tab(&mut self, ui: &mut egui::Ui, pal: &ThemePalette) {
        if self.residual_summary.is_some() {
            self.render_residual_inspector_view(ui, pal);
            return;
        }

        let s = self.language.strings();
        let res = self.language.residual_hunter();

        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.label(
                    RichText::new(s.uninst_title)
                        .size(19.0)
                        .strong()
                        .color(pal.text_primary),
                );
                ui.label(
                    RichText::new(s.uninst_subtitle)
                        .size(12.0)
                        .color(pal.text_secondary),
                );
            });

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if draw_action_button(
                    ui,
                    pal,
                    s.btn_refresh_list,
                    VectorIcon::RefreshSync,
                    ButtonVariant::Secondary,
                    true,
                ) {
                    self.installed_apps = list_installed_apps();
                }

                if draw_action_button(
                    ui,
                    pal,
                    res.btn_global_orphans,
                    VectorIcon::DiskRadar,
                    ButtonVariant::PrimaryScan,
                    true,
                ) {
                    let names: Vec<String> = self.installed_apps.iter().map(|a| a.display_name.clone()).collect();
                    let items = crate::residual_hunter::scan_global_orphans(&names);
                    let total_bytes: u64 = items.iter().map(|i| i.size_bytes).sum();
                    let total_dirs = items.iter().filter(|i| matches!(i.kind, crate::residual_hunter::ResidualKind::Directory(_))).count();
                    let total_files = items.iter().filter(|i| matches!(i.kind, crate::residual_hunter::ResidualKind::File(_))).count();
                    self.residual_summary = Some(crate::residual_hunter::ResidualScanSummary {
                        target_app_name: res.btn_global_orphans.to_string(),
                        target_publisher: "System AppData & ProgramData".to_string(),
                        items,
                        total_bytes,
                        total_dirs,
                        total_files,
                        total_registry_keys: 0,
                    });
                    self.is_global_orphan_mode = true;
                    self.residual_status_msg = None;
                }
            });
        });

        ui.add_space(8.0);

        card_frame(pal).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                draw_pill_badge(
                    ui,
                    &format!("{} {}", self.installed_apps.len(), s.badge_apps_detected),
                    Color32::from_rgba_unmultiplied(
                        pal.accent_purple.r(),
                        pal.accent_purple.g(),
                        pal.accent_purple.b(),
                        32,
                    ),
                    pal.accent_purple,
                );
                ui.separator();
                ui.label(RichText::new(s.lbl_filter).color(pal.text_secondary));
                ui.add(
                    egui::TextEdit::singleline(&mut self.uninstall_filter)
                        .hint_text(s.uninst_filter_hint)
                        .desired_width(280.0),
                );
            });
        });

        ui.add_space(8.0);
        let filter_low = self.uninstall_filter.to_lowercase();
        let mut launch_req: Option<InstalledApp> = None;
        let mut residual_scan_req: Option<InstalledApp> = None;
        let mut deep_uninstall_req: Option<InstalledApp> = None;

        let list_h = (ui.available_height() - 10.0).max(220.0);
        card_frame(pal).show(ui, |ui| {
            ui.set_height(list_h);
            egui::ScrollArea::vertical()
                .id_salt("uninstaller_scroll")
                .max_height((ui.available_height() - 6.0).max(180.0))
                .show(ui, |ui| {
                    for (row_idx, app) in self.installed_apps.iter().enumerate() {
                        if !filter_low.is_empty()
                            && !app.display_name.to_lowercase().contains(&filter_low)
                            && !app.publisher.to_lowercase().contains(&filter_low)
                        {
                            continue;
                        }
                        ui.push_id(row_idx, |ui| {
                            egui::Frame::new()
                                .fill(pal.bg_card_subtle)
                                .stroke(Stroke::new(1.0_f32, pal.border_subtle))
                                .corner_radius(CornerRadius::same(8))
                                .inner_margin(egui::Margin::symmetric(12, 9))
                                .show(ui, |ui| {
                                    let row_w = ui.available_width();
                                    ui.set_width(row_w);
                                    ui.horizontal(|ui| {
                                        let left_w = (row_w - 410.0).max(150.0);
                                        ui.allocate_ui_with_layout(
                                            egui::vec2(left_w, 38.0),
                                            egui::Layout::top_down(egui::Align::Min),
                                            |ui| {
                                                ui.set_max_width(left_w);
                                                ui.horizontal_wrapped(|ui| {
                                                    ui.label(
                                                        RichText::new(&app.display_name)
                                                            .size(14.0)
                                                            .strong()
                                                            .color(pal.text_primary),
                                                    );
                                                    if !app.display_version.is_empty() {
                                                        draw_pill_badge(
                                                            ui,
                                                            &format!("v{}", app.display_version),
                                                            Color32::from_rgba_unmultiplied(
                                                                pal.accent_cyan.r(),
                                                                pal.accent_cyan.g(),
                                                                pal.accent_cyan.b(),
                                                                28,
                                                            ),
                                                            pal.accent_cyan,
                                                        );
                                                    }
                                                    if let Some(kb) = app.estimated_size_kb {
                                                        draw_pill_badge(
                                                            ui,
                                                            &format!("{:.1} MB", kb as f64 / 1024.0),
                                                            Color32::from_rgba_unmultiplied(
                                                                pal.accent_emerald.r(),
                                                                pal.accent_emerald.g(),
                                                                pal.accent_emerald.b(),
                                                                28,
                                                            ),
                                                            pal.accent_emerald,
                                                        );
                                                    }
                                                    if !app.publisher.is_empty() {
                                                        ui.label(
                                                            RichText::new(format!(
                                                                "• {}",
                                                                app.publisher
                                                            ))
                                                            .size(11.5)
                                                            .color(pal.text_secondary),
                                                        );
                                                    }
                                                });
                                                let u_cmd = format!(
                                                    "{} {}",
                                                    s.lbl_command, app.uninstall_string
                                                );
                                                ui.add(
                                                    egui::Label::new(
                                                        RichText::new(&u_cmd)
                                                            .size(10.5)
                                                            .monospace()
                                                            .color(pal.text_muted),
                                                    )
                                                    .truncate(),
                                                )
                                                .on_hover_text(&app.uninstall_string);
                                            },
                                        );

                                        ui.with_layout(
                                            egui::Layout::right_to_left(egui::Align::Center),
                                            |ui| {
                                                if draw_action_button(
                                                    ui,
                                                    pal,
                                                    s.btn_uninstall,
                                                    VectorIcon::UninstallerBox,
                                                    ButtonVariant::Secondary,
                                                    true,
                                                ) {
                                                    launch_req = Some(app.clone());
                                                }

                                                if draw_action_button(
                                                    ui,
                                                    pal,
                                                    res.btn_scan_residuals,
                                                    VectorIcon::SearchLens,
                                                    ButtonVariant::Secondary,
                                                    true,
                                                ) {
                                                    residual_scan_req = Some(app.clone());
                                                }

                                                if draw_action_button(
                                                    ui,
                                                    pal,
                                                    res.btn_deep_uninstall,
                                                    VectorIcon::TrashDelete,
                                                    ButtonVariant::Danger,
                                                    true,
                                                ) {
                                                    deep_uninstall_req = Some(app.clone());
                                                }
                                            },
                                        );
                                    });
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

        if let Some(app) = residual_scan_req {
            let summary = crate::residual_hunter::scan_application_residuals(
                &app.display_name,
                &app.publisher,
                None,
                &app.reg_key_name,
            );
            self.residual_summary = Some(summary);
            self.is_global_orphan_mode = false;
            self.residual_status_msg = None;
        }

        if let Some(app) = deep_uninstall_req {
            let _ = launch_uninstaller(&app);
            let summary = crate::residual_hunter::scan_application_residuals(
                &app.display_name,
                &app.publisher,
                None,
                &app.reg_key_name,
            );
            self.residual_summary = Some(summary);
            self.is_global_orphan_mode = false;
            self.residual_status_msg = Some(format!(
                "Avviato disinstallatore standard. Esamina di seguito tutti i residui su disco e registro lasciati da '{}'.",
                app.display_name
            ));
        }
    }

    fn render_residual_inspector_view(&mut self, ui: &mut egui::Ui, pal: &ThemePalette) {
        let res = self.language.residual_hunter();
        let s = self.language.strings();
        let Some(summary) = &mut self.residual_summary else { return; };
        let mut close_inspector = false;

        // 1. Header with Back Button and Title
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(res.title_residual_hunter)
                            .size(19.0)
                            .strong()
                            .color(pal.text_primary),
                    );
                    draw_pill_badge(
                        ui,
                        &summary.target_app_name,
                        Color32::from_rgba_unmultiplied(
                            pal.accent_purple.r(),
                            pal.accent_purple.g(),
                            pal.accent_purple.b(),
                            32,
                        ),
                        pal.accent_purple,
                    );
                });
                ui.label(
                    RichText::new(res.sub_residual_hunter)
                        .size(12.0)
                        .color(pal.text_secondary),
                );
            });

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if draw_action_button(
                    ui,
                    pal,
                    res.btn_back_to_apps,
                    VectorIcon::RefreshSync,
                    ButtonVariant::Secondary,
                    true,
                ) {
                    close_inspector = true;
                }
            });
        });

        ui.add_space(8.0);

        // 2. Status message banner if any
        if let Some(msg) = &self.residual_status_msg {
            let is_err = msg.contains("Impossibile") || msg.contains("Errore");
            let banner_color = if is_err { pal.accent_rust } else { pal.accent_emerald };
            egui::Frame::new()
                .fill(Color32::from_rgba_unmultiplied(banner_color.r(), banner_color.g(), banner_color.b(), 24))
                .stroke(Stroke::new(1.0, banner_color))
                .corner_radius(CornerRadius::same(6))
                .inner_margin(egui::Margin::symmetric(12, 8))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.label(RichText::new(msg).color(banner_color).strong());
                });
            ui.add_space(6.0);
        }

        // 3. KPI Cards
        let selected_count = summary.items.iter().filter(|i| i.selected).count();
        let selected_bytes: u64 = summary.items.iter().filter(|i| i.selected).map(|i| i.size_bytes).sum();
        let reg_count = summary.items.iter().filter(|i| i.selected && matches!(i.kind, crate::residual_hunter::ResidualKind::RegistryKey { .. } | crate::residual_hunter::ResidualKind::RegistryValue { .. })).count();

        ui.horizontal(|ui| {
            let card_w = (ui.available_width() - 16.0) / 3.0;

            // Card 1: Elementi Residui
            card_frame(pal).show(ui, |ui| {
                ui.set_width(card_w);
                ui.vertical(|ui| {
                    ui.label(RichText::new(res.kpi_items_found).size(11.0).color(pal.text_muted).strong());
                    ui.label(RichText::new(format!("{selected_count} / {}", summary.items.len())).size(20.0).color(pal.accent_cyan).strong());
                });
            });

            // Card 2: Spazio Recuperabile
            card_frame(pal).show(ui, |ui| {
                ui.set_width(card_w);
                ui.vertical(|ui| {
                    ui.label(RichText::new(res.kpi_space_reclaimable).size(11.0).color(pal.text_muted).strong());
                    ui.label(RichText::new(crate::cleaner::format_bytes(selected_bytes)).size(20.0).color(pal.accent_emerald).strong());
                });
            });

            // Card 3: Chiavi Registro
            card_frame(pal).show(ui, |ui| {
                ui.set_width(card_w);
                ui.vertical(|ui| {
                    ui.label(RichText::new(res.kpi_reg_keys).size(11.0).color(pal.text_muted).strong());
                    ui.label(RichText::new(format!("{reg_count}")).size(20.0).color(pal.accent_purple).strong());
                });
            });
        });

        ui.add_space(8.0);

        // 4. Action bar with search filter, selection buttons, and Clean button
        card_frame(pal).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(RichText::new(s.lbl_filter).color(pal.text_secondary));
                ui.add(
                    egui::TextEdit::singleline(&mut self.residual_filter)
                        .hint_text(res.filter_placeholder)
                        .desired_width(220.0),
                );

                ui.separator();

                if ui.button(res.btn_select_all).clicked() {
                    for item in &mut summary.items {
                        item.selected = true;
                    }
                }
                if ui.button(res.btn_deselect_all).clicked() {
                    for item in &mut summary.items {
                        item.selected = false;
                    }
                }
                if ui.button(res.btn_select_safe).clicked() {
                    for item in &mut summary.items {
                        item.selected = item.confidence == crate::residual_hunter::ConfidenceLevel::High;
                    }
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let can_clean = selected_count > 0;
                    if draw_action_button(
                        ui,
                        pal,
                        res.btn_clean_selected,
                        VectorIcon::TrashDelete,
                        ButtonVariant::Danger,
                        can_clean,
                    ) {
                        let backup_dir = env::current_dir()
                            .unwrap_or_else(|_| PathBuf::from("."))
                            .join("registry_backups");
                        let report = crate::residual_hunter::delete_selected_residuals(&summary.items, &backup_dir);
                        
                        let backup_note = if let Some(p) = &report.reg_backup_path {
                            format!(" | Backup .reg: {}", p.file_name().unwrap_or_default().to_string_lossy())
                        } else {
                            String::new()
                        };

                        self.residual_status_msg = Some(format!(
                            "Eliminati {} elementi ({}) e {} chiavi di registro.{}",
                            report.deleted_items,
                            crate::cleaner::format_bytes(report.freed_bytes),
                            report.deleted_reg_keys,
                            backup_note
                        ));

                        // Remove deleted items from summary
                        summary.items.retain(|i| !i.selected);
                        summary.total_bytes = summary.items.iter().map(|i| i.size_bytes).sum();
                    }
                });
            });
        });

        ui.add_space(8.0);

        // 5. Residuals List
        let filter_low = self.residual_filter.to_lowercase();
        let list_h = (ui.available_height() - 10.0).max(200.0);

        card_frame(pal).show(ui, |ui| {
            ui.set_height(list_h);
            if summary.items.is_empty() {
                ui.vertical_centered(|ui| {
                    ui.add_space(30.0);
                    ui.label(RichText::new(res.empty_title).size(16.0).strong().color(pal.text_primary));
                    ui.add_space(4.0);
                    ui.label(RichText::new(res.empty_desc).size(12.0).color(pal.text_secondary));
                });
            } else {
                egui::ScrollArea::vertical()
                    .id_salt("residual_inspector_scroll")
                    .show(ui, |ui| {
                        for item in summary.items.iter_mut() {
                            if !filter_low.is_empty()
                                && !item.display_path.to_lowercase().contains(&filter_low)
                                && !item.reason.to_lowercase().contains(&filter_low)
                            {
                                continue;
                            }

                            ui.push_id(item.id, |ui| {
                                egui::Frame::new()
                                    .fill(pal.bg_card_subtle)
                                    .stroke(Stroke::new(1.0, pal.border_subtle))
                                    .corner_radius(CornerRadius::same(6))
                                    .inner_margin(egui::Margin::symmetric(10, 8))
                                    .show(ui, |ui| {
                                        ui.horizontal(|ui| {
                                            ui.checkbox(&mut item.selected, "");

                                            // Kind Badge
                                            match &item.kind {
                                                crate::residual_hunter::ResidualKind::Directory(_) => {
                                                    draw_pill_badge(ui, "CARTELLA", Color32::from_rgba_unmultiplied(pal.accent_cyan.r(), pal.accent_cyan.g(), pal.accent_cyan.b(), 32), pal.accent_cyan);
                                                }
                                                crate::residual_hunter::ResidualKind::File(_) => {
                                                    draw_pill_badge(ui, "FILE", Color32::from_rgba_unmultiplied(pal.accent_amber.r(), pal.accent_amber.g(), pal.accent_amber.b(), 32), pal.accent_amber);
                                                }
                                                crate::residual_hunter::ResidualKind::RegistryKey { .. } | crate::residual_hunter::ResidualKind::RegistryValue { .. } => {
                                                    draw_pill_badge(ui, "REGISTRO", Color32::from_rgba_unmultiplied(pal.accent_purple.r(), pal.accent_purple.g(), pal.accent_purple.b(), 32), pal.accent_purple);
                                                }
                                            }

                                            // Size if file/dir
                                            if item.size_bytes > 0 {
                                                draw_pill_badge(ui, &crate::cleaner::format_bytes(item.size_bytes), Color32::from_rgba_unmultiplied(pal.accent_emerald.r(), pal.accent_emerald.g(), pal.accent_emerald.b(), 28), pal.accent_emerald);
                                            }

                                            // Confidence badge
                                            let (conf_bg, conf_fg) = match item.confidence {
                                                crate::residual_hunter::ConfidenceLevel::High => (pal.accent_emerald, pal.accent_emerald),
                                                crate::residual_hunter::ConfidenceLevel::Medium => (pal.accent_amber, pal.accent_amber),
                                                crate::residual_hunter::ConfidenceLevel::Low => (pal.text_muted, pal.text_muted),
                                            };
                                            draw_pill_badge(ui, item.confidence.label(), Color32::from_rgba_unmultiplied(conf_bg.r(), conf_bg.g(), conf_bg.b(), 28), conf_fg);

                                            ui.vertical(|ui| {
                                                ui.label(RichText::new(&item.display_path).size(12.0).monospace().strong().color(pal.text_primary));
                                                ui.label(RichText::new(&item.reason).size(11.0).color(pal.text_secondary));
                                            });
                                        });
                                    });
                            });
                            ui.add_space(4.0);
                        }
                    });
            }
        });

        if close_inspector {
            self.residual_summary = None;
            self.residual_status_msg = None;
        }
    }

    fn render_decrapifier_tab(&mut self, ui: &mut egui::Ui, pal: &ThemePalette) {
        let dec = self.language.decrapifier();

        // 1. Header with Title, Subtitle, and Global Action Buttons
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.label(
                    RichText::new(dec.page_title)
                        .size(19.0)
                        .strong()
                        .color(pal.text_primary),
                );
                ui.label(
                    RichText::new(dec.page_subtitle)
                        .size(12.0)
                        .color(pal.text_secondary),
                );
            });

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                // Restore Stock Defaults button
                if draw_action_button(
                    ui,
                    pal,
                    dec.btn_restore_defaults,
                    VectorIcon::RefreshSync,
                    ButtonVariant::Secondary,
                    true,
                ) {
                    match crate::decrapifier::restore_all_windows_defaults() {
                        Ok(count) => {
                            self.decrap_status_msg = Some(format!("{} ({} impostazioni ripristinate)", dec.status_restored, count));
                            self.decrap_tweaks_state = crate::decrapifier::REGISTRY_TWEAKS
                                .iter()
                                .map(|t| (t.id.to_string(), crate::decrapifier::is_tweak_active(t)))
                                .collect();
                        }
                        Err(e) => {
                            self.decrap_status_msg = Some(format!("Errore ripristino default: {e}"));
                        }
                    }
                }

                // Apply Recommended button
                if draw_action_button(
                    ui,
                    pal,
                    dec.btn_apply_recommended,
                    VectorIcon::CheckShield,
                    ButtonVariant::PrimaryScan,
                    true,
                ) {
                    let res = crate::decrapifier::apply_all_recommended_tweaks();
                    let total_active = res.applied_count + res.already_active_count;
                    if res.failed_tweaks.is_empty() {
                        self.decrap_status_msg = Some(format!(
                            "{} ({} ottimizzazioni attive su {})",
                            dec.status_applied, total_active, crate::decrapifier::REGISTRY_TWEAKS.len()
                        ));
                    } else {
                        let needs_admin = res.failed_tweaks.iter().any(|(_, e)| e.contains("Amministratore") || e.contains("negato"));
                        if needs_admin && !self.is_admin {
                            self.decrap_status_msg = Some(format!(
                                "Applicate {} ottimizzazioni. I restanti {} tweak di sistema HKLM richiedono l'avvio come Amministratore.",
                                total_active, res.failed_tweaks.len()
                            ));
                        } else {
                            self.decrap_status_msg = Some(format!(
                                "Applicate {} ottimizzazioni ({} non applicate: {})",
                                total_active, res.failed_tweaks.len(), res.failed_tweaks[0].1
                            ));
                        }
                    }
                    self.decrap_tweaks_state = crate::decrapifier::REGISTRY_TWEAKS
                        .iter()
                        .map(|t| (t.id.to_string(), crate::decrapifier::is_tweak_active(t)))
                        .collect();
                }
            });
        });

        ui.add_space(8.0);

        // 2. Status message banner if any
        if let Some(msg) = &self.decrap_status_msg {
            let is_err = msg.contains("Impossibile") || msg.contains("Errore");
            let banner_color = if is_err { pal.accent_rust } else { pal.accent_emerald };
            egui::Frame::new()
                .fill(Color32::from_rgba_unmultiplied(banner_color.r(), banner_color.g(), banner_color.b(), 24))
                .stroke(Stroke::new(1.0, banner_color))
                .corner_radius(CornerRadius::same(6))
                .inner_margin(egui::Margin::symmetric(12, 8))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.label(RichText::new(msg).color(banner_color).strong());
                });
            ui.add_space(6.0);
        }

        // 3. KPI Cards
        let active_count = self.decrap_tweaks_state.iter().filter(|(_, active)| *active).count();
        let total_tweaks = crate::decrapifier::REGISTRY_TWEAKS.len();
        let privacy_ratio = active_count as f32 / total_tweaks as f32;

        ui.horizontal(|ui| {
            let card_w = (ui.available_width() - 16.0) / 3.0;

            // Card 1: Active Tweaks
            card_frame(pal).show(ui, |ui| {
                ui.set_width(card_w);
                ui.vertical(|ui| {
                    ui.label(RichText::new(dec.kpi_tweaks_active).size(11.0).color(pal.text_muted).strong());
                    ui.label(RichText::new(format!("{active_count} / {total_tweaks}")).size(20.0).color(pal.accent_cyan).strong());
                });
            });

            // Card 2: Privacy Status
            card_frame(pal).show(ui, |ui| {
                ui.set_width(card_w);
                ui.vertical(|ui| {
                    ui.label(RichText::new(dec.kpi_privacy_status).size(11.0).color(pal.text_muted).strong());
                    let status_str = if privacy_ratio >= 0.8 {
                        "MASSIMA (FORTEZZA)"
                    } else if privacy_ratio >= 0.4 {
                        "MODERATA"
                    } else {
                        "BASE (TELEMETRIA ATTIVA)"
                    };
                    let status_color = if privacy_ratio >= 0.8 {
                        pal.accent_emerald
                    } else if privacy_ratio >= 0.4 {
                        pal.accent_amber
                    } else {
                        pal.accent_rust
                    };
                    ui.label(RichText::new(status_str).size(15.0).color(status_color).strong());
                });
            });

            // Card 3: Bloatware Detected
            card_frame(pal).show(ui, |ui| {
                ui.set_width(card_w);
                ui.vertical(|ui| {
                    ui.label(RichText::new(dec.kpi_bloatware_found).size(11.0).color(pal.text_muted).strong());
                    let bloat_str = if self.decrap_appx_scanned {
                        format!("{} rilevati", self.decrap_appx_packages.len())
                    } else {
                        "Non scansionato".to_string()
                    };
                    ui.label(RichText::new(bloat_str).size(16.0).color(pal.accent_purple).strong());
                });
            });
        });

        ui.add_space(8.0);

        // 4. Subtab Switcher
        card_frame(pal).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                let tweaks_active = self.decrap_subtab == DecrapSubtab::Tweaks;
                if ui.selectable_label(tweaks_active, RichText::new(dec.tab_tweaks).strong()).clicked() {
                    self.decrap_subtab = DecrapSubtab::Tweaks;
                }

                let appx_active = self.decrap_subtab == DecrapSubtab::Appx;
                if ui.selectable_label(appx_active, RichText::new(dec.tab_appx).strong()).clicked() {
                    self.decrap_subtab = DecrapSubtab::Appx;
                    if !self.decrap_appx_scanned && !self.decrap_appx_busy {
                        self.decrap_appx_busy = true;
                        let rx = Arc::clone(&self.decrap_appx_rx);
                        std::thread::spawn(move || {
                            let pkgs = crate::decrapifier::scan_bloatware_packages();
                            if let Ok(mut g) = rx.lock() {
                                *g = Some(pkgs);
                            }
                        });
                    }
                }

                ui.separator();

                ui.label(RichText::new(self.language.strings().lbl_filter).color(pal.text_secondary));
                ui.add(
                    egui::TextEdit::singleline(&mut self.decrap_filter)
                        .hint_text("Cerca tweak o app...")
                        .desired_width(200.0),
                );
            });
        });

        ui.add_space(8.0);
        let filter_low = self.decrap_filter.to_lowercase();

        // 5. Content View
        match self.decrap_subtab {
            DecrapSubtab::Tweaks => {
                let list_h = (ui.available_height() - 10.0).max(220.0);
                card_frame(pal).show(ui, |ui| {
                    ui.set_height(list_h);
                    egui::ScrollArea::vertical()
                        .id_salt("decrap_tweaks_scroll")
                        .show(ui, |ui| {
                            let categories = [
                                (crate::decrapifier::TweakCategory::PrivacyTelemetry, dec.cat_privacy),
                                (crate::decrapifier::TweakCategory::StartAndExplorer, dec.cat_start),
                                (crate::decrapifier::TweakCategory::SystemAndEdge, dec.cat_edge),
                            ];

                            for (cat, cat_name) in categories {
                                let tweaks_in_cat: Vec<&crate::decrapifier::RegistryTweakDef> = crate::decrapifier::REGISTRY_TWEAKS
                                    .iter()
                                    .filter(|t| t.category == cat)
                                    .collect();

                                if tweaks_in_cat.is_empty() {
                                    continue;
                                }

                                ui.label(
                                    RichText::new(cat_name)
                                        .size(13.5)
                                        .strong()
                                        .color(pal.accent_cyan),
                                );
                                ui.add_space(4.0);

                                for tweak in tweaks_in_cat {
                                    if !filter_low.is_empty()
                                        && !tweak.name_it.to_lowercase().contains(&filter_low)
                                        && !tweak.desc_it.to_lowercase().contains(&filter_low)
                                    {
                                        continue;
                                    }

                                    let mut is_active = self
                                        .decrap_tweaks_state
                                        .iter()
                                        .find(|(id, _)| id == tweak.id)
                                        .map(|(_, a)| *a)
                                        .unwrap_or(false);

                                    ui.push_id(tweak.id, |ui| {
                                        egui::Frame::new()
                                            .fill(pal.bg_card_subtle)
                                            .stroke(Stroke::new(1.0, pal.border_subtle))
                                            .corner_radius(CornerRadius::same(6))
                                            .inner_margin(egui::Margin::symmetric(12, 9))
                                            .show(ui, |ui| {
                                                ui.horizontal(|ui| {
                                                    let toggle_resp = ui.checkbox(&mut is_active, "");
                                                    if toggle_resp.changed() {
                                                        if is_active {
                                                            if let Err(e) = crate::decrapifier::apply_tweak(tweak) {
                                                                self.decrap_status_msg = Some(format!("Attenzione: {e}"));
                                                            } else {
                                                                self.decrap_status_msg = Some(format!("Tweak '{}' attivato.", tweak.name_it));
                                                            }
                                                        } else {
                                                            let _ = crate::decrapifier::revert_tweak(tweak);
                                                            self.decrap_status_msg = Some(format!("Ripristinato valore predefinito per '{}'.", tweak.name_it));
                                                        }
                                                        let actual = crate::decrapifier::is_tweak_active(tweak);
                                                        if let Some(entry) = self.decrap_tweaks_state.iter_mut().find(|(id, _)| id == tweak.id) {
                                                            entry.1 = actual;
                                                        }
                                                    }

                                                    ui.vertical(|ui| {
                                                        ui.horizontal(|ui| {
                                                            ui.label(
                                                                RichText::new(tweak.name_it)
                                                                    .size(13.5)
                                                                    .strong()
                                                                    .color(pal.text_primary),
                                                            );

                                                            if tweak.recommended {
                                                                draw_pill_badge(
                                                                    ui,
                                                                    dec.badge_recommended,
                                                                    Color32::from_rgba_unmultiplied(
                                                                        pal.accent_emerald.r(),
                                                                        pal.accent_emerald.g(),
                                                                        pal.accent_emerald.b(),
                                                                        28,
                                                                    ),
                                                                    pal.accent_emerald,
                                                                );
                                                            }

                                                            if is_active {
                                                                draw_pill_badge(
                                                                    ui,
                                                                    dec.badge_active,
                                                                    Color32::from_rgba_unmultiplied(
                                                                        pal.accent_cyan.r(),
                                                                        pal.accent_cyan.g(),
                                                                        pal.accent_cyan.b(),
                                                                        28,
                                                                    ),
                                                                    pal.accent_cyan,
                                                                );
                                                            } else {
                                                                draw_pill_badge(
                                                                    ui,
                                                                    dec.badge_default,
                                                                    Color32::from_rgba_unmultiplied(
                                                                        pal.text_muted.r(),
                                                                        pal.text_muted.g(),
                                                                        pal.text_muted.b(),
                                                                        20,
                                                                    ),
                                                                    pal.text_muted,
                                                                );
                                                            }
                                                        });

                                                        ui.label(
                                                            RichText::new(tweak.desc_it)
                                                                .size(11.5)
                                                                .color(pal.text_secondary),
                                                        );
                                                    });
                                                });
                                            });
                                    });
                                    ui.add_space(4.0);
                                }
                                ui.add_space(8.0);
                            }
                        });
                });
            }
            DecrapSubtab::Appx => {
                let list_h = (ui.available_height() - 10.0).max(220.0);
                card_frame(pal).show(ui, |ui| {
                    ui.set_height(list_h);
                    if self.decrap_appx_busy {
                        ui.vertical_centered(|ui| {
                            ui.add_space(40.0);
                            ui.spinner();
                            ui.add_space(10.0);
                            ui.label(RichText::new(dec.appx_scanning).color(pal.text_secondary));
                        });
                    } else if !self.decrap_appx_scanned {
                        ui.vertical_centered(|ui| {
                            ui.add_space(40.0);
                            ui.label(RichText::new(dec.appx_not_scanned).size(15.0).strong().color(pal.text_primary));
                            ui.add_space(10.0);
                            if draw_action_button(
                                ui,
                                pal,
                                dec.btn_scan_appx,
                                VectorIcon::SearchLens,
                                ButtonVariant::PrimaryScan,
                                true,
                            ) {
                                self.decrap_appx_busy = true;
                                let rx = Arc::clone(&self.decrap_appx_rx);
                                std::thread::spawn(move || {
                                    let pkgs = crate::decrapifier::scan_bloatware_packages();
                                    if let Ok(mut g) = rx.lock() {
                                        *g = Some(pkgs);
                                    }
                                });
                            }
                        });
                    } else if self.decrap_appx_packages.is_empty() {
                        ui.vertical_centered(|ui| {
                            ui.add_space(40.0);
                            ui.label(RichText::new(dec.appx_none_found).size(15.0).strong().color(pal.accent_emerald));
                        });
                    } else {
                        ui.horizontal(|ui| {
                            let selected_count = self.decrap_appx_packages.iter().filter(|p| p.selected).count();
                            let count_label = dec.appx_count_label
                                .replace("{total}", &self.decrap_appx_packages.len().to_string())
                                .replace("{selected}", &selected_count.to_string());
                            ui.label(RichText::new(count_label).strong().color(pal.text_primary));
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if draw_action_button(
                                    ui,
                                    pal,
                                    dec.btn_remove_selected_appx,
                                    VectorIcon::TrashDelete,
                                    ButtonVariant::Danger,
                                    selected_count > 0,
                                ) {
                                    let mut removed_count = 0;
                                    let mut errors = Vec::new();
                                    for pkg in self.decrap_appx_packages.iter().filter(|p| p.selected) {
                                        match crate::decrapifier::remove_appx_package(&pkg.package_full_name) {
                                            Ok(()) => removed_count += 1,
                                            Err(e) => errors.push(e),
                                        }
                                    }
                                    self.decrap_appx_packages.retain(|p| !p.selected);
                                    if errors.is_empty() {
                                        self.decrap_status_msg = Some(dec.appx_removed_success.replace("{count}", &removed_count.to_string()));
                                    } else {
                                        self.decrap_status_msg = Some(dec.appx_removed_errors
                                            .replace("{count}", &removed_count.to_string())
                                            .replace("{errors}", &errors.join("; ")));
                                    }
                                }
                            });
                        });

                        ui.add_space(6.0);
                        egui::ScrollArea::vertical()
                            .id_salt("decrap_appx_scroll")
                            .show(ui, |ui| {
                                for (idx, pkg) in self.decrap_appx_packages.iter_mut().enumerate() {
                                    if !filter_low.is_empty()
                                        && !pkg.display_name.to_lowercase().contains(&filter_low)
                                        && !pkg.name.to_lowercase().contains(&filter_low)
                                    {
                                        continue;
                                    }

                                    ui.push_id(idx, |ui| {
                                        egui::Frame::new()
                                            .fill(pal.bg_card_subtle)
                                            .stroke(Stroke::new(1.0, pal.border_subtle))
                                            .corner_radius(CornerRadius::same(6))
                                            .inner_margin(egui::Margin::symmetric(10, 8))
                                            .show(ui, |ui| {
                                                ui.horizontal(|ui| {
                                                    ui.checkbox(&mut pkg.selected, "");
                                                    draw_pill_badge(ui, "APPX", Color32::from_rgba_unmultiplied(pal.accent_purple.r(), pal.accent_purple.g(), pal.accent_purple.b(), 28), pal.accent_purple);
                                                    ui.vertical(|ui| {
                                                        ui.label(RichText::new(&pkg.display_name).size(13.0).strong().color(pal.text_primary));
                                                        ui.label(RichText::new(&pkg.name).size(11.0).monospace().color(pal.text_secondary));
                                                    });
                                                });
                                            });
                                    });
                                    ui.add_space(4.0);
                                }
                            });
                    }
                });
            }
        }
    }

    fn render_wiper_tab(&mut self, ui: &mut egui::Ui, pal: &ThemePalette) {
        let s = self.language.strings();
        let current_lang = self.language;

        ui.label(
            RichText::new(s.wiper_title)
                .size(19.0)
                .strong()
                .color(pal.text_primary),
        );
        ui.label(
            RichText::new(s.wiper_subtitle)
                .size(12.0)
                .color(pal.text_secondary),
        );
        ui.add_space(8.0);

        card_frame(pal).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                let (icon_rect, _) =
                    ui.allocate_exact_size(egui::vec2(18.0, 18.0), Sense::hover());
                paint_vector_icon(
                    ui.painter(),
                    icon_rect,
                    VectorIcon::CheckShield,
                    pal.accent_emerald,
                );
                ui.label(
                    RichText::new(s.wiper_safety_note)
                        .size(12.0)
                        .color(pal.text_secondary),
                );
            });
        });

        ui.add_space(8.0);

        ui.columns(2, |cols| {
            card_frame(pal).show(&mut cols[0], |ui| {
                ui.set_width(ui.available_width());
                ui.label(
                    RichText::new(s.wiper_step1_hdr)
                        .size(14.5)
                        .strong()
                        .color(pal.text_primary),
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
                            pal.bg_card_hover
                        } else {
                            pal.bg_card_subtle
                        })
                        .stroke(Stroke::new(
                            1.2_f32,
                            if is_sel {
                                pal.accent_cyan
                            } else {
                                pal.border_subtle
                            },
                        ))
                        .corner_radius(CornerRadius::same(8))
                        .inner_margin(egui::Margin::symmetric(12, 9))
                        .show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            ui.horizontal(|ui| {
                                let rb = ui.radio_value(
                                    &mut self.selected_drive_idx,
                                    idx,
                                    RichText::new(format!(
                                        "{} {}",
                                        s.wiper_drive_prefix, d.root_path
                                    ))
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
                                            &format!(
                                                "{}: {}",
                                                s.drive_free_label,
                                                format_bytes(d.free_bytes)
                                            ),
                                            Color32::from_rgba_unmultiplied(
                                                pal.accent_emerald.r(),
                                                pal.accent_emerald.g(),
                                                pal.accent_emerald.b(),
                                                32,
                                            ),
                                            pal.accent_emerald,
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
                                pal.bg_track,
                            );
                            let mut used_rect = bar_rect;
                            used_rect.set_width(bar_rect.width() * used_ratio);
                            ui.painter().rect_filled(
                                used_rect,
                                CornerRadius::same(4),
                                pal.accent_cyan,
                            );

                            ui.label(
                                RichText::new(format!(
                                    "{}: {} ({:.0}%) / {}",
                                    s.wiper_used_summary,
                                    format_bytes(d.used_bytes),
                                    used_ratio * 100.0,
                                    format_bytes(d.total_bytes)
                                ))
                                .size(11.0)
                                .monospace()
                                .color(pal.text_muted),
                            );
                        });
                    ui.add_space(5.0);
                }
            });

            card_frame(pal).show(&mut cols[1], |ui| {
                ui.set_width(ui.available_width());
                ui.label(
                    RichText::new(s.wiper_step2_hdr)
                        .size(14.5)
                        .strong()
                        .color(pal.text_primary),
                );
                ui.separator();

                for &alg in WipeAlgorithm::all() {
                    let is_sel = self.wipe_algorithm == alg;
                    egui::Frame::new()
                        .fill(if is_sel {
                            pal.bg_card_hover
                        } else {
                            pal.bg_card_subtle
                        })
                        .stroke(Stroke::new(
                            1.2_f32,
                            if is_sel {
                                pal.accent_rust
                            } else {
                                pal.border_subtle
                            },
                        ))
                        .corner_radius(CornerRadius::same(8))
                        .inner_margin(egui::Margin::symmetric(10, 7))
                        .show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            let rb = ui.radio_value(
                                &mut self.wipe_algorithm,
                                alg,
                                RichText::new(current_lang.localize_wipe_alg(alg))
                                    .size(13.0)
                                    .strong(),
                            );
                            if rb.hovered() {
                                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                            }
                        });
                    ui.add_space(4.0);
                }

                ui.horizontal(|ui| {
                    ui.checkbox(&mut self.wipe_limit_mb_enabled, s.wiper_limit_checkbox);
                    if self.wipe_limit_mb_enabled {
                        ui.add(
                            egui::DragValue::new(&mut self.wipe_limit_mb)
                                .range(16..=65536)
                                .suffix(" MB"),
                        );
                    } else {
                        draw_pill_badge(
                            ui,
                            s.wiper_full_free_badge,
                            Color32::from_rgba_unmultiplied(
                                pal.accent_amber.r(),
                                pal.accent_amber.g(),
                                pal.accent_amber.b(),
                                32,
                            ),
                            pal.accent_amber,
                        );
                    }
                });
            });
        });

        ui.add_space(10.0);
        let prog_snapshot = self
            .wipe_progress
            .lock()
            .map(|p| p.clone())
            .unwrap_or_default();

        card_frame(pal).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                if draw_action_button(
                    ui,
                    pal,
                    s.btn_start_wipe,
                    VectorIcon::DriveCylinder,
                    ButtonVariant::ExecuteBrand,
                    !prog_snapshot.running,
                ) {
                    if let Some(drive) = self.drives.get(self.selected_drive_idx) {
                        let drive_root = drive.root_path.clone();
                        let alg = self.wipe_algorithm;
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
                        pal,
                        s.btn_stop_wipe,
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
                    ui.label(RichText::new(err).color(pal.accent_red));
                }
            }
        });
    }

    // === 6. SOFTWARE UPDATER TAB (WINGET WRAPPER) ===
    fn start_winget_scan(&mut self) {
        let s = self.language.strings();
        self.winget_busy = true;
        self.status_banner = s.soft_scanning_msg.to_string();
        let rx = Arc::clone(&self.winget_rx);
        thread::spawn(move || {
            let rep = check_for_updates();
            if let Ok(mut guard) = rx.lock() {
                *guard = Some(rep);
            }
        });
    }

    fn render_software_updater_tab(&mut self, ui: &mut egui::Ui, pal: &ThemePalette) {
        let s = self.language.strings();

        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.label(
                    RichText::new(s.soft_title)
                        .size(19.0)
                        .strong()
                        .color(pal.text_primary),
                );
                ui.label(
                    RichText::new(s.soft_subtitle)
                        .size(12.0)
                        .color(pal.text_secondary),
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
                    pal,
                    s.btn_upgrade_all,
                    VectorIcon::SoftwareCloudUp,
                    ButtonVariant::ExecuteBrand,
                    !self.winget_busy && has_updates,
                ) {
                    self.winget_busy = true;
                    self.status_banner =
                        "winget upgrade --all --silent...".to_string();
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
                    pal,
                    s.btn_check_updates,
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
                format!("{}", rep.updates.len()),
                if rep.winget_version.is_empty() {
                    "N/A".to_string()
                } else {
                    rep.winget_version.clone()
                },
            )
        } else {
            ("—".to_string(), "...".to_string())
        };

        ui.columns(3, |cols| {
            draw_kpi_card(
                &mut cols[0],
                pal,
                s.kpi_updates_avail,
                &count_str,
                s.kpi_updates_sub,
                pal.accent_cyan,
                VectorIcon::SoftwareCloudUp,
            );
            draw_kpi_card(
                &mut cols[1],
                pal,
                s.kpi_winget_engine,
                &ver_str,
                s.kpi_winget_subtitle,
                pal.accent_emerald,
                VectorIcon::CheckShield,
            );
            draw_kpi_card(
                &mut cols[2],
                pal,
                s.kpi_prop_servers,
                s.kpi_prop_servers_val,
                s.kpi_prop_servers_sub,
                pal.accent_purple,
                VectorIcon::BrandShield,
            );
        });

        ui.add_space(8.0);

        let list_h = (ui.available_height() - 10.0).max(220.0);
        card_frame(pal).show(ui, |ui| {
            ui.set_height(list_h);
            ui.horizontal(|ui| {
                ui.label(RichText::new(s.lbl_filter).color(pal.text_secondary));
                ui.add(
                    egui::TextEdit::singleline(&mut self.winget_filter)
                        .hint_text(s.soft_filter_hint)
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
                        RichText::new(s.soft_scanning_msg)
                            .size(14.0)
                            .color(pal.accent_cyan),
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
                                pal,
                                VectorIcon::CheckShield,
                                pal.accent_emerald,
                                s.soft_all_up_to_date,
                                &rep.raw_message,
                            );
                        } else {
                            for (row_idx, pkg) in rep.updates.iter().enumerate() {
                                if !filter_low.is_empty()
                                    && !pkg.name.to_lowercase().contains(&filter_low)
                                    && !pkg.id.to_lowercase().contains(&filter_low)
                                {
                                    continue;
                                }
                                ui.push_id(row_idx, |ui| {
                                    egui::Frame::new()
                                        .fill(pal.bg_card_subtle)
                                        .stroke(Stroke::new(1.0_f32, pal.border_subtle))
                                        .corner_radius(CornerRadius::same(8))
                                        .inner_margin(egui::Margin::symmetric(12, 9))
                                        .show(ui, |ui| {
                                            let row_w = ui.available_width();
                                            ui.set_width(row_w);
                                            ui.horizontal(|ui| {
                                                let left_w = (row_w - 155.0).max(180.0);
                                                ui.allocate_ui_with_layout(
                                                    egui::vec2(left_w, 38.0),
                                                    egui::Layout::top_down(egui::Align::Min),
                                                    |ui| {
                                                        ui.set_max_width(left_w);
                                                        ui.horizontal_wrapped(|ui| {
                                                            ui.label(
                                                                RichText::new(&pkg.name)
                                                                    .size(14.0)
                                                                    .strong()
                                                                    .color(pal.text_primary),
                                                            );
                                                            draw_pill_badge(
                                                                ui,
                                                                &format!(
                                                                    "{}: {}",
                                                                    s.badge_current_ver,
                                                                    pkg.current_version
                                                                ),
                                                                Color32::from_rgba_unmultiplied(
                                                                    pal.accent_amber.r(),
                                                                    pal.accent_amber.g(),
                                                                    pal.accent_amber.b(),
                                                                    28,
                                                                ),
                                                                pal.accent_amber,
                                                            );
                                                            ui.label(
                                                                RichText::new("->")
                                                                    .monospace()
                                                                    .color(pal.text_muted),
                                                            );
                                                            draw_pill_badge(
                                                                ui,
                                                                &format!(
                                                                    "{}: {}",
                                                                    s.badge_new_ver,
                                                                    pkg.available_version
                                                                ),
                                                                Color32::from_rgba_unmultiplied(
                                                                    pal.accent_emerald.r(),
                                                                    pal.accent_emerald.g(),
                                                                    pal.accent_emerald.b(),
                                                                    32,
                                                                ),
                                                                pal.accent_emerald,
                                                            );
                                                        });
                                                        ui.label(
                                                            RichText::new(format!(
                                                                "ID: {} • {}: {}",
                                                                pkg.id, s.lbl_package_source, pkg.source
                                                            ))
                                                            .size(10.5)
                                                            .monospace()
                                                            .color(pal.text_muted),
                                                        );
                                                    },
                                                );

                                                ui.with_layout(
                                                    egui::Layout::right_to_left(
                                                        egui::Align::Center,
                                                    ),
                                                    |ui| {
                                                        if draw_action_button(
                                                            ui,
                                                            pal,
                                                            s.btn_upgrade_single,
                                                            VectorIcon::SoftwareCloudUp,
                                                            ButtonVariant::ExecuteBrand,
                                                            true,
                                                        ) {
                                                            single_upgrade_id =
                                                                Some(pkg.id.clone());
                                                        }
                                                    },
                                                );
                                            });
                                        });
                                });
                                ui.add_space(4.0);
                            }
                        }
                    });

                if let Some(pkg_id) = single_upgrade_id {
                    self.winget_busy = true;
                    self.status_banner = format!("winget upgrade --id {pkg_id} --silent...");
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
        let s = self.language.strings();
        self.driver_busy = true;
        self.status_banner = s.drv_scanning_msg.to_string();
        let rx = Arc::clone(&self.driver_rx);
        thread::spawn(move || {
            let rep = scan_drivers(online_wu);
            if let Ok(mut g) = rx.lock() {
                *g = Some(rep);
            }
        });
    }

    fn start_driver_install(&mut self) {
        let s = self.language.strings();
        self.driver_busy = true;
        self.status_banner = s.drv_installing_msg.to_string();
        let rx = Arc::clone(&self.driver_action_rx);
        thread::spawn(move || {
            let res = install_wu_driver_updates();
            if let Ok(mut g) = rx.lock() {
                *g = Some(res);
            }
        });
    }

    fn render_driver_updater_tab(&mut self, ui: &mut egui::Ui, pal: &ThemePalette) {
        let s = self.language.strings();

        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.label(
                    RichText::new(s.drv_title)
                        .size(19.0)
                        .strong()
                        .color(pal.text_primary),
                );
                ui.label(
                    RichText::new(s.drv_subtitle)
                        .size(12.0)
                        .color(pal.text_secondary),
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
                    pal,
                    s.btn_install_wu_drivers,
                    VectorIcon::DriverChip,
                    ButtonVariant::ExecuteBrand,
                    !self.driver_busy && pending_count > 0,
                ) {
                    self.start_driver_install();
                }

                if draw_action_button(
                    ui,
                    pal,
                    s.btn_scan_wu_drivers,
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
                format!("{}", rep.wu_pending_drivers.len()),
                format!("{}", rep.installed_drivers.len()),
                format!("{signed} / {} WHQL", rep.installed_drivers.len()),
            )
        } else {
            ("—".to_string(), "—".to_string(), "—".to_string())
        };

        ui.columns(3, |cols| {
            draw_kpi_card(
                &mut cols[0],
                pal,
                s.kpi_wu_drivers,
                &wu_count,
                "Query: IsInstalled=0 and Type='Driver'",
                pal.accent_cyan,
                VectorIcon::SoftwareCloudUp,
            );
            draw_kpi_card(
                &mut cols[1],
                pal,
                s.kpi_pnp_drivers,
                &pnp_count,
                "GPU, Net, Audio, NVMe, USB, Chipset",
                pal.accent_purple,
                VectorIcon::DriverChip,
            );
            draw_kpi_card(
                &mut cols[2],
                pal,
                s.kpi_whql_cert,
                &signed_count,
                s.kpi_whql_sub,
                pal.accent_emerald,
                VectorIcon::CheckShield,
            );
        });

        ui.add_space(8.0);

        let list_h = (ui.available_height() - 10.0).max(220.0);
        card_frame(pal).show(ui, |ui| {
            ui.set_height(list_h);
            if self.driver_busy {
                ui.vertical_centered(|ui| {
                    ui.add_space(60.0);
                    ui.spinner();
                    ui.add_space(10.0);
                    ui.label(
                        RichText::new(s.drv_scanning_msg)
                            .size(14.0)
                            .color(pal.accent_cyan),
                    );
                });
            } else if let Some(rep) = &self.driver_report {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(s.lbl_filter).color(pal.text_secondary));
                    ui.add(
                        egui::TextEdit::singleline(&mut self.driver_filter)
                            .hint_text(s.drv_filter_hint)
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
                                RichText::new(s.drv_wu_pending_hdr)
                                    .size(13.5)
                                    .strong()
                                    .color(pal.accent_amber),
                            );
                            for wu in &rep.wu_pending_drivers {
                                egui::Frame::new()
                                    .fill(pal.bg_card_subtle)
                                    .stroke(Stroke::new(1.2_f32, pal.accent_amber))
                                    .corner_radius(CornerRadius::same(8))
                                    .inner_margin(egui::Margin::symmetric(12, 8))
                                    .show(ui, |ui| {
                                        ui.set_width(ui.available_width());
                                        ui.horizontal_wrapped(|ui| {
                                            draw_pill_badge(
                                                ui,
                                                s.badge_new_wu_driver,
                                                Color32::from_rgba_unmultiplied(
                                                    pal.accent_amber.r(),
                                                    pal.accent_amber.g(),
                                                    pal.accent_amber.b(),
                                                    35,
                                                ),
                                                pal.accent_amber,
                                            );
                                            ui.label(
                                                RichText::new(&wu.title)
                                                    .size(13.5)
                                                    .strong()
                                                    .color(pal.text_primary),
                                            );
                                            draw_pill_badge(
                                                ui,
                                                &format!("{:.2} MB", wu.size_mb),
                                                Color32::from_rgba_unmultiplied(
                                                    pal.accent_cyan.r(),
                                                    pal.accent_cyan.g(),
                                                    pal.accent_cyan.b(),
                                                    28,
                                                ),
                                                pal.accent_cyan,
                                            );
                                        });
                                        ui.label(
                                            RichText::new(format!(
                                                "{} • {} • {}",
                                                wu.driver_manufacturer,
                                                wu.driver_class,
                                                wu.driver_ver_date
                                            ))
                                            .size(11.0)
                                            .monospace()
                                            .color(pal.text_secondary),
                                        );
                                    });
                                ui.add_space(4.0);
                            }
                            ui.separator();
                        }

                        ui.label(
                            RichText::new(s.drv_pnp_installed_hdr)
                                .size(13.0)
                                .strong()
                                .color(pal.text_secondary),
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
                                .fill(pal.bg_card_subtle)
                                .stroke(Stroke::new(1.0_f32, pal.border_subtle))
                                .corner_radius(CornerRadius::same(8))
                                .inner_margin(egui::Margin::symmetric(12, 8))
                                .show(ui, |ui| {
                                    ui.set_width(ui.available_width());
                                    ui.horizontal_wrapped(|ui| {
                                        draw_pill_badge(
                                            ui,
                                            &drv.device_class,
                                            Color32::from_rgba_unmultiplied(
                                                pal.accent_cyan.r(),
                                                pal.accent_cyan.g(),
                                                pal.accent_cyan.b(),
                                                25,
                                            ),
                                            pal.accent_cyan,
                                        );
                                        ui.label(
                                            RichText::new(&drv.device_name)
                                                .size(13.0)
                                                .strong()
                                                .color(pal.text_primary),
                                        );
                                        draw_pill_badge(
                                            ui,
                                            &format!("v{}", drv.driver_version),
                                            Color32::from_rgba_unmultiplied(
                                                pal.accent_purple.r(),
                                                pal.accent_purple.g(),
                                                pal.accent_purple.b(),
                                                28,
                                            ),
                                            pal.accent_purple,
                                        );
                                        if drv.is_signed {
                                            draw_pill_badge(
                                                ui,
                                                s.badge_whql_signed,
                                                Color32::from_rgba_unmultiplied(
                                                    pal.accent_emerald.r(),
                                                    pal.accent_emerald.g(),
                                                    pal.accent_emerald.b(),
                                                    28,
                                                ),
                                                pal.accent_emerald,
                                            );
                                        }
                                    });
                                    ui.label(
                                        RichText::new(format!(
                                            "{} • {} • {}",
                                            drv.manufacturer, drv.driver_date, drv.signer
                                        ))
                                        .size(10.5)
                                        .monospace()
                                        .color(pal.text_muted),
                                    );
                                });
                            ui.add_space(4.0);
                        }
                    });
            }
        });
    }

    fn start_silent_clean_test(&mut self) {
        let s = self.language.strings();
        self.silent_clean_busy = true;
        self.status_banner = s.auto_running_silent_msg.to_string();
        let rx = Arc::clone(&self.silent_clean_rx);
        thread::spawn(move || {
            let res = run_silent_clean_and_log();
            if let Ok(mut g) = rx.lock() {
                *g = Some(res);
            }
        });
    }

    // === 8. AUTOMATION (ZERO-RAM SCHTASKS) & KERNEL MONITOR (NOTIFY) TAB ===
    fn render_automation_monitor_tab(&mut self, ui: &mut egui::Ui, pal: &ThemePalette) {
        let s = self.language.strings();
        let current_lang = self.language;

        ui.label(
            RichText::new(s.auto_title)
                .size(19.0)
                .strong()
                .color(pal.text_primary),
        );
        ui.label(
            RichText::new(s.auto_subtitle)
                .size(12.0)
                .color(pal.text_secondary),
        );
        ui.add_space(8.0);

        let col_h = (ui.available_height() - 10.0).max(320.0);

        ui.columns(2, |cols| {
            // LEFT CARD: Windows Task Scheduler (`schtasks.exe`)
            card_frame(pal).show(&mut cols[0], |ui| {
                ui.set_height(col_h);
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(s.sched_card_hdr)
                            .size(14.5)
                            .strong()
                            .color(pal.text_primary),
                    );
                    draw_pill_badge(
                        ui,
                        s.badge_zero_ram_idle,
                        Color32::from_rgba_unmultiplied(
                            pal.accent_emerald.r(),
                            pal.accent_emerald.g(),
                            pal.accent_emerald.b(),
                            32,
                        ),
                        pal.accent_emerald,
                    );
                });
                ui.separator();

                egui::Frame::new()
                    .fill(pal.bg_card_subtle)
                    .stroke(Stroke::new(1.0_f32, pal.border_subtle))
                    .corner_radius(CornerRadius::same(8))
                    .inner_margin(egui::Margin::symmetric(12, 8))
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(s.lbl_task_status).color(pal.text_secondary));
                            if self.schedule_status.exists {
                                draw_pill_badge(
                                    ui,
                                    s.badge_sched_active,
                                    Color32::from_rgba_unmultiplied(
                                        pal.accent_emerald.r(),
                                        pal.accent_emerald.g(),
                                        pal.accent_emerald.b(),
                                        32,
                                    ),
                                    pal.accent_emerald,
                                );
                            } else {
                                draw_pill_badge(
                                    ui,
                                    s.badge_sched_inactive,
                                    Color32::from_rgba_unmultiplied(
                                        pal.accent_amber.r(),
                                        pal.accent_amber.g(),
                                        pal.accent_amber.b(),
                                        28,
                                    ),
                                    pal.accent_amber,
                                );
                            }
                        });
                        if self.schedule_status.exists {
                            ui.label(
                                RichText::new(format!(
                                    "{} {}",
                                    s.lbl_next_run, self.schedule_status.next_run_time
                                ))
                                .size(11.5)
                                .monospace()
                                .color(pal.accent_cyan),
                            );
                        }
                    });

                ui.add_space(6.0);
                ui.label(
                    RichText::new(s.lbl_exec_freq)
                        .size(12.5)
                        .strong()
                        .color(pal.text_secondary),
                );
                for freq in [
                    ScheduleFrequency::Daily,
                    ScheduleFrequency::Weekly,
                    ScheduleFrequency::OnLogon,
                ] {
                    let rb = ui.radio_value(
                        &mut self.schedule_freq,
                        freq,
                        current_lang.localize_schedule_freq(freq),
                    );
                    if rb.hovered() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    }
                }

                if self.schedule_freq != ScheduleFrequency::OnLogon {
                    ui.horizontal(|ui| {
                        ui.label(s.lbl_time_hhmm);
                        ui.add(
                            egui::TextEdit::singleline(&mut self.schedule_time_hhmm)
                                .desired_width(70.0),
                        );
                    });
                }

                ui.checkbox(&mut self.schedule_highest_uac, s.sched_uac_checkbox);

                ui.add_space(6.0);
                ui.horizontal_wrapped(|ui| {
                    if draw_action_button(
                        ui,
                        pal,
                        s.btn_save_task,
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
                        pal,
                        s.btn_test_silent,
                        VectorIcon::PlayRun,
                        ButtonVariant::Secondary,
                        !self.silent_clean_busy,
                    ) {
                        self.start_silent_clean_test();
                    }

                    if self.schedule_status.exists
                        && draw_action_button(
                            ui,
                            pal,
                            s.btn_remove_task,
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
                    RichText::new(s.sched_logs_hdr)
                        .size(11.5)
                        .strong()
                        .color(pal.text_secondary),
                );
                egui::ScrollArea::vertical()
                    .id_salt("sched_log_scroll")
                    .max_height((ui.available_height() - 6.0).max(80.0))
                    .show(ui, |ui| {
                        if self.schedule_logs.is_empty() {
                            ui.label(
                                RichText::new(s.sched_logs_empty)
                                    .size(11.0)
                                    .color(pal.text_muted),
                            );
                        } else {
                            for line in &self.schedule_logs {
                                ui.label(
                                    RichText::new(line)
                                        .size(10.5)
                                        .monospace()
                                        .color(pal.accent_emerald),
                                );
                            }
                        }
                    });
            });

            // RIGHT CARD: Real-Time Kernel Event Watcher (`notify` / `ReadDirectoryChangesW`)
            card_frame(pal).show(&mut cols[1], |ui| {
                ui.set_height(col_h);
                let mon_snap = self
                    .monitor_state
                    .lock()
                    .map(|m| m.clone())
                    .unwrap_or_default();

                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(s.mon_card_hdr)
                            .size(14.5)
                            .strong()
                            .color(pal.text_primary),
                    );
                    draw_pill_badge(
                        ui,
                        s.badge_zero_cpu,
                        Color32::from_rgba_unmultiplied(
                            pal.accent_cyan.r(),
                            pal.accent_cyan.g(),
                            pal.accent_cyan.b(),
                            32,
                        ),
                        pal.accent_cyan,
                    );
                });
                ui.separator();

                ui.horizontal(|ui| {
                    if !mon_snap.active {
                        if draw_action_button(
                            ui,
                            pal,
                            s.btn_start_monitor,
                            VectorIcon::AutomationPulse,
                            ButtonVariant::PrimaryScan,
                            true,
                        ) {
                            self.monitor_stop_flag.store(false, Ordering::Relaxed);
                            start_realtime_watcher(
                                Arc::clone(&self.monitor_state),
                                Arc::clone(&self.monitor_stop_flag),
                            );
                            self.status_banner = s.banner_monitor_active.to_string();
                        }
                    } else if draw_action_button(
                        ui,
                        pal,
                        s.btn_stop_monitor,
                        VectorIcon::StopOctagon,
                        ButtonVariant::Danger,
                        true,
                    ) {
                        self.monitor_stop_flag.store(true, Ordering::Relaxed);
                        self.status_banner = s.banner_monitor_stopped.to_string();
                    }

                    draw_pill_badge(
                        ui,
                        &format!("{}: {}", s.badge_kernel_events, mon_snap.total_events_received),
                        Color32::from_rgba_unmultiplied(
                            pal.accent_purple.r(),
                            pal.accent_purple.g(),
                            pal.accent_purple.b(),
                            28,
                        ),
                        pal.accent_purple,
                    );
                });

                ui.add_space(4.0);
                if let Ok(mut st) = self.monitor_state.lock() {
                    ui.checkbox(&mut st.auto_clean_on_browser_close, s.mon_auto_browser_cb);
                }

                ui.label(
                    RichText::new(s.mon_watched_paths_hdr)
                        .size(11.5)
                        .strong()
                        .color(pal.text_secondary),
                );
                for (lbl, p) in &mon_snap.watched_paths {
                    let wp = format!("  • {lbl}: {}", p.display());
                    ui.add(
                        egui::Label::new(
                            RichText::new(&wp)
                                .size(10.5)
                                .monospace()
                                .color(pal.text_muted),
                        )
                        .truncate(),
                    )
                    .on_hover_text(p.display().to_string());
                }

                if let Some(last_auto) = &mon_snap.last_auto_clean_summary {
                    ui.label(
                        RichText::new(last_auto)
                            .size(11.0)
                            .color(pal.accent_emerald),
                    );
                }

                ui.separator();
                ui.label(
                    RichText::new(s.mon_live_feed_hdr)
                        .size(12.0)
                        .strong()
                        .color(pal.text_primary),
                );

                egui::ScrollArea::vertical()
                    .id_salt("kernel_events_scroll")
                    .max_height((ui.available_height() - 6.0).max(100.0))
                    .show(ui, |ui| {
                        if mon_snap.events.is_empty() {
                            draw_empty_state(
                                ui,
                                pal,
                                VectorIcon::AutomationPulse,
                                pal.accent_purple,
                                s.mon_empty_title,
                                s.mon_empty_desc,
                            );
                        } else {
                            for ev in &mon_snap.events {
                                ui.horizontal_wrapped(|ui| {
                                    ui.label(
                                        RichText::new(&ev.timestamp)
                                            .size(10.5)
                                            .monospace()
                                            .color(pal.text_muted),
                                    );
                                    draw_pill_badge(
                                        ui,
                                        &ev.event_type,
                                        Color32::from_rgba_unmultiplied(
                                            pal.accent_cyan.r(),
                                            pal.accent_cyan.g(),
                                            pal.accent_cyan.b(),
                                            25,
                                        ),
                                        pal.accent_cyan,
                                    );
                                    ui.label(
                                        RichText::new(&ev.source_label)
                                            .size(11.0)
                                            .strong()
                                            .color(pal.text_primary),
                                    );
                                    ui.add(
                                        egui::Label::new(
                                            RichText::new(&ev.path)
                                                .size(10.5)
                                                .monospace()
                                                .color(pal.text_secondary),
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

    // === 9. DISK SPACE ANALYZER, LARGE FILES & SHA-256 DUPLICATE FINDER TAB ===
    fn start_disk_analysis(&mut self) {
        let ds = self.language.disk_analyzer();
        self.disk_busy = true;
        self.disk_cancel.store(false, Ordering::Relaxed);
        self.status_banner = ds.scanning_msg.to_string();

        let roots = resolve_preset_paths(self.disk_preset, &self.disk_custom_path);
        let min_large_mb = self.disk_min_large_mb;
        let min_dup_kb = self.disk_min_dup_kb;
        let cancel_arc = Arc::clone(&self.disk_cancel);
        let rx_arc = Arc::clone(&self.disk_rx);

        thread::spawn(move || {
            let report = run_disk_analysis(&roots, min_large_mb, min_dup_kb, cancel_arc);
            if let Ok(mut guard) = rx_arc.lock() {
                *guard = Some(report);
            }
        });
    }

    fn render_disk_analyzer_tab(&mut self, ui: &mut egui::Ui, pal: &ThemePalette) {
        let s = self.language.strings();
        let ds = self.language.disk_analyzer();
        let current_lang = self.language;

        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.label(
                    RichText::new(ds.page_title)
                        .size(19.0)
                        .strong()
                        .color(pal.text_primary),
                );
                ui.label(
                    RichText::new(ds.page_subtitle)
                        .size(12.0)
                        .color(pal.text_secondary),
                );
            });

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if self.disk_busy
                    && draw_action_button(
                        ui,
                        pal,
                        ds.btn_stop_scan,
                        VectorIcon::StopOctagon,
                        ButtonVariant::Danger,
                        true,
                    )
                {
                    self.disk_cancel.store(true, Ordering::Relaxed);
                }

                if draw_action_button(
                    ui,
                    pal,
                    ds.btn_start_scan,
                    VectorIcon::DiskRadar,
                    ButtonVariant::PrimaryScan,
                    !self.disk_busy,
                ) {
                    self.start_disk_analysis();
                }
            });
        });

        ui.add_space(6.0);

        let (scanned_bytes_str, scanned_files_str, large_cnt_str, dup_wasted_str, dup_sub_str) =
            if let Some(rep) = &self.disk_report {
                (
                    format_bytes(rep.total_bytes_scanned),
                    format!("{} files", rep.total_files_scanned),
                    format!("{}", rep.large_files.len()),
                    format_bytes(rep.total_duplicate_wasted_bytes),
                    format!(
                        "{} groups • {} copies",
                        rep.duplicate_groups.len(),
                        rep.total_duplicate_extra_files
                    ),
                )
            } else {
                (
                    "—".to_string(),
                    s.kpi_waiting_scan.to_string(),
                    "—".to_string(),
                    "—".to_string(),
                    ds.kpi_dup_groups_sub.to_string(),
                )
            };

        ui.columns(3, |cols| {
            draw_kpi_card(
                &mut cols[0],
                pal,
                ds.kpi_scanned_space,
                &scanned_bytes_str,
                &scanned_files_str,
                pal.accent_cyan,
                VectorIcon::DiskRadar,
            );
            draw_kpi_card(
                &mut cols[1],
                pal,
                ds.kpi_large_files,
                &large_cnt_str,
                ds.kpi_large_sub,
                pal.accent_amber,
                VectorIcon::DriveCylinder,
            );
            draw_kpi_card(
                &mut cols[2],
                pal,
                ds.kpi_dup_wasted,
                &dup_wasted_str,
                &dup_sub_str,
                pal.accent_rust,
                VectorIcon::CleanerSweep,
            );
        });

        ui.add_space(6.0);

        // Configuration & Target Bar
        card_frame(pal).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal_wrapped(|ui| {
                ui.label(
                    RichText::new(ds.lbl_target_preset)
                        .size(12.0)
                        .strong()
                        .color(pal.text_secondary),
                );
                let presets = [
                    ScanTargetPreset::UserProfileMediaAndDocs,
                    ScanTargetPreset::DownloadsFolder,
                    ScanTargetPreset::DocumentsAndDesktop,
                    ScanTargetPreset::SystemDriveRoot,
                    ScanTargetPreset::CustomPath,
                ];
                egui::ComboBox::from_id_salt("disk_preset_combo")
                    .selected_text(
                        RichText::new(current_lang.localize_scan_preset(self.disk_preset))
                            .size(12.0)
                            .strong()
                            .color(pal.text_primary),
                    )
                    .width(310.0)
                    .show_ui(ui, |ui| {
                        for &p in &presets {
                            ui.selectable_value(
                                &mut self.disk_preset,
                                p,
                                current_lang.localize_scan_preset(p),
                            );
                        }
                    });

                if self.disk_preset == ScanTargetPreset::CustomPath {
                    ui.label(RichText::new(ds.lbl_custom_path).color(pal.text_secondary));
                    ui.add(
                        egui::TextEdit::singleline(&mut self.disk_custom_path)
                            .desired_width(200.0),
                    );
                }

                ui.separator();
                ui.label(
                    RichText::new(ds.lbl_min_large_mb)
                        .size(11.5)
                        .color(pal.text_secondary),
                );
                ui.add(
                    egui::DragValue::new(&mut self.disk_min_large_mb)
                        .range(5..=10240)
                        .suffix(" MB"),
                );

                ui.separator();
                ui.label(
                    RichText::new(ds.lbl_min_dup_kb)
                        .size(11.5)
                        .color(pal.text_secondary),
                );
                ui.add(
                    egui::DragValue::new(&mut self.disk_min_dup_kb)
                        .range(1..=1048576)
                        .suffix(" KB"),
                );
            });
        });

        ui.add_space(6.0);

        // Sub-view switcher + Search filter + Duplicate batch actions
        let mut trigger_auto_select_dups = false;
        let mut trigger_delete_selected_dups = false;

        card_frame(pal).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal_wrapped(|ui| {
                let (dup_cnt, large_cnt) = self
                    .disk_report
                    .as_ref()
                    .map(|r| (r.duplicate_groups.len(), r.large_files.len()))
                    .unwrap_or((0, 0));

                let views = [
                    (
                        DiskSubView::Duplicates,
                        format!("{} ({dup_cnt})", ds.subview_duplicates),
                        pal.accent_rust,
                    ),
                    (
                        DiskSubView::LargeFiles,
                        format!("{} ({large_cnt})", ds.subview_large_files),
                        pal.accent_amber,
                    ),
                    (
                        DiskSubView::Categories,
                        ds.subview_categories.to_string(),
                        pal.accent_cyan,
                    ),
                ];

                for (sv, label, accent) in views {
                    let active = self.disk_subview == sv;
                    let btn = egui::Button::new(
                        RichText::new(&label)
                            .size(12.0)
                            .strong()
                            .color(if active {
                                Color32::WHITE
                            } else {
                                pal.text_secondary
                            }),
                    )
                    .fill(if active { accent } else { pal.bg_card_subtle })
                    .stroke(Stroke::new(
                        1.0_f32,
                        if active { accent } else { pal.border_subtle },
                    ))
                    .corner_radius(CornerRadius::same(14));

                    let resp = ui.add(btn);
                    if resp.hovered() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    }
                    if resp.clicked() {
                        self.disk_subview = sv;
                    }
                }

                ui.separator();
                ui.add(
                    egui::TextEdit::singleline(&mut self.disk_search_filter)
                        .hint_text(s.lbl_search_filter)
                        .desired_width(180.0),
                );

                if self.disk_subview == DiskSubView::Duplicates && self.disk_report.is_some() {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let any_selected = self
                            .disk_report
                            .as_ref()
                            .map(|r| {
                                r.duplicate_groups
                                    .iter()
                                    .any(|g| g.files.iter().any(|f| f.selected_for_deletion))
                            })
                            .unwrap_or(false);

                        if draw_action_button(
                            ui,
                            pal,
                            ds.btn_delete_selected_dups,
                            VectorIcon::TrashDelete,
                            ButtonVariant::Danger,
                            any_selected,
                        ) {
                            trigger_delete_selected_dups = true;
                        }

                        if draw_action_button(
                            ui,
                            pal,
                            ds.btn_auto_select_dups,
                            VectorIcon::CheckShield,
                            ButtonVariant::Secondary,
                            true,
                        ) {
                            trigger_auto_select_dups = true;
                        }
                    });
                }
            });
        });

        if trigger_auto_select_dups {
            if let Some(rep) = &mut self.disk_report {
                for grp in &mut rep.duplicate_groups {
                    for (idx, item) in grp.files.iter_mut().enumerate() {
                        item.selected_for_deletion = idx > 0;
                    }
                }
            }
        }

        if trigger_delete_selected_dups {
            let mut deleted_count = 0usize;
            let mut freed_bytes = 0u64;
            if let Some(rep) = &mut self.disk_report {
                for grp in &mut rep.duplicate_groups {
                    let each_sz = grp.each_size_bytes;
                    grp.files.retain(|item| {
                        if item.selected_for_deletion && delete_analyzed_file(&item.path).is_ok() {
                            deleted_count += 1;
                            freed_bytes = freed_bytes.saturating_add(each_sz);
                            return false;
                        }
                        true
                    });
                    let extra = grp.files.len().saturating_sub(1) as u64;
                    grp.wasted_bytes = each_sz.saturating_mul(extra);
                }
                rep.duplicate_groups.retain(|g| g.files.len() >= 2);
                rep.total_duplicate_wasted_bytes =
                    rep.duplicate_groups.iter().map(|g| g.wasted_bytes).sum();
                rep.total_duplicate_extra_files = rep
                    .duplicate_groups
                    .iter()
                    .map(|g| g.files.len().saturating_sub(1))
                    .sum();
            }
            self.status_banner = format!(
                "{}: {} ({})",
                s.kpi_space_freed,
                deleted_count,
                format_bytes(freed_bytes)
            );
        }

        ui.add_space(6.0);

        let list_h = (ui.available_height() - 10.0).max(200.0);
        let filter_low = self.disk_search_filter.trim().to_lowercase();
        let mut single_large_delete_path: Option<PathBuf> = None;

        card_frame(pal).show(ui, |ui| {
            ui.set_height(list_h);

            if self.disk_busy {
                ui.vertical_centered(|ui| {
                    ui.add_space(65.0);
                    ui.spinner();
                    ui.add_space(12.0);
                    ui.label(
                        RichText::new(ds.scanning_msg)
                            .size(14.0)
                            .color(pal.accent_cyan),
                    );
                });
                return;
            }

            let Some(rep) = &mut self.disk_report else {
                draw_empty_state(
                    ui,
                    pal,
                    VectorIcon::DiskRadar,
                    pal.accent_cyan,
                    ds.empty_ready_title,
                    ds.empty_ready_desc,
                );
                return;
            };

            match self.disk_subview {
                DiskSubView::Duplicates => {
                    egui::ScrollArea::vertical()
                        .id_salt("disk_duplicates_scroll")
                        .max_height((ui.available_height() - 6.0).max(160.0))
                        .show(ui, |ui| {
                            if rep.duplicate_groups.is_empty() {
                                draw_empty_state(
                                    ui,
                                    pal,
                                    VectorIcon::CheckShield,
                                    pal.accent_emerald,
                                    ds.empty_no_dups_title,
                                    ds.empty_no_dups_desc,
                                );
                                return;
                            }

                            for (grp_idx, grp) in rep.duplicate_groups.iter_mut().enumerate() {
                                if !filter_low.is_empty()
                                    && !grp.sha256_hex.contains(&filter_low)
                                    && !grp.files.iter().any(|f| {
                                        f.path.to_string_lossy().to_lowercase().contains(&filter_low)
                                    })
                                {
                                    continue;
                                }

                                ui.push_id(grp_idx, |ui| {
                                    egui::Frame::new()
                                        .fill(pal.bg_card_subtle)
                                        .stroke(Stroke::new(1.0_f32, pal.border_subtle))
                                        .corner_radius(CornerRadius::same(8))
                                        .inner_margin(egui::Margin::symmetric(12, 8))
                                        .show(ui, |ui| {
                                            ui.set_width(ui.available_width());
                                            ui.horizontal_wrapped(|ui| {
                                                draw_pill_badge(
                                                    ui,
                                                    &format!(
                                                        "{}: {}",
                                                        ds.badge_wasted,
                                                        format_bytes(grp.wasted_bytes)
                                                    ),
                                                    Color32::from_rgba_unmultiplied(
                                                        pal.accent_rust.r(),
                                                        pal.accent_rust.g(),
                                                        pal.accent_rust.b(),
                                                        30,
                                                    ),
                                                    pal.accent_rust,
                                                );
                                                draw_pill_badge(
                                                    ui,
                                                    &format!(
                                                        "{}: {} × {}",
                                                        ds.badge_each,
                                                        format_bytes(grp.each_size_bytes),
                                                        grp.files.len()
                                                    ),
                                                    Color32::from_rgba_unmultiplied(
                                                        pal.accent_cyan.r(),
                                                        pal.accent_cyan.g(),
                                                        pal.accent_cyan.b(),
                                                        25,
                                                    ),
                                                    pal.accent_cyan,
                                                );
                                                let short_sha =
                                                    &grp.sha256_hex[..grp.sha256_hex.len().min(16)];
                                                ui.label(
                                                    RichText::new(format!("SHA-256: {short_sha}…"))
                                                        .size(11.0)
                                                        .monospace()
                                                        .color(pal.text_muted),
                                                )
                                                .on_hover_text(&grp.sha256_hex);
                                            });

                                            ui.add_space(4.0);
                                            for (f_idx, item) in grp.files.iter_mut().enumerate() {
                                                ui.push_id(f_idx, |ui| {
                                                    let row_w = ui.available_width();
                                                    ui.horizontal(|ui| {
                                                        let left_w = (row_w - 120.0).max(180.0);
                                                        ui.allocate_ui_with_layout(
                                                            egui::vec2(left_w, 24.0),
                                                            egui::Layout::left_to_right(
                                                                egui::Align::Center,
                                                            ),
                                                            |ui| {
                                                                ui.set_max_width(left_w);
                                                                let cb = ui.checkbox(
                                                                    &mut item.selected_for_deletion,
                                                                    "",
                                                                );
                                                                if cb.hovered() {
                                                                    ui.ctx().set_cursor_icon(
                                                                        egui::CursorIcon::PointingHand,
                                                                    );
                                                                }
                                                                if f_idx == 0
                                                                    && !item.selected_for_deletion
                                                                {
                                                                    draw_pill_badge(
                                                                        ui,
                                                                        ds.badge_original_keep,
                                                                        Color32::from_rgba_unmultiplied(
                                                                            pal.accent_emerald.r(),
                                                                            pal.accent_emerald.g(),
                                                                            pal.accent_emerald.b(),
                                                                            28,
                                                                        ),
                                                                        pal.accent_emerald,
                                                                    );
                                                                } else {
                                                                    draw_pill_badge(
                                                                        ui,
                                                                        ds.badge_duplicate_copy,
                                                                        Color32::from_rgba_unmultiplied(
                                                                            pal.accent_amber.r(),
                                                                            pal.accent_amber.g(),
                                                                            pal.accent_amber.b(),
                                                                            28,
                                                                        ),
                                                                        pal.accent_amber,
                                                                    );
                                                                }
                                                                let p_str = item
                                                                    .path
                                                                    .to_string_lossy()
                                                                    .to_string();
                                                                ui.add(
                                                                    egui::Label::new(
                                                                        RichText::new(&p_str)
                                                                            .size(11.0)
                                                                            .monospace()
                                                                            .color(pal.text_primary),
                                                                    )
                                                                    .truncate(),
                                                                )
                                                                .on_hover_text(&p_str);
                                                            },
                                                        );

                                                        ui.with_layout(
                                                            egui::Layout::right_to_left(
                                                                egui::Align::Center,
                                                            ),
                                                            |ui| {
                                                                if draw_action_button(
                                                                    ui,
                                                                    pal,
                                                                    ds.btn_reveal_folder,
                                                                    VectorIcon::FolderOpen,
                                                                    ButtonVariant::Secondary,
                                                                    true,
                                                                ) {
                                                                    let _ =
                                                                        reveal_in_explorer(&item.path);
                                                                }
                                                            },
                                                        );
                                                    });
                                                });
                                            }
                                        });
                                });
                                ui.add_space(4.0);
                            }
                        });
                }
                DiskSubView::LargeFiles => {
                    egui::ScrollArea::vertical()
                        .id_salt("disk_large_files_scroll")
                        .max_height((ui.available_height() - 6.0).max(160.0))
                        .show(ui, |ui| {
                            if rep.large_files.is_empty() {
                                draw_empty_state(
                                    ui,
                                    pal,
                                    VectorIcon::CheckShield,
                                    pal.accent_emerald,
                                    ds.empty_no_large_title,
                                    ds.empty_no_large_desc,
                                );
                                return;
                            }

                            let max_sz = rep
                                .large_files
                                .first()
                                .map(|f| f.size_bytes.max(1))
                                .unwrap_or(1);

                            for (row_idx, lf) in rep.large_files.iter().enumerate() {
                                let p_str = lf.path.to_string_lossy().to_string();
                                if !filter_low.is_empty()
                                    && !p_str.to_lowercase().contains(&filter_low)
                                {
                                    continue;
                                }

                                ui.push_id(row_idx, |ui| {
                                    egui::Frame::new()
                                        .fill(pal.bg_card_subtle)
                                        .stroke(Stroke::new(1.0_f32, pal.border_subtle))
                                        .corner_radius(CornerRadius::same(8))
                                        .inner_margin(egui::Margin::symmetric(12, 8))
                                        .show(ui, |ui| {
                                            let row_w = ui.available_width();
                                            ui.set_width(row_w);
                                            ui.horizontal(|ui| {
                                                let left_w = (row_w - 210.0).max(180.0);
                                                ui.allocate_ui_with_layout(
                                                    egui::vec2(left_w, 38.0),
                                                    egui::Layout::top_down(egui::Align::Min),
                                                    |ui| {
                                                        ui.set_max_width(left_w);
                                                        ui.horizontal_wrapped(|ui| {
                                                            draw_pill_badge(
                                                                ui,
                                                                &format_bytes(lf.size_bytes),
                                                                Color32::from_rgba_unmultiplied(
                                                                    pal.accent_amber.r(),
                                                                    pal.accent_amber.g(),
                                                                    pal.accent_amber.b(),
                                                                    30,
                                                                ),
                                                                pal.accent_amber,
                                                            );
                                                            draw_pill_badge(
                                                                ui,
                                                                current_lang
                                                                    .localize_file_category(lf.category),
                                                                Color32::from_rgba_unmultiplied(
                                                                    pal.accent_cyan.r(),
                                                                    pal.accent_cyan.g(),
                                                                    pal.accent_cyan.b(),
                                                                    25,
                                                                ),
                                                                pal.accent_cyan,
                                                            );
                                                            let fname = lf
                                                                .path
                                                                .file_name()
                                                                .and_then(|n| n.to_str())
                                                                .unwrap_or(&p_str);
                                                            ui.label(
                                                                RichText::new(fname)
                                                                    .size(13.0)
                                                                    .strong()
                                                                    .color(pal.text_primary),
                                                            );
                                                        });
                                                        ui.add(
                                                            egui::Label::new(
                                                                RichText::new(&p_str)
                                                                    .size(10.5)
                                                                    .monospace()
                                                                    .color(pal.text_muted),
                                                            )
                                                            .truncate(),
                                                        )
                                                        .on_hover_text(&p_str);
                                                    },
                                                );

                                                ui.with_layout(
                                                    egui::Layout::right_to_left(
                                                        egui::Align::Center,
                                                    ),
                                                    |ui| {
                                                        if draw_action_button(
                                                            ui,
                                                            pal,
                                                            ds.btn_delete_file,
                                                            VectorIcon::TrashDelete,
                                                            ButtonVariant::Danger,
                                                            true,
                                                        ) {
                                                            single_large_delete_path =
                                                                Some(lf.path.clone());
                                                        }
                                                        if draw_action_button(
                                                            ui,
                                                            pal,
                                                            ds.btn_reveal_folder,
                                                            VectorIcon::FolderOpen,
                                                            ButtonVariant::Secondary,
                                                            true,
                                                        ) {
                                                            let _ = reveal_in_explorer(&lf.path);
                                                        }
                                                    },
                                                );
                                            });

                                            let ratio =
                                                (lf.size_bytes as f32 / max_sz as f32).clamp(0.02, 1.0);
                                            let (bar_rect, _) = ui.allocate_exact_size(
                                                egui::vec2(ui.available_width(), 4.0),
                                                Sense::hover(),
                                            );
                                            ui.painter().rect_filled(
                                                bar_rect,
                                                CornerRadius::same(2),
                                                pal.bg_track,
                                            );
                                            let mut fill_r = bar_rect;
                                            fill_r.set_width(bar_rect.width() * ratio);
                                            ui.painter().rect_filled(
                                                fill_r,
                                                CornerRadius::same(2),
                                                pal.accent_amber,
                                            );
                                        });
                                });
                                ui.add_space(4.0);
                            }
                        });
                }
                DiskSubView::Categories => {
                    let total_b = rep.total_bytes_scanned.max(1);
                    let cat_colors = |cat: FileCategory| -> Color32 {
                        match cat {
                            FileCategory::VideoMedia => pal.accent_purple,
                            FileCategory::ArchivesDiskImages => pal.accent_amber,
                            FileCategory::ExecutablesInstallers => pal.accent_rust,
                            FileCategory::DevVmArtifacts => pal.accent_cyan,
                            FileCategory::DocumentsPictures => pal.accent_emerald,
                            FileCategory::LogsDumpsOther => pal.text_muted,
                        }
                    };

                    // Stacked distribution bar
                    let (bar_rect, _) = ui.allocate_exact_size(
                        egui::vec2(ui.available_width(), 16.0),
                        Sense::hover(),
                    );
                    ui.painter()
                        .rect_filled(bar_rect, CornerRadius::same(6), pal.bg_track);
                    let mut x_cursor = bar_rect.left();
                    for st in &rep.categories {
                        if st.total_bytes == 0 {
                            continue;
                        }
                        let w = (st.total_bytes as f32 / total_b as f32) * bar_rect.width();
                        if w >= 1.5 {
                            let seg = Rect::from_min_max(
                                Pos2::new(x_cursor, bar_rect.top()),
                                Pos2::new(
                                    (x_cursor + w).min(bar_rect.right()),
                                    bar_rect.bottom(),
                                ),
                            );
                            ui.painter().rect_filled(
                                seg,
                                CornerRadius::same(3),
                                cat_colors(st.category),
                            );
                            x_cursor += w;
                        }
                    }

                    ui.add_space(10.0);

                    for st in &rep.categories {
                        let pct = (st.total_bytes as f64 / total_b as f64) * 100.0;
                        let col = cat_colors(st.category);
                        egui::Frame::new()
                            .fill(pal.bg_card_subtle)
                            .stroke(Stroke::new(1.0_f32, pal.border_subtle))
                            .corner_radius(CornerRadius::same(8))
                            .inner_margin(egui::Margin::symmetric(12, 8))
                            .show(ui, |ui| {
                                ui.set_width(ui.available_width());
                                ui.horizontal(|ui| {
                                    let (dot_r, _) = ui.allocate_exact_size(
                                        egui::vec2(10.0, 10.0),
                                        Sense::hover(),
                                    );
                                    ui.painter().circle_filled(dot_r.center(), 5.0, col);
                                    ui.label(
                                        RichText::new(
                                            current_lang.localize_file_category(st.category),
                                        )
                                        .size(13.5)
                                        .strong()
                                        .color(pal.text_primary),
                                    );
                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::Center),
                                        |ui| {
                                            draw_pill_badge(
                                                ui,
                                                &format!("{pct:.1}%"),
                                                Color32::from_rgba_unmultiplied(
                                                    col.r(),
                                                    col.g(),
                                                    col.b(),
                                                    28,
                                                ),
                                                col,
                                            );
                                            ui.label(
                                                RichText::new(format!(
                                                    "{} ({} files)",
                                                    format_bytes(st.total_bytes),
                                                    st.file_count
                                                ))
                                                .size(12.5)
                                                .monospace()
                                                .strong()
                                                .color(pal.text_secondary),
                                            );
                                        },
                                    );
                                });
                            });
                        ui.add_space(4.0);
                    }
                }
            }
        });

        if let Some(target_del) = single_large_delete_path {
            match delete_analyzed_file(&target_del) {
                Ok(()) => {
                    if let Some(rep) = &mut self.disk_report {
                        rep.large_files.retain(|f| f.path != target_del);
                    }
                    self.status_banner = format!("Deleted '{}'", target_del.display());
                }
                Err(e) => self.status_banner = e,
            }
        }
    }
}
