# RustCleaner — Multi-Agent Engineering & Collaboration Protocols

This document defines the rules, roles, architectural invariants, and collaboration protocols for AI agents and developers working on the **RustCleaner** codebase.

---

## 1. Project Overview & Core Principles

**RustCleaner** is a high-performance, zero-telemetry, zero-background-RAM Windows system optimizer, registry maintenance utility, software/driver updater, and unallocated disk space wiper written in **Rust 2021**.

All agents working on this project MUST strictly uphold the four core pillars:

1. **Zero Telemetry**: Under NO circumstances may any tracking, analytics, phone-home network calls, or third-party telemetry libraries be added. The only permitted network interactions are official Microsoft utilities (`winget` and Windows Update COM APIs).
2. **0 MB Idle Background RAM**: RustCleaner never runs persistent background background services or daemon loops. Scheduled maintenance (`--silent-clean`) relies exclusively on the native Windows Task Scheduler (`schtasks.exe`), terminating immediately upon job completion.
3. **Single Portable Binary**: The application must build into a single self-contained portable executable without external dynamic dependencies (`+crt-static`). It must run on pristine Windows 10/11 installations without requiring Visual C++ Redistributable runtime DLLs.
4. **Safety-First Architecture**: Performance must never compromise system stability or user data. Irreversible operations require explicit validation, dry-run capabilities, and mandatory rollback backups.

---

## 2. Multi-Agent Team Structure & Ownership

RustCleaner employs a specialized team of 6 domain agents:

| Agent Identifier | Role | Primary Subsystems & Files | Responsibilities |
| :--- | :--- | :--- | :--- |
| **`rustcleaner-architect`** | Lead Architect & Project Orchestrator | Entire workspace, `src/main.rs`, `Cargo.toml` | Task decomposition, cross-agent workflows, API design, architectural integrity, and final code synthesis. |
| **`rustcleaner-win32-security`** | Win32, Registry & Windows Security | `src/registry.rs`, `src/uac.rs`, `src/driver_updater.rs`, `src/startup_uninstall.rs` | Windows Registry scanning/fixing, mandatory `.reg` backups, UAC elevation (`ShellExecuteExW`), COM interfaces, and handle safety. |
| **`rustcleaner-engine-dev`** | Core Engine, Concurrency & Wiper | `src/cleaner.rs`, `src/wiper.rs`, `src/scheduler.rs`, `src/monitor.rs`, `src/software_updater.rs` | Parallel disk traversal (`rayon` + `walkdir`), free space wiping (`FILE_FLAG_WRITE_THROUGH`), Task Scheduler, and kernel monitor. |
| **`rustcleaner-gui-ux`** | Desktop GUI & Localization Specialist | `src/gui.rs`, `src/i18n.rs` | Immediate-mode UI (`egui`/`eframe`), Obsidian Dark/Light themes, non-blocking UI threads, and 7-language localization. |
| **`rustcleaner-qa-safety`** | QA, Safety Invariants & Rollback Auditor | Unit tests, test suites, edge case verification | Invariant enforcement, system path blacklisting, `.reg` rollback verification, dry-run checks, and CI test verification. |
| **`rustcleaner-devops-release`** | DevOps, Build & Release Engineer | `.cargo/config.toml`, `.github/workflows/ci.yml`, `README.md` | Compiler optimization, static CRT linking, CI/CD GitHub Actions, release artifacts, SHA-256 hashes, and Winget packaging. |

---

## 3. Non-Negotiable Safety Invariants

### 3.1 Protected System Directories (Blacklist)
Neither `cleaner.rs`, `wiper.rs`, nor any CLI command may ever delete or overwrite files within:
- `C:\Windows\System32`
- `C:\Windows\SysWOW64`
- `C:\Windows\WinSxS`
- `C:\Windows\Boot` or `bootmgr`
- `C:\Program Files\Windows NT`
- User Personal Directories (`Documents`, `Desktop`, `Pictures`, `Music`, `Videos`) or code repositories.

### 3.2 Mandatory Registry Backup Protocol
- **No registry key or value may ever be modified or deleted without FIRST exporting a valid `.reg` backup** using `registry::export_reg_backup`.
- The `.reg` file must strictly follow the `Windows Registry Editor Version 5.00` format so that the user or an administrator can double-click it to immediately restore the registry state.

### 3.3 Wiper Boundaries
- The free space wiper must ONLY write to dedicated temporary wiper files in the root of the targeted drive (e.g. `X:\_rustcleaner_wipe_*.tmp`).
- Under NO circumstances may raw volume or physical disk device paths (`\\.\PhysicalDriveX`) be passed to the wiper.
- Temporary wipe files must be cleanly deleted on completion, error, or early user cancellation.

---

## 4. Rust Coding & Engineering Guidelines

1. **Error Handling**: Never use naked `.unwrap()` or `.expect()` in production code paths where user input or filesystem conditions can fail. Use `Result<T, E>`, `Option<T>`, or `?` with descriptive error logging.
2. **Handle In-Use Files Gracefully**: When traversing directories, in-use or locked files (`ERROR_SHARING_VIOLATION`, `ERROR_ACCESS_DENIED`) must be skipped and reported as "skipped", never halting the scan.
3. **Localization Discipline**: Never hardcode user-facing strings in `gui.rs`. Every text, label, or tooltip must be registered in `src/i18n.rs` and translated across all 7 supported languages (EN, IT, FR, ES, DE, ZH, PT).
4. **Immediate-Mode GUI Threading**: Never execute blocking I/O, heavy filesystem scans, or network requests on the `egui` update thread. Always spawn background worker threads and exchange state using thread-safe channels (`mpsc`) or atomic primitives.
5. **Static CRT & Compilation**: Keep `target-feature=+crt-static` configured in `.cargo/config.toml`. Verify that every build remains a single, portable executable.
