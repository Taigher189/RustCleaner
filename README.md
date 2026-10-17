# 🦀 RustCleaner

[![Status: Mega Alpha](https://img.shields.io/badge/Status-MEGA%20ALPHA%20(v1.0.0--alpha)-red?style=for-the-badge)](#)
[![Rust](https://img.shields.io/badge/Language-Rust%202021-orange?style=for-the-badge&logo=rust)](https://www.rust-lang.org/)
[![Platform](https://img.shields.io/badge/Platform-Windows%2010%20%2F%2011%20(x64)-0078D6?style=for-the-badge&logo=windows)](https://microsoft.com/windows)
[![Zero Telemetry](https://img.shields.io/badge/Privacy-Zero%20Telemetry%20%E2%80%A2%200%20MB%20Background%20RAM-10B981?style=for-the-badge)](#)
[![License: MIT](https://img.shields.io/badge/License-MIT-green?style=for-the-badge)](LICENSE)

🌐 **Languages / Lingue:** [**🇬🇧 English**](#-english-documentation) | [**🇮🇹 Italiano**](#-documentazione-in-italiano)

---

## 🇬🇧 English Documentation

> [!WARNING]
> **⚠️ MEGA ALPHA / EARLY EXPERIMENTAL PREVIEW (`v1.0.0-alpha`)**
> **RustCleaner** is currently in a **Mega Alpha** stage of development. Although it implements non-destructive dry-run analysis and mandatory `.reg` backups before any Windows Registry repair, use it at your own risk and always review the detected items before cleaning.

**RustCleaner** is a fast, lightweight, zero-dependency, **zero-telemetry** Windows system maintenance, privacy, software/driver updater, and disk optimization utility written entirely in **Rust**.

It ships as a single **100% standalone portable `.exe`** (~6.3 MB, statically linked CRT `+crt-static`) containing both a modern **Obsidian Dark Desktop GUI** (built with [`eframe`/`egui`](https://github.com/emilk/egui)) and a full **Command-Line Interface (CLI)** (built with [`clap`](https://github.com/clap-rs/clap)).

### ✨ Core & "Premium-for-Free" Native Architecture (8 Modules)

1. **🧹 File System & Browser Privacy Cleaner (`src/cleaner.rs`)**
   - Multi-threaded parallel scanning (`rayon` + `walkdir`) across `%TEMP%`, `C:\Windows\Temp`, Explorer Thumbnail Cache, Jump Lists, Memory Dumps, Windows Logs, `WER`, Prefetch, the **Windows Recycle Bin** (`SHEmptyRecycleBinW`), and **Chrome, Edge, Firefox, Brave, VS Code & Discord** caches.
   - **Safe Locked-File Handling**: probes files for Win32 lock errors (`ERROR_SHARING_VIOLATION` `32`, `ERROR_LOCK_VIOLATION` `33`, `ERROR_ACCESS_DENIED` `5`) and **safely skips active/locked files without panicking**.

2. **🧩 Windows Registry Maintenance with Mandatory `.reg` Backup (`src/registry.rs`)**
   - Scans `HKLM`, `HKCU`, and `HKCR` via [`winreg`](https://github.com/gentoo90/winreg-rs) across 7 orphan categories (`SharedDLLs`, `App Paths`, `FileExts`, `MuiCache`, `AppCompatFlags`, `Run`, `Installer\Folders`).
   - 🛡️ **Mandatory `.reg` Backup**: automatically exports a standard `Windows Registry Editor Version 5.00` backup (`registry_backups/cc_registry_backup_YYYYMMDD_HHMMSS.reg`) before deleting any Registry entry.

3. **🚀 Startup Manager & Software Uninstaller (`src/startup_uninstall.rs`)**
   - Toggles startup apps non-destructively via Windows' native 12-byte binary flags in `Explorer\StartupApproved\Run` and launches native software uninstallers (`UninstallString` / `msiexec.exe`).

4. **💿 Low-Level Free Space & MFT Wiper (`src/wiper.rs`)**
   - Overwrites unallocated disk space (1-Pass Zero, 3-Pass DoD 5220.22-M, 7-Pass NSA, 35-Pass Gutmann) using 4 MB buffers and `FILE_FLAG_WRITE_THROUGH` (`0x80000000`) to bypass OS cache, guarded by RAII cleanup (`WipeCleanupGuard`).

5. **🔄 Software Updater — Zero Proprietary Databases (`src/software_updater.rs`)**
   - Wraps Microsoft's official **Winget (Windows Package Manager)** (`winget upgrade`), parsing available updates in the background and supporting one-click single or batch silent updates (`winget upgrade --all --silent`).

6. **🎛️ Driver Updater — Official Windows Update COM (`src/driver_updater.rs`)**
   - Avoids unsafe third-party driver databases (and BSOD risks) by querying Microsoft's official **Windows Update Agent (`WUApiLib` / `Microsoft.Update.Session`)** COM interface (`IsInstalled=0 and Type='Driver'`) alongside installed WHQL-signed PnP hardware drivers (`Win32_PnPSignedDriver`).

7. **⏰ Zero-RAM Scheduled Cleaning (`src/scheduler.rs`)**
   - Instead of running a bloated background daemon consuming 50 MB of RAM 24/7, `RustCleaner` registers a native Windows Task Scheduler job (`schtasks.exe`) that invokes `rustcleaner.exe --silent-clean`. When not cleaning, the app is **100% closed (0 MB RAM)**.

8. **⚡ Zero-Polling Real-Time Kernel Monitor (`src/monitor.rs`)**
   - Powered by [`notify`](https://crates.io/crates/notify) and Windows' native `ReadDirectoryChangesW` kernel API. The watcher sleeps at the OS kernel level (**0% CPU polling overhead**) and wakes up only when `%TEMP%` or browser session/lock files change, optionally auto-cleaning browser cache on closure.

---

## 🇮🇹 Documentazione in Italiano

> [!WARNING]
> **⚠️ VERSIONE MEGA ALPHA / ANTEPRIMA SPERIMENTALE (`v1.0.0-alpha`)**
> **RustCleaner** si trova attualmente in stato di **Mega Alpha**. Sebbene includa la modalità di sola analisi (non distruttiva) e il salvataggio obbligatorio del backup `.reg` prima di ogni intervento sul Registro di Windows, il software è sperimentale: utilizzalo con cautela controllando sempre gli elementi rilevati prima della pulizia.

**RustCleaner** è un'utility leggera, veloce, **100% priva di telemetria** e completamente portatile per la pulizia del file system, la manutenzione del registro, l'aggiornamento di software/driver e la tutela della privacy su Windows, scritta interamente in **Rust**.

Viene distribuita come **singolo eseguibile `.exe` 100% standalone** (~6.3 MB, con C Runtime statico `+crt-static`), dotato sia di **interfaccia grafica Desktop Obsidian Dark (GUI)** sia di **interfaccia a riga di comando (CLI)**.

### ✨ Architettura Completa (8 Moduli Nativi + Zero Telemetria)

1. **🧹 Pulizia File System & Privacy Browser (`src/cleaner.rs`)**:
   - Scansione parallela multi-thread (`rayon` + `walkdir`) con gestione sicura dei file bloccati (`ERROR_SHARING_VIOLATION` `32`), svuotamento Cestino (`SHEmptyRecycleBinW`) e pulizia cache di Chrome, Edge, Firefox, Brave, VS Code e Discord.
2. **🧩 Pulizia del Registro con Backup `.reg` Obbligatorio (`src/registry.rs`)**:
   - Analizza `HKLM`, `HKCU` e `HKCR` ed esporta sempre un file `Windows Registry Editor Version 5.00` in `registry_backups/` prima di qualsiasi modifica.
3. **🚀 Gestione Avvio (Startup) & Disinstallazione (`src/startup_uninstall.rs`)**:
   - Abilita/disabilita i programmi all'avvio tramite i flag binari a 12 byte `StartupApproved\Run` e avvia gli uninstaller nativi (`UninstallString` / `msiexec.exe`).
4. **💿 Bonifica dello Spazio Libero (`Wipe Free Space`) (`src/wiper.rs`)**:
   - Sovrascrive lo spazio libero (1 passaggio Zero, 3 passaggi DoD, 7 passaggi NSA, 35 passaggi Gutmann) con buffer da 4 MB e flag `FILE_FLAG_WRITE_THROUGH`.
5. **🔄 Software Updater senza Database Proprietari (`src/software_updater.rs`)**:
   - Interroga in background **Microsoft Winget** (`winget upgrade`), mostrando tutti i programmi che dispongono di un aggiornamento e permettendo l'aggiornamento singolo o totale silenzioso (`winget upgrade --all --silent`).
6. **🎛️ Driver Updater Sicuro tramite Windows Update COM (`src/driver_updater.rs`)**:
   - Evita i pericolosi database di terze parti interrogando direttamente l'infrastruttura ufficiale **Windows Update Agent (`WUApiLib` / `Microsoft.Update.Session`)** (`IsInstalled=0 and Type='Driver'`) e ispezionando i driver hardware PnP certificati WHQL.
7. **⏰ Pulizia Programmata a Zero RAM (`src/scheduler.rs`)**:
   - Sfrutta `schtasks.exe` per registrare un'attività nell'Utilità di Pianificazione di Windows che lancia `rustcleaner.exe --silent-clean`. Quando non pulisce, l'app è completamente chiusa (**0 MB di RAM occupata**).
8. **⚡ Monitoraggio Kernel in Tempo Reale a Zero Polling (`src/monitor.rs`)**:
   - Usa il crate [`notify`](https://crates.io/crates/notify) basato sull'API nativa `ReadDirectoryChangesW`: il thread dorme a livello di kernel (**0% consumo CPU**) e si risveglia solo quando un browser viene chiuso o `%TEMP%` viene modificata.

---

## 🚀 CLI Usage / Comandi CLI

```powershell
.\rustcleaner.exe --help
.\rustcleaner.exe analyze
.\rustcleaner.exe clean
.\rustcleaner.exe --silent-clean
.\rustcleaner.exe registry-scan
.\rustcleaner.exe registry-fix --backup-dir registry_backups
.\rustcleaner.exe startup-list
.\rustcleaner.exe uninstall-list
.\rustcleaner.exe software-updates
.\rustcleaner.exe driver-scan
.\rustcleaner.exe schedule-status
.\rustcleaner.exe wipe-free-space --drive "C:\" --passes 1 --limit-mb 32
```

---

## 🛠️ Build from Source / Compilazione dal Sorgente

```powershell
cargo test
cargo build --release
```

Output binary / Eseguibile generato: `target\release\rustcleaner.exe`

---

## ⚖️ License / Licenza

Distributed under the **MIT License**. See [`LICENSE`](LICENSE) for details.
