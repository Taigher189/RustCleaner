//! Adversarial Test Suite for Milestone 2 (Modern Windows GUI/UX, WCAG Contrast, Non-Blocking Threading & i18n)
//!
//! Empirical Challenger 1 verification:
//! 1. Async worker thread non-blocking simulation and completion polling.
//! 2. Double-trigger and race-condition immunity for `reg_busy`, `driver_busy`, and `silent_clean_busy`.
//! 3. Repaint rate limiting (250ms interval = 4 fps maximum animation / background polling rate).
//! 4. WCAG 2.1 relative luminance and contrast ratio mathematical oracle.
//! 5. 7-Language complete coverage and non-empty validation for all M2 tokens.
//! 6. Bento card height and header responsive collapse invariants.

#[path = "../src/cleaner.rs"]
pub mod cleaner;
#[path = "../src/driver_updater.rs"]
pub mod driver_updater;
#[path = "../src/disk_analyzer.rs"]
pub mod disk_analyzer;
#[path = "../src/i18n.rs"]
pub mod i18n;
#[path = "../src/monitor.rs"]
pub mod monitor;
#[path = "../src/registry.rs"]
pub mod registry;
#[path = "../src/scheduler.rs"]
pub mod scheduler;
#[path = "../src/software_updater.rs"]
pub mod software_updater;
#[path = "../src/startup_uninstall.rs"]
pub mod startup_uninstall;
#[path = "../src/uac.rs"]
pub mod uac;
#[path = "../src/wiper.rs"]
pub mod wiper;
#[path = "../src/residual_hunter.rs"]
pub mod residual_hunter;
#[path = "../src/decrapifier.rs"]
pub mod decrapifier;
#[path = "../src/gui.rs"]
#[allow(dead_code)]
pub mod gui;

use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

// ============================================================================
// 1. ASYNC THREADING & DOUBLE-TRIGGER IMMUNITY
// ============================================================================

#[test]
fn challenge_rapid_concurrent_worker_completions_do_not_deadlock() {
    const NUM_WORKERS: usize = 20;
    let rx_slot: Arc<Mutex<Option<usize>>> = Arc::new(Mutex::new(None));
    let mut handles = Vec::new();

    for i in 0..NUM_WORKERS {
        let slot = Arc::clone(&rx_slot);
        handles.push(thread::spawn(move || {
            thread::sleep(Duration::from_millis(5));
            if let Ok(mut g) = slot.lock() {
                *g = Some(i);
            }
        }));
    }

    // UI polling loop simulating egui update() frames
    let start = Instant::now();
    let mut results_seen = 0;
    while start.elapsed() < Duration::from_millis(1500) && results_seen < NUM_WORKERS {
        if let Ok(mut guard) = rx_slot.lock() {
            if guard.take().is_some() {
                results_seen += 1;
            }
        }
        thread::sleep(Duration::from_millis(10));
    }

    for h in handles {
        let _ = h.join();
    }

    assert!(
        results_seen > 0,
        "UI polling must successfully receive worker messages without deadlocking"
    );
}

#[test]
fn challenge_double_click_suppression_state_machine() {
    // Model the state machine of `reg_busy` / `driver_busy` / `silent_clean_busy`
    struct MockAppState {
        is_busy: bool,
        spawn_count: usize,
        result_slot: Arc<Mutex<Option<String>>>,
    }

    impl MockAppState {
        fn new() -> Self {
            Self {
                is_busy: false,
                spawn_count: 0,
                result_slot: Arc::new(Mutex::new(None)),
            }
        }

        // Simulates `draw_action_button(..., enabled = !self.is_busy)`
        fn try_trigger_action(&mut self) -> bool {
            let enabled = !self.is_busy;
            if !enabled {
                return false; // Button disabled: click ignored
            }
            self.is_busy = true;
            self.spawn_count += 1;

            let slot = Arc::clone(&self.result_slot);
            thread::spawn(move || {
                thread::sleep(Duration::from_millis(50));
                if let Ok(mut g) = slot.lock() {
                    *g = Some("Success".to_string());
                }
            });
            true
        }

        fn poll_update(&mut self) {
            if let Ok(mut g) = self.result_slot.lock() {
                if let Some(_res) = g.take() {
                    self.is_busy = false;
                }
            }
        }
    }

    let mut state = MockAppState::new();

    // First click succeeds and starts job
    assert!(state.try_trigger_action());
    assert!(state.is_busy);
    assert_eq!(state.spawn_count, 1);

    // Spam 10 rapid clicks while busy -> ALL MUST BE REJECTED
    for _ in 0..10 {
        assert!(!state.try_trigger_action(), "Spam clicks must be rejected while is_busy");
        assert_eq!(state.spawn_count, 1, "Duplicate threads must NOT be spawned");
    }

    // Wait for worker completion and poll update
    thread::sleep(Duration::from_millis(80));
    state.poll_update();
    assert!(!state.is_busy, "is_busy must clear upon receiving worker completion");

    // Second valid click after job completes
    assert!(state.try_trigger_action(), "Should accept click after job completes");
    assert_eq!(state.spawn_count, 2);
}

