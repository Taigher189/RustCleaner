#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
//! RustCleaner [MEGA ALPHA] — Windows System Cleaner, Registry Maintenance & Optimizer in Rust.
//! Supports both a classic Desktop GUI (`eframe`/`egui`) and full CLI subcommands (`clap`).

pub mod cleaner;
pub mod gui;
pub mod registry;
pub mod startup_uninstall;
pub mod uac;
pub mod wiper;

use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

#[derive(Parser)]
#[command(
    name = "rustcleaner",
    version = "0.1.0-alpha",
    about = "[MEGA ALPHA] Utility di pulizia e ottimizzazione per Windows scritta in Rust (Pulizia File/Browser, Registro con Backup .reg, Startup, Uninstaller e Bonifica Spazio Libero)"
)]
struct Cli {
    /// Richiede automaticamente l'elevazione UAC (Amministratore) all'avvio se non già elevato
    #[arg(long)]
    elevate: bool,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// 1A. Scansiona il File System e le cache dei Browser (sola lettura)
    Analyze,
    /// 1B. Esegue la pulizia dei file temporanei e cache saltando i file in uso
    Clean,
    /// 2A. Scansiona il Registro di Windows alla ricerca di chiavi/valori orfani
    RegistryScan,
    /// 2B. Esporta un backup .reg e ripara i problemi del Registro
    RegistryFix {
        /// Cartella in cui salvare il file di backup .reg prima della pulizia
        #[arg(long, default_value = "registry_backups")]
        backup_dir: PathBuf,
    },
    /// 3A. Elenca i programmi in esecuzione automatica (Startup)
    StartupList,
    /// 3B. Elenca i software installati e i relativi UninstallString
    UninstallList,
    /// 4. Bonifica lo spazio libero su disco (Wipe Free Space)
    WipeFreeSpace {
        /// Volume da bonificare (es. "C:\")
        #[arg(long, default_value = "C:\\")]
        drive: String,
        /// Numero di passaggi (1 = Zero, 3 = DoD, 7 = NSA, 35 = Gutmann)
        #[arg(long, default_value_t = 1)]
        passes: u32,
        /// Limite opzionale in MB (per test veloci e sicuri)
        #[arg(long, default_value_t = 32)]
        limit_mb: u64,
    },
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // In release builds (`windows_subsystem = "windows"`), attach to the parent terminal
    // if the user invoked the binary with CLI arguments (e.g. `rustcleaner.exe analyze` or `--help`).
    #[cfg(windows)]
    if std::env::args_os().len() > 1 {
        unsafe {
            use windows_sys::Win32::System::Console::{AttachConsole, ATTACH_PARENT_PROCESS};
            let _ = AttachConsole(ATTACH_PARENT_PROCESS);
        }
    }

    let cli = Cli::parse();

    if cli.elevate && !uac::is_elevated() {
        println!("[UAC] Richiesta privilegi di Amministratore tramite ShellExecuteW(\"runas\")...");
        uac::relaunch_as_admin(&[])?;
        return Ok(());
    }

