# 🦀 RustCleaner

[![Status: Mega Alpha](https://img.shields.io/badge/Status-MEGA%20ALPHA%20(v0.1.0--alpha)-red?style=for-the-badge)](#)
[![Rust](https://img.shields.io/badge/Language-Rust%202021-orange?style=for-the-badge&logo=rust)](https://www.rust-lang.org/)
[![Platform](https://img.shields.io/badge/Platform-Windows%2010%20%2F%2011%20(x64)-0078D6?style=for-the-badge&logo=windows)](https://microsoft.com/windows)
[![License: MIT](https://img.shields.io/badge/License-MIT-green?style=for-the-badge)](LICENSE)

🌐 **Languages / Lingue:** [**🇬🇧 English**](#-english-documentation) | [**🇮🇹 Italiano**](#-documentazione-in-italiano)

---

## 🇬🇧 English Documentation

> [!WARNING]
> **⚠️ MEGA ALPHA / EARLY EXPERIMENTAL PREVIEW (`v0.1.0-alpha`)**
> **RustCleaner** is currently in a **Mega Alpha** stage of development. Although it implements non-destructive dry-run analysis and mandatory `.reg` backups before any Windows Registry repair, use it at your own risk and always review the detected items before cleaning.

**RustCleaner** is a fast, lightweight, zero-dependency Windows system maintenance, privacy, and disk optimization utility written entirely in **Rust** (inspired by classic system utilities like *CCleaner*).

It ships as a single **100% standalone portable `.exe`** (~6.2 MB) containing both a native **Desktop GUI** (built with [`eframe`/`egui`](https://github.com/emilk/egui)) and a full **Command-Line Interface (CLI)** (built with [`clap`](https://github.com/clap-rs/clap)).

### ✨ Key Features

#### 1. 🧹 File System & Browser Privacy Cleaner (`src/cleaner.rs`)
- **Multi-Threaded Parallel Traversal**: powered by [`rayon`](https://github.com/rayon-rs/rayon) and [`walkdir`](https://github.com/BurntSushi/walkdir) for high-speed recursive directory scanning without freezing the UI.
- **Safe Locked / In-Use File Handling**: explicitly probes files for Win32 lock errors (`ERROR_SHARING_VIOLATION` `32`, `ERROR_LOCK_VIOLATION` `33`, `ERROR_ACCESS_DENIED` `5`) and **safely skips active/locked files without panicking or crashing**.
- **Built-in Cleaning Targets**:
  - **Windows Explorer**: Thumbnail Cache (`thumbcache_*.db`), Recent Documents, Jump Lists (`AutomaticDestinations` / `CustomDestinations`).
  - **Windows System**: `%TEMP%`, `C:\Windows\Temp`, Memory Dumps (`*.dmp`, `Minidump`), Windows Logs & `Panther`, `Windows Error Reporting (WER)`, Delivery Optimization cache, Prefetch, and the **Windows Recycle Bin** (via native `SHEmptyRecycleBinW`).
  - **Web Browsers & Apps**: Cache, Code Cache, GPUCache, Cookies, and History for **Google Chrome**, **Microsoft Edge**, **Mozilla Firefox** (dynamic `Profiles\*` discovery), **Brave Browser**, **Windows 11 Widgets**, **VS Code**, and **Discord**.

#### 2. 🧩 Windows Registry Maintenance with Mandatory `.reg` Backup (`src/registry.rs`)
- Scans native Windows Registry hives (`HKLM`, `HKCU`, `HKCR`) via [`winreg`](https://github.com/gentoo90/winreg-rs) across 7 orphan categories:
  - `R_SHAREDDLLS` (Missing Shared DLLs), `R_APPPATHS` (Broken Application Paths), `R_FILEEXTS` (Unused File Extensions), `R_MUICACHE` (Stale MUI Cache entries), `R_APPCOMPAT` (Orphaned AppCompat Flags), `R_RUNSTARTUP` (Broken Startup Registry links), and `R_INSTALLER` (Missing Windows Installer folders).
- 🛡️ **Safety-First `.reg` Backup**: before deleting any Registry value or subkey, `RustCleaner` automatically exports a standard `Windows Registry Editor Version 5.00` backup file (`registry_backups/cc_registry_backup_YYYYMMDD_HHMMSS.reg`) that can be restored with a double-click in `regedit.exe`.

#### 3. 🚀 Startup Manager & Software Uninstaller (`src/startup_uninstall.rs`)
- **Startup Manager**: reads and toggles startup programs from `HKCU`/`HKLM` `Run`, `RunOnce`, `WOW6432Node\Run`, and Startup folders. Enables/disables items non-destructively by writing the native 12-byte binary flag (`0x02` = Enabled, `0x03` = Disabled) in `Explorer\StartupApproved\Run` (compatible with Windows Task Manager).
- **Software Uninstaller**: enumerates installed 64-bit, 32-bit, and user applications from `Uninstall` Registry keys and launches their native `UninstallString` / `msiexec.exe`.

#### 4. 💿 Low-Level Free Space & MFT Wiper (`src/wiper.rs`)
- Overwrites unallocated disk space to prevent forensic recovery of previously deleted files:
  - **1-Pass** Zero Fill (`0x00`), **3-Pass** DoD 5220.22-M, **7-Pass** NSA, and **35-Pass** Peter Gutmann.
- Uses **4 MB aligned buffers** with native Win32 `FILE_FLAG_WRITE_THROUGH` (`0x80000000`) and `FlushFileBuffers` to bypass OS file system caching, protected by an RAII cleanup guard (`WipeCleanupGuard`).

#### 5. 🔑 UAC Privilege Management (`src/uac.rs`)
- Detects Administrator elevation via `OpenProcessToken` + `TokenElevation` and provides one-click UAC self-elevation via `ShellExecuteW(..., "runas", ...)`.

### 🚀 Quick Start (English)

1. **Download**: grab `RustCleaner-Windows-x64-Portable.exe` from the [**Releases**](https://github.com/Taigher189/RustCleaner/releases) page (no installation or extra DLLs required).
2. **Launch Desktop GUI**: double-click the `.exe` or run it without arguments.
3. **CLI Commands**:
   ```powershell
   .\rustcleaner.exe --help
   .\rustcleaner.exe analyze
   .\rustcleaner.exe clean
   .\rustcleaner.exe registry-scan
   .\rustcleaner.exe registry-fix --backup-dir registry_backups
   .\rustcleaner.exe startup-list
   .\rustcleaner.exe uninstall-list
   .\rustcleaner.exe wipe-free-space --drive "C:\" --passes 1 --limit-mb 32
   ```

---

## 🇮🇹 Documentazione in Italiano

> [!WARNING]
> **⚠️ VERSIONE MEGA ALPHA / ANTEPRIMA SPERIMENTALE (`v0.1.0-alpha`)**
> **RustCleaner** si trova attualmente in stato di **Mega Alpha**. Sebbene includa la modalità di sola analisi (non distruttiva) e il salvataggio obbligatorio del backup `.reg` prima di ogni intervento sul Registro di Windows, il software è sperimentale: utilizzalo con cautela controllando sempre gli elementi rilevati prima della pulizia.

**RustCleaner** è un'utility leggera, veloce e completamente portatile per la pulizia del file system, la tutela della privacy e la manutenzione di Windows scritta interamente in **Rust** (ispirata alle classiche utility di sistema come *CCleaner*).

Viene distribuita come **singolo eseguibile `.exe` 100% standalone** (~6.2 MB) con C Runtime statico (`+crt-static`), dotato sia di **interfaccia grafica Desktop (GUI)** basata su [`eframe`/`egui`](https://github.com/emilk/egui) sia di **interfaccia a riga di comando (CLI)** basata su [`clap`](https://github.com/clap-rs/clap).

### ✨ Caratteristiche Principali

1. **🧹 Pulizia File System & Privacy Browser (`src/cleaner.rs`)**:
   - Scansione parallela multi-thread (`rayon` + `walkdir`) di `%TEMP%`, `C:\Windows\Temp`, Cache Miniature Explorer, Jump Lists, Memory Dump, Log di Windows, `WER`, Prefetch, **Cestino di Windows** (`SHEmptyRecycleBinW`) e cache/cookie/cronologia di **Chrome, Edge, Firefox, Brave, VS Code e Discord**.
   - **Gestione Sicura dei File "In Uso"**: intercetta `ERROR_SHARING_VIOLATION` (`os error 32`), `ERROR_LOCK_VIOLATION` (`33`) e `ERROR_ACCESS_DENIED` (`5`), saltando i file bloccati senza mai andare in panic.
2. **🧩 Pulizia del Registro con Backup `.reg` Obbligatorio (`src/registry.rs`)**:
   - Analizza `HKLM`, `HKCU` e `HKCR` alla ricerca di DLL condivise orfane (`SharedDLLs`), `App Paths`, estensioni file inutilizzate (`FileExts`), `MuiCache`, `AppCompatFlags`, voci `Run` orfane e cartelle `Installer`.
   - **Backup `.reg` Automatico**: esporta sempre un file `Windows Registry Editor Version 5.00` in `registry_backups/` prima di applicare qualsiasi modifica.
3. **🚀 Gestione Avvio (Startup) & Disinstallazione (`src/startup_uninstall.rs`)**:
   - Abilita/disabilita i programmi all'avvio tramite i flag binari a 12 byte `StartupApproved\Run` (compatibile con Gestione Attività di Windows) e avvia gli uninstaller nativi (`UninstallString` / `msiexec.exe`).
4. **💿 Bonifica dello Spazio Libero (`Wipe Free Space`) (`src/wiper.rs`)**:
   - Sovrascrive lo spazio libero (1 passaggio Zero, 3 passaggi DoD, 7 passaggi NSA, 35 passaggi Gutmann) con buffer da 4 MB e flag `FILE_FLAG_WRITE_THROUGH` per bypassare la cache del sistema operativo.
5. **🔑 Gestione Privilegi UAC (`src/uac.rs`)**:
   - Verifica il token amministrativo (`TokenElevation`) e permette il riavvio come Amministratore (`ShellExecuteW("runas")`) con un clic.

---

## 🛠️ Build from Source / Compilazione dal Sorgente

```powershell
# Run unit tests / Esegui i test unitari
cargo test

# Build standalone portable release EXE / Compila l'eseguibile portatile
cargo build --release
```

Output binary / Eseguibile generato: `target\release\rustcleaner.exe`

---

## ⚖️ License / Licenza

Distributed under the **MIT License**. See [`LICENSE`](LICENSE) for details.
