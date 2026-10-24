# 🦀 RustCleaner

[![Status: Alpha](https://img.shields.io/badge/Status-v1.4.0--alpha-red?style=for-the-badge)](#)
[![Rust](https://img.shields.io/badge/Language-Rust%202021-orange?style=for-the-badge&logo=rust)](https://www.rust-lang.org/)
[![Platform](https://img.shields.io/badge/Platform-Windows%2010%20%2F%2011%20(x64)-0078D6?style=for-the-badge&logo=windows)](https://microsoft.com/windows)
[![Privacy](https://img.shields.io/badge/Privacy-Zero%20Telemetry%20%E2%80%A2%200%20MB%20Idle%20RAM-10B981?style=for-the-badge)](#)
[![License: MIT](https://img.shields.io/badge/License-MIT-green?style=for-the-badge)](LICENSE)

**RustCleaner** is a lightweight, zero-dependency, **zero-telemetry** Windows system optimizer, cleaner, privacy tool, software/driver updater, and disk sanitizer written in **Rust**.

Ships as a single **standalone portable `.exe`** with a responsive **Dark/Light Desktop GUI** ([`eframe`/`egui`](https://github.com/emilk/egui)) and a full **Command-Line Interface (CLI)** ([`clap`](https://github.com/clap-rs/clap)).

---

## ✨ Features

- **🧹 File System & Browser Cleaner**: Multi-threaded parallel scanning (`rayon` + `walkdir`) across temp folders, logs, crash dumps, Recycle Bin (`SHEmptyRecycleBinW`), and browser/app caches (Chrome, Edge, Firefox, Brave, VS Code, Discord). Safely skips locked files without errors.
- **🔍 Disk Space Analyzer & SHA-256 Duplicate Finder**: Multi-stage duplicate detection (Exact Size -> 8 KB Boundary Fingerprint -> Full Cryptographic SHA-256), large file hunter, and interactive category disk breakdown with safe duplicate deletion.
- **🧩 Registry Cleaner with Mandatory Backup**: Scans orphaned keys (`SharedDLLs`, `App Paths`, `FileExts`, `MuiCache`, `AppCompatFlags`, `Run`, `Installer`) with automatic `Windows Registry Editor Version 5.00` (`.reg`) backup before changes.
- **🚀 Startup & Uninstaller Manager**: Inspects and toggles startup entries via native `StartupApproved\Run` flags and launches uninstallers with automatic UAC elevation support.
- **💿 Low-Level Free Space Wiper**: Sanitizes unallocated drive space (Zero, DoD 5220.22-M, NSA, Gutmann) using unbuffered writes (`FILE_FLAG_WRITE_THROUGH`) while leaving user files and Windows system untouched.
- **🔄 Software Updater (Winget)**: Background query via official Microsoft Winget (`winget upgrade`) with one-click single or batch silent upgrades.
- **🎛️ Driver Updater (Official Windows Update)**: Inspects signed PnP hardware drivers and checks Microsoft's official Windows Update Agent (`WUApiLib` COM `IsInstalled=0 and Type='Driver'`) without third-party driver databases.
- **⏰ Zero-RAM Scheduled Tasks**: Leverages Windows Task Scheduler (`schtasks.exe`) to run silent maintenance (`--silent-clean`) with **0 MB background RAM** when idle.
- **⚡ Real-Time Kernel Monitor**: Zero-polling event monitor powered by Win32 `ReadDirectoryChangesW` (`notify`) to detect browser closure and temp file generation at 0% CPU.
- **🌐 7 Built-In UI Languages**: Runtime language switcher for English, Italian, French, Spanish, German, Chinese, and Portuguese.

---

## 🚀 CLI Usage

```powershell
# Display help and available commands
.\rustcleaner.exe --help

# Analyze files and caches (read-only dry run)
.\rustcleaner.exe analyze

# Execute cleaning
.\rustcleaner.exe clean

# Silent background cleaning (used by Task Scheduler)
.\rustcleaner.exe --silent-clean

# Scan and repair Windows Registry
.\rustcleaner.exe registry-scan
.\rustcleaner.exe registry-fix --backup-dir registry_backups

# List startup and installed applications
.\rustcleaner.exe startup-list
.\rustcleaner.exe uninstall-list

# Check for software and driver updates
.\rustcleaner.exe software-updates
.\rustcleaner.exe driver-scan

# Check scheduled task status
.\rustcleaner.exe schedule-status

# Wipe free space on drive
.\rustcleaner.exe wipe-free-space --drive "C:\" --passes 1 --limit-mb 32
```

---

## 🛠️ Build from Source

Requirements: Rust stable toolchain (MSVC or MinGW).

```powershell
# Run unit tests
cargo test

# Build optimized release executable
cargo build --release
```

Output binary: `target\release\rustcleaner.exe`

---

## ⚖️ License

Distributed under the **MIT License**. See [`LICENSE`](LICENSE) for details.
