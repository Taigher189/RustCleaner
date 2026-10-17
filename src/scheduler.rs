//! 3. Pulizia Programmata a Zero RAM (Windows Task Scheduler `schtasks.exe` + `--silent-clean`)
//! Registers or removes a native Windows Task Scheduler job (`RustCleaner_AutoClean`) that invokes
//! `rustcleaner.exe --silent-clean`. When not actively cleaning, the app is 100% closed (0 MB RAM).

use crate::cleaner::{default_clean_rules, execute_clean, format_bytes};
use chrono::Local;
use serde::{Deserialize, Serialize};
use std::env;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::os::windows::process::CommandExt;
use std::path::PathBuf;
use std::process::Command;

const CREATE_NO_WINDOW: u32 = 0x08000000;
pub const TASK_NAME: &str = "RustCleaner_AutoClean";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScheduleFrequency {
    Daily,
    Weekly,
    OnLogon,
}

impl ScheduleFrequency {
    pub fn label(&self) -> &'static str {
        match self {
            ScheduleFrequency::Daily => "Ogni Giorno (DAILY)",
            ScheduleFrequency::Weekly => "Ogni Settimana - Venerdì (WEEKLY)",
            ScheduleFrequency::OnLogon => "Ad ogni Accesso a Windows (ONLOGON)",
        }
    }

    pub fn schtasks_arg(&self) -> &'static str {
        match self {
            ScheduleFrequency::Daily => "DAILY",
            ScheduleFrequency::Weekly => "WEEKLY",
            ScheduleFrequency::OnLogon => "ONLOGON",
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ScheduledTaskStatus {
    pub exists: bool,
    pub task_name: String,
    pub next_run_time: String,
    pub status: String,
    pub last_run_time: String,
    pub schedule_type: String,
    pub command_to_run: String,
}

pub fn silent_log_path() -> PathBuf {
    env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."))
        .join("rustcleaner_scheduled.log")
}

/// Queries `schtasks /Query /TN "RustCleaner_AutoClean" /FO CSV /V` to inspect the current scheduled task.
pub fn query_scheduled_task() -> ScheduledTaskStatus {
    let Ok(output) = Command::new("schtasks")
        .args(["/Query", "/TN", TASK_NAME, "/FO", "LIST", "/V"])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
    else {
        return ScheduledTaskStatus::default();
    };

    if !output.status.success() {
        return ScheduledTaskStatus {
            exists: false,
            task_name: TASK_NAME.to_string(),
            status: "Non attiva (0 MB RAM)".to_string(),
            ..Default::default()
        };
    }

    let text = String::from_utf8_lossy(&output.stdout);
    let mut st = ScheduledTaskStatus {
        exists: true,
        task_name: TASK_NAME.to_string(),
        status: "Attiva in Windows Task Scheduler".to_string(),
        ..Default::default()
    };

    for line in text.lines() {
        if let Some((k, v)) = line.split_once(':') {
            let key = k.trim().to_lowercase();
            let val = v.trim().to_string();
            if (key.contains("next run time") || key.contains("prossima esecuzione"))
                && st.next_run_time.is_empty()
            {
                st.next_run_time = val;
            } else if (key == "status" || key == "stato") && st.status.starts_with("Attiva") {
                st.status = val;
            } else if (key.contains("last run time") || key.contains("ultima esecuzione"))
                && st.last_run_time.is_empty()
            {
                st.last_run_time = val;
            } else if (key.contains("schedule type") || key.contains("tipo di pianificazione"))
                && st.schedule_type.is_empty()
            {
                st.schedule_type = val;
            } else if (key.contains("task to run") || key.contains("attività da eseguire"))
                && st.command_to_run.is_empty()
            {
                st.command_to_run = val;
            }
        }
    }

    st
}

/// Creates or updates the `RustCleaner_AutoClean` task in Windows Task Scheduler via `schtasks.exe`.
pub fn create_scheduled_task(
    freq: ScheduleFrequency,
    time_hhmm: &str,
    run_highest_admin: bool,
) -> Result<String, String> {
    let exe_path = env::current_exe()
        .map_err(|e| format!("Impossibile determinare il percorso dell'eseguibile: {e}"))?;
    let tr_command = format!("\"{}\" --silent-clean", exe_path.display());

    let mut args: Vec<String> = vec![
        "/Create".to_string(),
        "/F".to_string(),
        "/TN".to_string(),
        TASK_NAME.to_string(),
        "/TR".to_string(),
        tr_command.clone(),
        "/SC".to_string(),
        freq.schtasks_arg().to_string(),
    ];

    if freq != ScheduleFrequency::OnLogon {
        args.push("/ST".to_string());
        args.push(time_hhmm.to_string());
    }
    if freq == ScheduleFrequency::Weekly {
        args.push("/D".to_string());
        args.push("FRI".to_string());
    }
    if run_highest_admin {
        args.push("/RL".to_string());
        args.push("HIGHEST".to_string());
    }

    let output = Command::new("schtasks")
        .args(&args)
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| format!("Errore nell'esecuzione di schtasks.exe: {e}"))?;

    if output.status.success() {
        Ok(format!(
            "✅ Pianificazione '{}' creata nell'Utilità di Pianificazione di Windows ({} -> {}). Consumo RAM a riposo: 0 MB!",
            TASK_NAME,
            freq.label(),
            tr_command
        ))
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(format!(
            "⚠️ Impossibile creare l'attività pianificata (se hai spuntato 'Massimi privilegi UAC', avvia prima RustCleaner come Amministratore): {}",
            stderr.trim()
        ))
    }
}

/// Removes the `RustCleaner_AutoClean` scheduled task from Windows Task Scheduler.
pub fn delete_scheduled_task() -> Result<String, String> {
    let output = Command::new("schtasks")
        .args(["/Delete", "/F", "/TN", TASK_NAME])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| format!("Errore nell'esecuzione di schtasks /Delete: {e}"))?;

    if output.status.success() {
        Ok(format!(
            "🗑️ Pianificazione '{TASK_NAME}' rimossa dall'Utilità di Pianificazione di Windows."
        ))
    } else {
        Err(format!(
            "Impossibile rimuovere '{TASK_NAME}': {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ))
    }
}

/// Triggers an immediate run of `RustCleaner_AutoClean` via `schtasks /Run` (or executes silent clean directly).
pub fn run_silent_clean_and_log() -> Result<String, String> {
    let rules = default_clean_rules();
    let summary = execute_clean(&rules);
    let log_line = format!(
        "[{}] SILENT-CLEAN COMPLETATO: Eliminati {} file ({}) | File in uso saltati: {}\r\n",
        Local::now().format("%Y-%m-%d %H:%M:%S"),
        summary.total_files_deleted,
        format_bytes(summary.total_bytes_deleted),
        summary.total_skipped_locked
    );

    let log_file = silent_log_path();
    if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(&log_file) {
        let _ = f.write_all(log_line.as_bytes());
    }

    Ok(log_line.trim().to_string())
}

pub fn read_scheduled_logs() -> Vec<String> {
    let path = silent_log_path();
    let Ok(content) = fs::read_to_string(path) else {
        return Vec::new();
    };
    content
        .lines()
        .rev()
        .take(30)
        .map(|s| s.to_string())
        .collect()
}