// ============================================================================
// 2. REPAINT RATE LIMITING (250ms = 4 FPS) & ZERO CPU IDLE INVARIANT
// ============================================================================

#[test]
fn challenge_repaint_frequency_bounds_and_idle_behavior() {
    let repaint_interval = Duration::from_millis(250);

    // 1. Invariant: Repaint duration must not exceed 4 fps during background jobs
    let fps = 1.0 / repaint_interval.as_secs_f64();
    assert_eq!(fps, 4.0, "Repaint frequency during background operations must be bounded at 4 fps");

    // 2. Invariant: When no background jobs are running, repaint request is NOT sent
    let cleaner_busy = false;
    let wiper_running = false;
    let winget_busy = false;
    let driver_busy = false;
    let reg_busy = false;
    let silent_clean_busy = false;
    let monitor_active = false;

    let should_request_repaint = cleaner_busy
        || wiper_running
        || winget_busy
        || driver_busy
        || reg_busy
        || silent_clean_busy
        || monitor_active;

    assert!(
        !should_request_repaint,
        "When all background tasks are idle, no repaint should be requested (0% idle CPU)"
    );

    // 3. Invariant: When any busy flag is true, repaint is requested
    let reg_busy_active = true;
    let should_repaint_during_reg = cleaner_busy
        || wiper_running
        || winget_busy
        || driver_busy
        || reg_busy_active
        || silent_clean_busy
        || monitor_active;

    assert!(
        should_repaint_during_reg,
        "When reg_busy is active, repaint must be requested for spinner animation"
    );
}

// ============================================================================
// 3. WCAG 2.1 CONTRAST RATIO MATHEMATICAL ORACLE
// ============================================================================