    match cli.command {
        None => {
            // Launch Desktop GUI
            let native_options = eframe::NativeOptions {
                viewport: eframe::egui::ViewportBuilder::default()
                    .with_inner_size([1180.0, 720.0])
                    .with_min_inner_size([980.0, 620.0])
                    .with_title("RustCleaner v0.1.0-alpha [MEGA ALPHA] — Windows System Cleaner"),
                ..Default::default()
            };
            eframe::run_native(
                "RustCleaner",
                native_options,
                Box::new(|cc| {
                    cc.egui_ctx.set_visuals(eframe::egui::Visuals::dark());
                    Ok(Box::new(gui::RustCleanerApp::default()))
                }),
            )?;
        }
        Some(Commands::Analyze) => {
            println!(
                "=== RustCleaner: Analisi File System & Privacy (UAC Admin: {}) ===",
                uac::is_elevated()
            );
            let rules = cleaner::default_clean_rules();
            let summary = cleaner::analyze_all(&rules);
            for r in &summary.results {
                if !r.files.is_empty() {
                    println!(
                        "  [{}] {}: {} file ({}) — {} bloccati/in uso",
                        r.rule_id,
                        r.rule_name,
                        r.files.len(),
                        cleaner::format_bytes(r.total_bytes),
                        r.locked_count
                    );
                }
            }
            println!(
                "\nTotale: {} file ({}) | File in uso saltati: {}",
                summary.total_files_found,
                cleaner::format_bytes(summary.total_bytes_found),
                summary.total_skipped_locked
            );
        }
        Some(Commands::Clean) => {
            println!("=== RustCleaner: Pulizia File System & Privacy ===");
            let rules = cleaner::default_clean_rules();
            let summary = cleaner::execute_clean(&rules);
            println!(
                "Eliminati: {} file ({}) | Saltati perché in uso: {}",
                summary.total_files_deleted,
                cleaner::format_bytes(summary.total_bytes_deleted),
                summary.total_skipped_locked
            );
        }
        Some(Commands::RegistryScan) => {
            println!("=== RustCleaner: Scansione Registro di Windows ===");
            let issues = registry::scan_registry(registry::RegistryCategory::all());
            for (i, issue) in issues.iter().enumerate() {
                println!(
                    "  {:03}. [{}] {} -> {} ({})",
                    i + 1,
                    issue.category.code(),
                    issue.problem_description,
                    issue.data_reference,
                    issue.full_reg_path()
                );
            }
            println!("\nTotale incongruenze rilevate: {}", issues.len());
        }
        Some(Commands::RegistryFix { backup_dir }) => {
            let issues = registry::scan_registry(registry::RegistryCategory::all());
            if issues.is_empty() {
                println!("Nessun problema del registro da riparare.");
                return Ok(());
            }
            let report = registry::backup_and_fix_registry(&issues, &backup_dir)?;
            println!(
                "Backup .reg salvato in: {}\nVoci riparate: {} (Errori: {})",
                report.backup_file.display(),
                report.fixed_count,
                report.failed_count
            );
        }
        Some(Commands::StartupList) => {
            println!("=== RustCleaner: Programmi all'Avvio (Startup) ===");
            let entries = startup_uninstall::list_startup_entries();
            for e in &entries {
                println!(
                    "  [{}] {:14} | {:30} | {}",
                    if e.enabled { "ON " } else { "OFF" },
                    e.source.key_label(),
                    e.name,
                    e.command
                );
            }
            println!("\nTotale voci di avvio: {}", entries.len());
        }
        Some(Commands::UninstallList) => {
            println!("=== RustCleaner: Software Installati (Uninstaller) ===");
            let apps = startup_uninstall::list_installed_apps();
            for a in apps.iter().take(40) {
                println!(
                    "  • {} (v{}) — {} | {}",
                    a.display_name, a.display_version, a.publisher, a.uninstall_string
                );
            }
            println!(
                "\nTotale programmi installati rilevati: {} (mostrati primi 40)",
                apps.len()
            );
        }
        Some(Commands::WipeFreeSpace {
            drive,
            passes,
            limit_mb,
        }) => {
            let alg = match passes {
                3 => wiper::WipeAlgorithm::DodThreePass,
                7 => wiper::WipeAlgorithm::NsaSevenPass,
                35 => wiper::WipeAlgorithm::GutmannThirtyFivePass,
                _ => wiper::WipeAlgorithm::SimpleOnePassZero,
            };
            println!(
                "=== RustCleaner: Bonifica Spazio Libero su {} ({}, limite: {} MB) ===",
                drive,
                alg.label(),
                limit_mb
            );
            let prog = Arc::new(Mutex::new(wiper::WipeProgress::default()));
            let cancel = Arc::new(AtomicBool::new(false));
            let written = wiper::wipe_free_space(&drive, alg, true, Some(limit_mb), prog.clone(), cancel)?;
            let final_msg = prog.lock().unwrap().status_message.clone();
            println!("{final_msg} (Byte scritti: {written})");
        }
    }

    Ok(())
}
