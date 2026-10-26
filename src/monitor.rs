//! 4. Monitoraggio in Tempo Reale a Zero Polling (`notify` crate + `ReadDirectoryChangesW`)
//!
//! Sleeps at the Windows kernel level (0% CPU polling overhead) and wakes up only when the OS
//! filesystem driver emits an event in `%TEMP%` or browser session/lock directories.

use crate::cleaner::{default_clean_rules, execute_clean, expand_env_vars, format_bytes};
use chrono::Local;
use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KernelWatchEvent {
    pub timestamp: String,
    pub source_label: String,
    pub event_type: String,
    pub path: String,
    pub auto_action_triggered: Option<String>,
}

#[derive(Debug, Clone)]
pub struct RealtimeMonitorState {
    pub active: bool,
    pub watched_paths: Vec<(String, PathBuf)>,
    pub events: Vec<KernelWatchEvent>,
    pub total_events_received: usize,
    pub auto_clean_on_browser_close: bool,
    pub auto_clean_temp_threshold_mb: u64,
    pub last_auto_clean_summary: Option<String>,
}

impl Default for RealtimeMonitorState {
    fn default() -> Self {
        Self {
            active: false,
            watched_paths: default_watched_targets(),
            events: Vec::new(),
            total_events_received: 0,
            auto_clean_on_browser_close: true,
            auto_clean_temp_threshold_mb: 512,
            last_auto_clean_summary: None,
        }
    }
}

fn default_watched_targets() -> Vec<(String, PathBuf)> {
    let candidates = [
        ("Windows %TEMP%", r"%TEMP%"),
        (
            "Chrome Session/Lock",
            r"%LOCALAPPDATA%\Google\Chrome\User Data\Default",
        ),
        (
            "Edge Session/Lock",
            r"%LOCALAPPDATA%\Microsoft\Edge\User Data\Default",
        ),
        (
            "Brave Session/Lock",
            r"%LOCALAPPDATA%\BraveSoftware\Brave-Browser\User Data\Default",
        ),
    ];

    let mut list = Vec::new();
    for (label, raw) in candidates {
        let expanded = expand_env_vars(raw);
        if expanded.exists() {
            list.push((label.to_string(), expanded));
        }
    }
    list
}

/// Starts a background kernel-level `ReadDirectoryChangesW` watcher (`notify::RecommendedWatcher`).
/// Does NOT poll in a loop — blocks on the OS channel until `stop_flag` is set or an event arrives.
pub fn start_realtime_watcher(
    state_arc: Arc<Mutex<RealtimeMonitorState>>,
    stop_flag: Arc<AtomicBool>,
) {
    let targets = {
        let Ok(mut st) = state_arc.lock() else {
            return;
        };
        st.active = true;
        st.watched_paths.clone()
    };

    thread::spawn(move || {
        let (tx, rx) = std::sync::mpsc::channel::<notify::Result<Event>>();

        let Ok(mut watcher) = RecommendedWatcher::new(tx, Config::default()) else {
            if let Ok(mut st) = state_arc.lock() {
                st.active = false;
            }
            return;
        };

        for (_, path) in &targets {
            let _ = watcher.watch(path, RecursiveMode::NonRecursive);
        }

        // Debounce timer to prevent recursive event cascades when temp files are deleted
        let mut last_auto_clean = std::time::Instant::now()
            .checked_sub(std::time::Duration::from_secs(60))
            .unwrap_or_else(std::time::Instant::now);

        // Sleep waiting on OS kernel events (with 400ms recv_timeout only to check the stop flag)
        while !stop_flag.load(Ordering::Relaxed) {
            if let Ok(Ok(event)) = rx.recv_timeout(std::time::Duration::from_millis(400)) {
                let kind_str = match event.kind {
                    EventKind::Create(_) => "CREATE",
                    EventKind::Modify(_) => "MODIFY",
                    EventKind::Remove(_) => "REMOVE",
                    _ => continue,
                };

                let Some(first_path) = event.paths.first() else {
                    continue;
                };
                let path_str = first_path.display().to_string();
                let fname_low = first_path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or_default()
                    .to_lowercase();

                let source_label = if path_str.to_lowercase().contains("chrome") {
                    "Google Chrome"
                } else if path_str.to_lowercase().contains("edge") {
                    "Microsoft Edge"
                } else if path_str.to_lowercase().contains("brave") {
                    "Brave Browser"
                } else {
                    "Sistema %TEMP%"
                };

                let is_browser_lock_release = matches!(event.kind, EventKind::Remove(_))
                    && (fname_low.contains("lock")
                        || fname_low.contains("session")
                        || fname_low.ends_with("-journal"));

                let mut auto_msg = None;
                let now = std::time::Instant::now();
                let on_cooldown = now.duration_since(last_auto_clean) < std::time::Duration::from_secs(15);
                let should_auto_clean = state_arc
                    .lock()
                    .map(|s| s.auto_clean_on_browser_close && is_browser_lock_release && !on_cooldown)
                    .unwrap_or(false);

                if should_auto_clean {
                    last_auto_clean = now;
                    let rules = default_clean_rules();
                    let summary = execute_clean(&rules);
                    let msg = format!(
                        "Auto-Pulizia attivata da chiusura {}: {} file rimossi ({})",
                        source_label,
                        summary.total_files_deleted,
                        format_bytes(summary.total_bytes_deleted)
                    );
                    auto_msg = Some(msg);
                }

                if let Ok(mut st) = state_arc.lock() {
                    st.total_events_received += 1;
                    if let Some(m) = &auto_msg {
                        st.last_auto_clean_summary = Some(m.clone());
                    }
                    st.events.insert(
                        0,
                        KernelWatchEvent {
                            timestamp: Local::now().format("%H:%M:%S").to_string(),
                            source_label: source_label.to_string(),
                            event_type: kind_str.to_string(),
                            path: path_str,
                            auto_action_triggered: auto_msg,
                        },
                    );
                    if st.events.len() > 80 {
                        st.events.truncate(80);
                    }
                }
            }
        }

        if let Ok(mut st) = state_arc.lock() {
            st.active = false;
        }
    });
}