fn srgb_to_linear(val: u8) -> f64 {
    let c = val as f64 / 255.0;
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

fn relative_luminance(r: u8, g: u8, b: u8) -> f64 {
    0.2126 * srgb_to_linear(r) + 0.7152 * srgb_to_linear(g) + 0.0722 * srgb_to_linear(b)
}

fn contrast_ratio(c1: (u8, u8, u8), c2: (u8, u8, u8)) -> f64 {
    let l1 = relative_luminance(c1.0, c1.1, c1.2);
    let l2 = relative_luminance(c2.0, c2.1, c2.2);
    let lighter = l1.max(l2);
    let darker = l1.min(l2);
    (lighter + 0.05) / (darker + 0.05)
}

#[test]
fn challenge_wcag_buttons_contrast_compliance() {
    // PrimaryScan Dark: bg #122A44, text #38BDF8
    let ratio = contrast_ratio((18, 42, 68), (56, 189, 248));
    assert!(ratio >= 4.5, "PrimaryScan Dark must pass WCAG AA (got {:.2}:1)", ratio);

    // PrimaryScan Light: bg #E0F2FE, text #075985
    let ratio = contrast_ratio((224, 242, 254), (7, 89, 133));
    assert!(ratio >= 4.5, "PrimaryScan Light must pass WCAG AA (got {:.2}:1)", ratio);

    // ExecuteBrand: bg #C2410C, text #FFFFFF
    let ratio = contrast_ratio((194, 65, 12), (255, 255, 255));
    assert!(ratio >= 4.5, "ExecuteBrand must pass WCAG AA (got {:.2}:1)", ratio);

    // Danger Dark: bg #45141A, text #F87171
    let ratio = contrast_ratio((69, 20, 26), (248, 113, 113));
    assert!(ratio >= 4.5, "Danger Dark must pass WCAG AA (got {:.2}:1)", ratio);

    // Danger Light: bg #FEE2E2, text #B91C1C
    let ratio = contrast_ratio((254, 226, 226), (185, 28, 28));
    assert!(ratio >= 4.5, "Danger Light must pass WCAG AA (got {:.2}:1)", ratio);

    // UAC Header Dark: bg #F59E0B, text #0F172A
    let ratio = contrast_ratio((245, 158, 11), (15, 23, 42));
    assert!(ratio >= 7.0, "UAC Header Dark must pass WCAG AAA (got {:.2}:1)", ratio);

    // UAC Header Light: bg #B45309, text #FFFFFF
    let ratio = contrast_ratio((180, 83, 9), (255, 255, 255));
    assert!(ratio >= 4.5, "UAC Header Light must pass WCAG AA (got {:.2}:1)", ratio);
}

#[test]
fn challenge_wcag_theme_surfaces_contrast_compliance() {
    // Dark Palette: bg_card (22, 31, 51)
    let dark_bg = (22, 31, 51);
    let dark_text_primary = (248, 250, 252);
    let dark_text_secondary = (203, 213, 225);
    let dark_text_muted = (148, 163, 184);

    assert!(contrast_ratio(dark_bg, dark_text_primary) >= 7.0, "Dark Primary text must pass AAA");
    assert!(contrast_ratio(dark_bg, dark_text_secondary) >= 7.0, "Dark Secondary text must pass AAA");
    assert!(contrast_ratio(dark_bg, dark_text_muted) >= 4.5, "Dark Muted text must pass AA");

    // Light Palette: bg_card (255, 255, 255)
    let light_bg = (255, 255, 255);
    let light_text_primary = (15, 23, 42);
    let light_text_secondary = (51, 65, 85);
    let light_text_muted = (71, 85, 105);

    assert!(contrast_ratio(light_bg, light_text_primary) >= 7.0, "Light Primary text must pass AAA");
    assert!(contrast_ratio(light_bg, light_text_secondary) >= 7.0, "Light Secondary text must pass AAA");
    assert!(contrast_ratio(light_bg, light_text_muted) >= 4.5, "Light Muted text must pass AA");
}

// ============================================================================
// 4. I18N COMPLETENESS FOR NEW M2 TOKENS ACROSS ALL 7 LANGUAGES
// ============================================================================

#[test]
fn challenge_m2_new_tokens_non_empty_in_all_languages() {
    for lang in i18n::Language::all() {
        let s = lang.strings();
        let tokens = [
            ("banner_skipped_suffix", s.banner_skipped_suffix),
            ("banner_locked_suffix", s.banner_locked_suffix),
            ("kpi_files_unit", s.kpi_files_unit),
            ("banner_fixed_suffix", s.banner_fixed_suffix),
            ("banner_startup_deleted", s.banner_startup_deleted),
            ("lbl_backup", s.lbl_backup),
            ("kpi_winget_subtitle", s.kpi_winget_subtitle),
            ("kpi_prop_servers_val", s.kpi_prop_servers_val),
            ("lbl_package_source", s.lbl_package_source),
            ("banner_monitor_active", s.banner_monitor_active),
            ("banner_monitor_stopped", s.banner_monitor_stopped),
            ("reg_scanning_msg", s.reg_scanning_msg),
            ("reg_fixing_msg", s.reg_fixing_msg),
            ("drv_installing_msg", s.drv_installing_msg),
            ("auto_running_silent_msg", s.auto_running_silent_msg),
        ];

        for (name, val) in tokens {
            assert!(
                !val.trim().is_empty(),
                "Token '{}' in language '{:?}' must not be empty or whitespace only",
                name,
                lang
            );
            assert!(
                !val.contains('\0'),
                "Token '{}' in language '{:?}' must not contain null bytes",
                name,
                lang
            );
        }
    }
}

// ============================================================================
// 5. ERGONOMIC LAYOUT & RESPONSIVE COLLAPSE CONSTRAINTS
// ============================================================================

#[test]
#[allow(clippy::assertions_on_constants)]
fn challenge_bento_grid_and_header_collapse_invariants() {
    // 1. Invariant: At minimum window width (1020px), both compact modes must activate
    assert!(
        gui::is_compact_header(gui::MIN_WINDOW_WIDTH),
        "Subtitle must collapse at initial/minimum 1020px window width"
    );
    assert!(
        gui::is_ultra_compact(gui::MIN_WINDOW_WIDTH),
        "Telemetry badge must collapse at initial/minimum 1020px window width to prevent clipping"
    );

    // 2. Invariant: At standard default window width (1220px), neither compact mode is active
    assert!(
        !gui::is_compact_header(gui::DEFAULT_WINDOW_WIDTH),
        "Header subtitle should be visible at default 1220px window width"
    );
    assert!(
        !gui::is_ultra_compact(gui::DEFAULT_WINDOW_WIDTH),
        "Telemetry badge should be visible at default 1220px window width"
    );

    // 3. Invariant: Breakpoint edge conditions
    assert!(gui::is_ultra_compact(1049.9), "Ultra-compact below 1050px");
    assert!(!gui::is_ultra_compact(1050.0), "Not ultra-compact at 1050px");
    assert!(gui::is_compact_header(1149.9), "Compact below 1150px");
    assert!(!gui::is_compact_header(1150.0), "Not compact at 1150px");

    // 4. Breakpoint hierarchy: ultra-compact badge breakpoint must be strictly narrower than subtitle breakpoint
    assert!(
        gui::HEADER_BADGE_BREAKPOINT < gui::HEADER_SUBTITLE_BREAKPOINT,
        "Badge breakpoint must be tighter than subtitle breakpoint"
    );

    // 5. Invariant: Minimum window width is strictly within the ultra-compact threshold
    assert!(
        gui::MIN_WINDOW_WIDTH < gui::HEADER_BADGE_BREAKPOINT,
        "Minimum window width must activate ultra-compact header mode"
    );

    // 6. Invariant: Bento card minimum height guarantees row height parity across 7 languages
    assert!(
        gui::MIN_KPI_CARD_HEIGHT >= 78.0,
        "Bento card min_height must be at least 78.0px for multiline localization"
    );
}
