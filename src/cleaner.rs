//! File System & Browser Privacy Cleaner Engine
//! Reconstructed from CCleaner64.exe embedded resources:
//! - `INI/270/1033` (`winsys.ini` @ 0x2192bf0, Revision 2089)
//! - `INI/273/1033` (`winapp.ini` @ 0x2199758)
//! - SpecialKey handlers (`N_TEMP_DIRS` @ 0x19e7d48, `N_TEMP_RECYCLEBIN` @ 0x19e7d60,
//!   `N_CHROME_*`, `N_EDGE_CHROMIUM_*` @ 0x19e8c18..0x19e8c90, `N_BRAVE_*`, `N_VIVALDI_*`, `N_OPERA_GX_*`).

use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::env;
use std::fs::{self, OpenOptions};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;
use windows_sys::Win32::UI::Shell::{
    SHEmptyRecycleBinW, SHERB_NOCONFIRMATION, SHERB_NOPROGRESSUI, SHERB_NOSOUND,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RuleGroup {
    WindowsSystem,
    WindowsExplorer,
    BrowsersPrivacy,
    Applications,
}

impl RuleGroup {
    pub fn label(&self) -> &'static str {
        match self {
            RuleGroup::WindowsSystem => "Sistema Windows",
            RuleGroup::WindowsExplorer => "Esplora File (Windows Explorer)",
            RuleGroup::BrowsersPrivacy => "Browser & Privacy (Chrome, Edge, Firefox, Brave)",
            RuleGroup::Applications => "Applicazioni & Strumenti",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileTarget {
    /// Directory path supporting `%VAR%` expansion (e.g. `%TEMP%`, `%LOCALAPPDATA%\Google\Chrome\User Data\Default\Cache`)
    pub base_dir: String,
    /// Semicolon-separated glob masks (e.g. `*.*` or `*.log;*.dmp;*.tmp`)
    pub patterns: Vec<String>,
    /// Whether to traverse subdirectories (`RECURSE` or `REMOVESELF` in CCleaner INI syntax)
    pub recursive: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CleanRule {
    pub id: u32,
    pub name: String,
    pub group: RuleGroup,
    pub description: String,
    pub enabled: bool,
    pub requires_admin: bool,
    pub special_recycle_bin: bool,
    pub targets: Vec<FileTarget>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScannedFileItem {
    pub path: PathBuf,
    pub size_bytes: u64,
    pub locked_or_denied: bool,
    pub error_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleScanResult {
    pub rule_id: u32,
    pub rule_name: String,
    pub group: RuleGroup,
    pub files: Vec<ScannedFileItem>,
    pub total_bytes: u64,
    pub cleanable_bytes: u64,
    pub locked_count: usize,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CleanSummary {
    pub results: Vec<RuleScanResult>,
    pub total_files_found: usize,
    pub total_bytes_found: u64,
    pub total_files_deleted: usize,
    pub total_bytes_deleted: u64,
    pub total_skipped_locked: usize,
    pub errors: Vec<String>,
}

/// Expands Windows-style `%ENV_VAR%` tokens in a path string, matching CCleaner's INI path resolver.
pub fn expand_env_vars(input: &str) -> PathBuf {
    let mut out = input.to_string();
    let mappings = [
        ("%TEMP%", env::var("TEMP").unwrap_or_else(|_| r"C:\Windows\Temp".to_string())),
        ("%TMP%", env::var("TMP").unwrap_or_else(|_| r"C:\Windows\Temp".to_string())),
        ("%WINDIR%", env::var("WINDIR").unwrap_or_else(|_| r"C:\Windows".to_string())),
        ("%windir%", env::var("WINDIR").unwrap_or_else(|_| r"C:\Windows".to_string())),
        ("%WinDir%", env::var("WINDIR").unwrap_or_else(|_| r"C:\Windows".to_string())),
        (
            "%SystemDirectory%",
            format!(
                r"{}\System32",
                env::var("WINDIR").unwrap_or_else(|_| r"C:\Windows".to_string())
            ),
        ),
        (
            "%SystemDrive%",
            env::var("SystemDrive").unwrap_or_else(|_| "C:".to_string()),
        ),
        (
            "%LOCALAPPDATA%",
            env::var("LOCALAPPDATA").unwrap_or_default(),
        ),
        (
            "%LocalAppData%",
            env::var("LOCALAPPDATA").unwrap_or_default(),
        ),
        (
            "%LocalLowAppData%",
            env::var("USERPROFILE")
                .map(|u| format!(r"{u}\AppData\LocalLow"))
                .unwrap_or_default(),
        ),
        ("%APPDATA%", env::var("APPDATA").unwrap_or_default()),
        ("%AppData%", env::var("APPDATA").unwrap_or_default()),
        ("%USERPROFILE%", env::var("USERPROFILE").unwrap_or_default()),
        ("%UserProfile%", env::var("USERPROFILE").unwrap_or_default()),
        (
            "%ProgramData%",
            env::var("ProgramData").unwrap_or_else(|_| r"C:\ProgramData".to_string()),
        ),
        (
            "%CommonAppData%",
            env::var("ProgramData").unwrap_or_else(|_| r"C:\ProgramData".to_string()),
        ),
        (
            "%commonappdata%",
            env::var("ProgramData").unwrap_or_else(|_| r"C:\ProgramData".to_string()),
        ),
        (
            "%ALLUSERSPROFILE%",
            env::var("ALLUSERSPROFILE").unwrap_or_else(|_| r"C:\ProgramData".to_string()),
        ),
        (
            "%ProgramFiles%",
            env::var("ProgramFiles").unwrap_or_else(|_| r"C:\Program Files".to_string()),
        ),
    ];

    for (token, val) in mappings {
        if out.contains(token) && !val.is_empty() {
            out = out.replace(token, &val);
        }
    }

    // Also expand any remaining generic `%ENV_VAR%` tokens
    while let Some(start) = out.find('%') {
        if let Some(rel_end) = out[start + 1..].find('%') {
            let end = start + 1 + rel_end;
            let var_name = &out[start + 1..end];
            if let Ok(val) = env::var(var_name) {
                out.replace_range(start..=end, &val);
                continue;
            }
        }
        break;
    }

    PathBuf::from(out)
}

/// Checks if a file name matches a CCleaner pattern like `*.*`, `*.log`, `thumbcache_*.db`, etc.
fn matches_pattern(file_name: &str, pattern: &str) -> bool {
    let pat = pattern.trim().to_lowercase();
    let name = file_name.to_lowercase();
    if pat == "*.*" || pat == "*" {
        return true;
    }
    if let Some(ext) = pat.strip_prefix("*.") {
        return name.ends_with(&format!(".{ext}"));
    }
    if let Some(prefix) = pat.strip_suffix("*.*") {
        return name.starts_with(prefix);
    }
    if pat.contains('*') || pat.contains('?') {
        if let Ok(glob_pat) = glob::Pattern::new(&pat) {
            return glob_pat.matches(&name);
        }
    }
    name == pat
}

/// Returns `true` if `path` or any parent of `path` falls inside protected system or user directories:
/// `System32`, `SysWOW64`, `WinSxS`, `Boot`, `bootmgr`, `Program Files\Windows NT`,
/// user personal folders (`Documents`, `Desktop`, `Pictures`, `Music`, `Videos`),
/// code repositories, or root volume devices.
pub fn is_protected_system_path(path: &Path) -> bool {
    let raw_str = path.to_string_lossy();
    let trimmed = raw_str.trim();
    if trimmed.is_empty() {
        return true;
    }

    // Strip \\?\ verbatim prefix if present
    let clean_str = if let Some(stripped) = trimmed.strip_prefix(r"\\?\") {
        stripped
    } else {
        trimmed
    };

    let norm = clean_str.replace('/', "\\").to_lowercase();

    // 1. Device and physical drive paths
    if norm.starts_with(r"\\.\")
        || norm.starts_with(r"\??\")
        || norm.contains("physicaldrive")
        || norm.contains("volume{")
    {
        return true;
    }

    // 2. Drive root protection (e.g. "C:\", "C:", "\")
    if norm == "\\"
        || (norm.len() == 2 && norm.ends_with(':'))
        || (norm.len() == 3 && norm.ends_with(r":\"))
    {
        return true;
    }

    // 3. bootmgr file
    if norm == "bootmgr" || norm.ends_with(r"\bootmgr") {
        return true;
    }
    if let Some(file_name) = path.file_name().and_then(|f| f.to_str()) {
        if file_name.eq_ignore_ascii_case("bootmgr") {
            return true;
        }
    }

    // 4. Windows System Directories
    let windir = env::var("WINDIR")
        .or_else(|_| env::var("SystemRoot"))
        .unwrap_or_else(|_| r"C:\Windows".to_string())
        .replace('/', "\\")
        .to_lowercase();

    let protected_system_dirs = [
        format!(r"{windir}\system32"),
        format!(r"{windir}\syswow64"),
        format!(r"{windir}\winsxs"),
        format!(r"{windir}\boot"),
        r"c:\windows\system32".to_string(),
        r"c:\windows\syswow64".to_string(),
        r"c:\windows\winsxs".to_string(),
        r"c:\windows\boot".to_string(),
    ];

    for sys_dir in &protected_system_dirs {
        if norm == *sys_dir || norm.starts_with(&format!(r"{sys_dir}\")) {
            return true;
        }
    }

    // 5. Program Files \ Windows NT
    let pf = env::var("ProgramFiles")
        .unwrap_or_else(|_| r"C:\Program Files".to_string())
        .replace('/', "\\")
        .to_lowercase();
    let pf86 = env::var("ProgramFiles(x86)")
        .unwrap_or_else(|_| r"C:\Program Files (x86)".to_string())
        .replace('/', "\\")
        .to_lowercase();

    let protected_pf_dirs = [
        format!(r"{pf}\windows nt"),
        format!(r"{pf86}\windows nt"),
        r"c:\program files\windows nt".to_string(),
        r"c:\program files (x86)\windows nt".to_string(),
    ];

    for dir in &protected_pf_dirs {
        if norm == *dir || norm.starts_with(&format!(r"{dir}\")) {
            return true;
        }
    }

    // 6. User Personal Directories: Documents, Desktop, Pictures, Music, Videos
    let personal_folders = [
        "documents",
        "desktop",
        "pictures",
        "music",
        "videos",
        "my documents",
        "my pictures",
        "my music",
        "my videos",
    ];

    // Check against USERPROFILE if set
    if let Ok(userprofile) = env::var("USERPROFILE") {
        let up = userprofile.replace('/', "\\").to_lowercase();
        for folder in &personal_folders {
            let target = format!(r"{up}\{folder}");
            if norm == target || norm.starts_with(&format!(r"{target}\")) {
                return true;
            }
            // Also OneDrive user folders (e.g. %USERPROFILE%\OneDrive\Documents)
            let onedrive_target = format!(r"{up}\onedrive\{folder}");
            if norm == onedrive_target || norm.starts_with(&format!(r"{onedrive_target}\")) {
                return true;
            }
        }
    }

    // Generic pattern check: \users\<username>\<personal_folder> (and with \onedrive\)
    if let Some(pos) = norm.find(r"\users\") {
        let after_users = &norm[pos + 7..];
        let segments: Vec<&str> = after_users.split('\\').collect();
        if segments.len() >= 2 {
            let sub = segments[1];
            if personal_folders.contains(&sub) {
                return true;
            }
            if sub == "onedrive" && segments.len() >= 3 && personal_folders.contains(&segments[2]) {
                return true;
            }
        }
    }

    // 7. Code Repositories
    for comp in path.components() {
        if let Some(comp_str) = comp.as_os_str().to_str() {
            let c_lower = comp_str.to_lowercase();
            if c_lower == ".git" || c_lower == ".hg" || c_lower == ".svn" {
                return true;
            }
        }
    }

    if norm.contains(r"\.git\") || norm.ends_with(r"\.git") {
        return true;
    }

    // Check if path or any ancestor contains .git or Cargo.toml
    let check_path = if path.is_file() {
        path.parent()
    } else {
        Some(path)
    };
    if let Some(mut curr) = check_path {
        for _ in 0..16 {
            if curr.join(".git").exists() || curr.join("Cargo.toml").exists() {
                return true;
            }
            match curr.parent() {
                Some(p) => curr = p,
                None => break,
            }
        }
    }

    false
}

/// Tests if a file is currently locked/in-use by another process or protected by OS permissions.
/// Never mutates file permissions on disk (strictly read-only analysis).
/// Never panics: returns `(is_locked, reason)`.
fn probe_file_lock(path: &Path) -> (bool, Option<String>) {
    if is_protected_system_path(path) {
        return (true, Some("Percorso protetto dal sistema".to_string()));
    }

    // desktop.ini and PFRO.log are excluded by CCleaner (`0x1a090c0`, `winsys.ini:434`)
    if let Some(fname) = path.file_name().and_then(|n| n.to_str()) {
        let lower = fname.to_lowercase();
        if lower == "desktop.ini" || lower == "pfro.log" || lower == "info2" {
            return (true, Some("File di sistema protetto (escluso da CCleaner)".to_string()));
        }
    }

    let is_readonly = path.metadata().map(|m| m.permissions().readonly()).unwrap_or(false);

    if is_readonly {
        // Do NOT mutate permissions during read-only analysis.
        // Test lock status via read open.
        match OpenOptions::new().read(true).open(path) {
            Ok(_) => (false, None),
            Err(e) => {
                let code = e.raw_os_error().unwrap_or(0);
                let msg = match code {
                    32 | 33 => format!("File in uso da un altro processo (OS error {code})"),
                    5 => "Accesso negato (richiede privilegi UAC o file protetto)".to_string(),
                    _ => format!("Impossibile accedere: {e}"),
                };
                (true, Some(msg))
            }
        }
    } else {
        match OpenOptions::new().write(true).open(path) {
            Ok(_) => (false, None),
            Err(e) => {
                let code = e.raw_os_error().unwrap_or(0);
                let msg = match code {
                    32 | 33 => format!("File in uso da un altro processo (OS error {code})"),
                    5 => "Accesso negato (richiede privilegi UAC o file protetto)".to_string(),
                    _ => format!("Impossibile accedere: {e}"),
                };
                (true, Some(msg))
            }
        }
    }
}

/// Built-in CCleaner rules extracted from `winsys.ini` (`INI/270/1033`) and `winapp.ini` (`INI/273/1033`).
pub fn default_clean_rules() -> Vec<CleanRule> {
    vec![
        // === SISTEMA WINDOWS (`winsys.ini` LangSecRef=3003) ===
        CleanRule {
            id: 1013,
            name: "File Temporanei (%TEMP% e Windows Temp)".to_string(),
            group: RuleGroup::WindowsSystem,
            description: "File temporanei utente (%TEMP%) e di sistema (%WINDIR%\\Temp) [SpecialKey1=N_TEMP_DIRS]".to_string(),
            enabled: true,
            requires_admin: false,
            special_recycle_bin: false,
            targets: vec![
                FileTarget {
                    base_dir: "%TEMP%".to_string(),
                    patterns: vec!["*.*".to_string()],
                    recursive: true,
                },
                FileTarget {
                    base_dir: r"%WINDIR%\Temp".to_string(),
                    patterns: vec!["*.*".to_string()],
                    recursive: true,
                },
            ],
        },
        CleanRule {
            id: 1012,
            name: "Svuota Cestino (Recycle Bin)".to_string(),
            group: RuleGroup::WindowsSystem,
            description: "File eliminati nel Cestino di tutti i volumi [$Recycle.Bin / N_TEMP_RECYCLEBIN]".to_string(),
            enabled: false, // Safe default so user explicitly opts in to empty Recycle Bin
            requires_admin: false,
            special_recycle_bin: true,
            targets: vec![FileTarget {
                base_dir: r"%SystemDrive%\$Recycle.Bin".to_string(),
                patterns: vec!["*.*".to_string()],
                recursive: true,
            }],
        },
        CleanRule {
            id: 1015,
            name: "Memory Dumps (Crash Dump di Sistema e App)".to_string(),
            group: RuleGroup::WindowsSystem,
            description: "Dump di memoria del kernel e crash dump utente (%WINDIR%\\MiniDump, %LocalAppData%\\CrashDumps)".to_string(),
            enabled: true,
            requires_admin: true,
            special_recycle_bin: false,
            targets: vec![
                FileTarget {
                    base_dir: r"%WINDIR%\MiniDump".to_string(),
                    patterns: vec!["*.dmp".to_string()],
                    recursive: false,
                },
                FileTarget {
                    base_dir: r"%WINDIR%\LiveKernelReports".to_string(),
                    patterns: vec!["*.dmp".to_string()],
                    recursive: true,
                },
                FileTarget {
                    base_dir: r"%LocalAppData%\CrashDumps".to_string(),
                    patterns: vec!["*.dmp".to_string()],
                    recursive: false,
                },
            ],
        },
        CleanRule {
            id: 1017,
            name: "File di Log di Windows".to_string(),
            group: RuleGroup::WindowsSystem,
            description: "Log di sistema WBEM, Panther, CBS e SoftwareDistribution [winsys.ini ID=1017]".to_string(),
            enabled: true,
            requires_admin: true,
            special_recycle_bin: false,
            targets: vec![
                FileTarget {
                    base_dir: r"%WINDIR%\Logs".to_string(),
                    patterns: vec!["*.log".to_string(), "*.lo_".to_string()],
                    recursive: false,
                },
                FileTarget {
                    base_dir: r"%WINDIR%\Logs".to_string(),
                    patterns: vec!["*.log".to_string()],
                    recursive: true,
                },
                FileTarget {
                    base_dir: r"%WINDIR%\Panther".to_string(),
                    patterns: vec!["setupact.log".to_string(), "setuperr.log".to_string()],
                    recursive: true,
                },
                FileTarget {
                    base_dir: r"%WINDIR%\SoftwareDistribution\DataStore\Logs".to_string(),
                    patterns: vec!["*.log".to_string()],
                    recursive: false,
                },
            ],
        },
        CleanRule {
            id: 1018,
            name: "Segnalazione Errori Windows (WER)".to_string(),
            group: RuleGroup::WindowsSystem,
            description: "Archivi e code di Windows Error Reporting (%ProgramData%\\Microsoft\\Windows\\WER) [winsys.ini ID=1018]".to_string(),
            enabled: true,
            requires_admin: false,
            special_recycle_bin: false,
            targets: vec![
                FileTarget {
                    base_dir: r"%ALLUSERSPROFILE%\Microsoft\Windows\WER\ReportArchive".to_string(),
                    patterns: vec!["*.*".to_string()],
                    recursive: true,
                },
                FileTarget {
                    base_dir: r"%ALLUSERSPROFILE%\Microsoft\Windows\WER\ReportQueue".to_string(),
                    patterns: vec!["*.*".to_string()],
                    recursive: true,
                },
                FileTarget {
                    base_dir: r"%LocalAppData%\Microsoft\Windows\WER\ReportArchive".to_string(),
                    patterns: vec!["*.*".to_string()],
                    recursive: true,
                },
                FileTarget {
                    base_dir: r"%LocalAppData%\Microsoft\Windows\WER\ReportQueue".to_string(),
                    patterns: vec!["*.*".to_string()],
                    recursive: true,
                },
            ],
        },
        CleanRule {
            id: 2509,
            name: "Windows Web & Cryptnet URL Cache".to_string(),
            group: RuleGroup::WindowsSystem,
            description: "Cache certificati e URL Cryptnet (%LocalLowAppData%\\Microsoft\\CryptnetUrlCache) [winsys.ini ID=2509]".to_string(),
            enabled: true,
            requires_admin: false,
            special_recycle_bin: false,
            targets: vec![FileTarget {
                base_dir: r"%LocalLowAppData%\Microsoft\CryptnetUrlCache".to_string(),
                patterns: vec!["*.*".to_string()],
                recursive: true,
            }],
        },
        // === ESPLORA FILE (WINDOWS EXPLORER, LangSecRef=3002) ===
        CleanRule {
            id: 1029,
            name: "Cache Anteprime (Thumbnail Cache)".to_string(),
            group: RuleGroup::WindowsExplorer,
            description: "Database miniature di Esplora File (thumbcache_*.db) [winsys.ini ID=1029]".to_string(),
            enabled: true,
            requires_admin: false,
            special_recycle_bin: false,
            targets: vec![
                FileTarget {
                    base_dir: r"%LocalAppData%\Microsoft\Windows\Explorer".to_string(),
                    patterns: vec!["thumbcache_*.db".to_string()],
                    recursive: false,
                },
                FileTarget {
                    base_dir: r"%LocalAppData%\Microsoft\Windows\Explorer\ThumbCacheToDelete".to_string(),
                    patterns: vec!["thm*.tmp".to_string()],
                    recursive: false,
                },
            ],
        },
        CleanRule {
            id: 1008,
            name: "Documenti Recenti (Recent Documents)".to_string(),
            group: RuleGroup::WindowsExplorer,
            description: "Collegamenti ai file aperti di recente in Esplora Risorse (%APPDATA%\\Microsoft\\Windows\\Recent)".to_string(),
            enabled: false,
            requires_admin: false,
            special_recycle_bin: false,
            targets: vec![FileTarget {
                base_dir: r"%APPDATA%\Microsoft\Windows\Recent".to_string(),
                patterns: vec!["*.lnk".to_string()],
                recursive: false,
            }],
        },
        // === BROWSER & PRIVACY (Chrome, Edge Chromium, Firefox, Brave) ===
        CleanRule {
            id: 2001,
            name: "Google Chrome — Cache, Code Cache & GPUCache".to_string(),
            group: RuleGroup::BrowsersPrivacy,
            description: "Cache HTTP, Code Cache JS/WASM, GPUCache e metriche .pma di Google Chrome [N_CHROME_CACHE]".to_string(),
            enabled: true,
            requires_admin: false,
            special_recycle_bin: false,
            targets: vec![
                FileTarget {
                    base_dir: r"%LocalAppData%\Google\Chrome\User Data\Default\Cache\Cache_Data".to_string(),
                    patterns: vec!["*.*".to_string()],
                    recursive: true,
                },
                FileTarget {
                    base_dir: r"%LocalAppData%\Google\Chrome\User Data\Default\Code Cache".to_string(),
                    patterns: vec!["*.*".to_string()],
                    recursive: true,
                },
                FileTarget {
                    base_dir: r"%LocalAppData%\Google\Chrome\User Data\Default\GPUCache".to_string(),
                    patterns: vec!["*.*".to_string()],
                    recursive: true,
                },
                FileTarget {
                    base_dir: r"%LocalAppData%\Google\Chrome\User Data\Default\Service Worker\CacheStorage".to_string(),
                    patterns: vec!["*.*".to_string()],
                    recursive: true,
                },
                FileTarget {
                    base_dir: r"%LocalAppData%\Google\Chrome\User Data".to_string(),
                    patterns: vec!["*.pma".to_string()],
                    recursive: false,
                },
            ],
        },
        CleanRule {
            id: 2002,
            name: "Google Chrome — Cookie e Cronologia (Privacy)".to_string(),
            group: RuleGroup::BrowsersPrivacy,
            description: "Database SQLite Cookies, History e Visited Links di Google Chrome [N_CHROME_COOKIES / N_CHROME_HISTORY]".to_string(),
            enabled: false,
            requires_admin: false,
            special_recycle_bin: false,
            targets: vec![
                FileTarget {
                    base_dir: r"%LocalAppData%\Google\Chrome\User Data\Default\Network".to_string(),
                    patterns: vec!["Cookies".to_string(), "Cookies-journal".to_string()],
                    recursive: false,
                },
                FileTarget {
                    base_dir: r"%LocalAppData%\Google\Chrome\User Data\Default".to_string(),
                    patterns: vec![
                        "History".to_string(),
                        "History-journal".to_string(),
                        "Visited Links".to_string(),
                    ],
                    recursive: false,
                },
            ],
        },
        CleanRule {
            id: 1057,
            name: "Microsoft Edge Chromium — Cache, Code Cache & Metriche".to_string(),
            group: RuleGroup::BrowsersPrivacy,
            description: "Cache HTTP, Code Cache, GPUCache e file .pma di Microsoft Edge [winsys.ini ID=1057, 1075]".to_string(),
            enabled: true,
            requires_admin: false,
            special_recycle_bin: false,
            targets: vec![
                FileTarget {
                    base_dir: r"%LocalAppData%\Microsoft\Edge\User Data\Default\Cache\Cache_Data".to_string(),
                    patterns: vec!["*.*".to_string()],
                    recursive: true,
                },
                FileTarget {
                    base_dir: r"%LocalAppData%\Microsoft\Edge\User Data\Default\Code Cache".to_string(),
                    patterns: vec!["*.*".to_string()],
                    recursive: true,
                },
                FileTarget {
                    base_dir: r"%LocalAppData%\Microsoft\Edge\User Data\Default\GPUCache".to_string(),
                    patterns: vec!["*.*".to_string()],
                    recursive: true,
                },
                FileTarget {
                    base_dir: r"%LocalAppData%\Microsoft\Edge\User Data".to_string(),
                    patterns: vec!["*.pma".to_string()],
                    recursive: false,
                },
            ],
        },
        CleanRule {
            id: 1058,
            name: "Microsoft Edge Chromium — Cookie e Cronologia".to_string(),
            group: RuleGroup::BrowsersPrivacy,
            description: "Database SQLite Cookies e History di Microsoft Edge [winsys.ini ID=1058, 1059]".to_string(),
            enabled: false,
            requires_admin: false,
            special_recycle_bin: false,
            targets: vec![
                FileTarget {
                    base_dir: r"%LocalAppData%\Microsoft\Edge\User Data\Default\Network".to_string(),
                    patterns: vec!["Cookies".to_string(), "Cookies-journal".to_string()],
                    recursive: false,
                },
                FileTarget {
                    base_dir: r"%LocalAppData%\Microsoft\Edge\User Data\Default".to_string(),
                    patterns: vec!["History".to_string(), "History-journal".to_string()],
                    recursive: false,
                },
            ],
        },
        CleanRule {
            id: 2101,
            name: "Mozilla Firefox — Cache2 & Crash Reports".to_string(),
            group: RuleGroup::BrowsersPrivacy,
            description: "Cache profili Firefox (%LocalAppData%\\Mozilla\\Firefox\\Profiles) e Crash Reports [winapp.ini LangSecRef=3026]".to_string(),
            enabled: true,
            requires_admin: false,
            special_recycle_bin: false,
            targets: vec![
                FileTarget {
                    base_dir: r"%LocalAppData%\Mozilla\Firefox\Profiles".to_string(),
                    patterns: vec!["*.tmp".to_string(), "*.moz-cache*".to_string()],
                    recursive: true,
                },
                FileTarget {
                    base_dir: r"%APPDATA%\Mozilla\Firefox\Crash Reports".to_string(),
                    patterns: vec!["*.dmp".to_string(), "*.extra".to_string()],
                    recursive: true,
                },
            ],
        },
        CleanRule {
            id: 2201,
            name: "Brave Browser — Cache & Code Cache".to_string(),
            group: RuleGroup::BrowsersPrivacy,
            description: "Cache di navigazione e GPUCache di Brave Browser [N_BRAVE_CACHE @ 0x19e8cc0]".to_string(),
            enabled: true,
            requires_admin: false,
            special_recycle_bin: false,
            targets: vec![
                FileTarget {
                    base_dir: r"%LocalAppData%\BraveSoftware\Brave-Browser\User Data\Default\Cache\Cache_Data".to_string(),
                    patterns: vec!["*.*".to_string()],
                    recursive: true,
                },
                FileTarget {
                    base_dir: r"%LocalAppData%\BraveSoftware\Brave-Browser\User Data\Default\Code Cache".to_string(),
                    patterns: vec!["*.*".to_string()],
                    recursive: true,
                },
            ],
        },
        // === APPLICAZIONI & STRUMENTI (`winapp.ini` LangSecRef=3021/3024) ===
        CleanRule {
            id: 3101,
            name: "Windows Widgets WebExperience Cache".to_string(),
            group: RuleGroup::Applications,
            description: "Cache EBWebView dei Widget di Windows 11 [winsys.ini ID=3042]".to_string(),
            enabled: true,
            requires_admin: false,
            special_recycle_bin: false,
            targets: vec![
                FileTarget {
                    base_dir: r"%LocalAppData%\Packages\MicrosoftWindows.Client.WebExperience_cw5n1h2txyewy\LocalState\EBWebView\Default\Code Cache".to_string(),
                    patterns: vec!["*.*".to_string()],
                    recursive: true,
                },
                FileTarget {
                    base_dir: r"%LocalAppData%\Packages\MicrosoftWindows.Client.WebExperience_cw5n1h2txyewy\LocalState\EBWebView\Default\Service Worker\CacheStorage".to_string(),
                    patterns: vec!["*.*".to_string()],
                    recursive: true,
                },
            ],
        },
        CleanRule {
            id: 3102,
            name: "VS Code & Discord — Cache e Log".to_string(),
            group: RuleGroup::Applications,
            description: "Cache temporanea e log di Visual Studio Code e Discord (%APPDATA%\\Code\\Cache, %APPDATA%\\discord\\Cache)".to_string(),
            enabled: true,
            requires_admin: false,
            special_recycle_bin: false,
            targets: vec![
                FileTarget {
                    base_dir: r"%APPDATA%\Code\Cache\Cache_Data".to_string(),
                    patterns: vec!["*.*".to_string()],
                    recursive: true,
                },
                FileTarget {
                    base_dir: r"%APPDATA%\Code\logs".to_string(),
                    patterns: vec!["*.log".to_string()],
                    recursive: true,
                },
                FileTarget {
                    base_dir: r"%APPDATA%\discord\Cache\Cache_Data".to_string(),
                    patterns: vec!["*.*".to_string()],
                    recursive: true,
                },
            ],
        },
    ]
}

/// Scans a single `CleanRule` using `WalkDir` and `rayon` without modifying the disk.
/// Gracefully handles missing directories, permission errors, and locked files without panicking.
pub fn scan_rule(rule: &CleanRule) -> RuleScanResult {
    let mut files_collected: Vec<ScannedFileItem> = Vec::new();

    for target in &rule.targets {
        let base = expand_env_vars(&target.base_dir);
        if is_protected_system_path(&base) || !base.exists() {
            continue;
        }

        let max_depth = if target.recursive { 16 } else { 1 };
        let walker = WalkDir::new(&base)
            .max_depth(max_depth)
            .follow_links(false)
            .into_iter()
            .filter_map(|e| e.ok());

        for entry in walker {
            let path = entry.path();
            if !path.is_file() || is_protected_system_path(path) {
                continue;
            }
            let Some(fname) = path.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            if !target.patterns.iter().any(|p| matches_pattern(fname, p)) {
                continue;
            }
            let size_bytes = entry.metadata().map(|m| m.len()).unwrap_or(0);
            let (locked, err_reason) = probe_file_lock(path);
            files_collected.push(ScannedFileItem {
                path: path.to_path_buf(),
                size_bytes,
                locked_or_denied: locked,
                error_reason: err_reason,
            });
        }
    }

    let total_bytes: u64 = files_collected.iter().map(|f| f.size_bytes).sum();
    let cleanable_bytes: u64 = files_collected
        .iter()
        .filter(|f| !f.locked_or_denied)
        .map(|f| f.size_bytes)
        .sum();
    let locked_count = files_collected.iter().filter(|f| f.locked_or_denied).count();

    RuleScanResult {
        rule_id: rule.id,
        rule_name: rule.name.clone(),
        group: rule.group,
        files: files_collected,
        total_bytes,
        cleanable_bytes,
        locked_count,
    }
}

/// Parallel scan of all enabled rules using `rayon`.
pub fn analyze_all(rules: &[CleanRule]) -> CleanSummary {
    let enabled_rules: Vec<&CleanRule> = rules.iter().filter(|r| r.enabled).collect();

    let results: Vec<RuleScanResult> = enabled_rules
        .par_iter()
        .map(|rule| scan_rule(rule))
        .collect();

    let total_files_found = results.iter().map(|r| r.files.len()).sum();
    let total_bytes_found = results.iter().map(|r| r.total_bytes).sum();
    let total_skipped_locked = results.iter().map(|r| r.locked_count).sum();

    CleanSummary {
        results,
        total_files_found,
        total_bytes_found,
        total_files_deleted: 0,
        total_bytes_deleted: 0,
        total_skipped_locked,
        errors: Vec::new(),
    }
}

/// Executes the cleaning operation on all enabled rules.
/// Uses `SHEmptyRecycleBinW` when the Recycle Bin rule is enabled, and deletes unlocked files in parallel.
/// Locked, in-use, or protected files are safely skipped without panicking.
pub fn execute_clean(rules: &[CleanRule]) -> CleanSummary {
    let mut summary = analyze_all(rules);
    let mut deleted_files = 0usize;
    let mut deleted_bytes = 0u64;
    let mut skipped_locked = 0usize;
    let mut errors = Vec::new();

    for rule in rules.iter().filter(|r| r.enabled && r.special_recycle_bin) {
        unsafe {
            let hr = SHEmptyRecycleBinW(
                std::ptr::null_mut(),
                std::ptr::null(),
                SHERB_NOCONFIRMATION | SHERB_NOPROGRESSUI | SHERB_NOSOUND,
            );
            let hr_u32 = hr as u32;
            // 0 = S_OK
            // 0x80004005 = E_FAIL (already empty on Win10/11)
            // 0x8000FFFF = E_UNEXPECTED (already empty or no items)
            // 0x80070002 = ERROR_FILE_NOT_FOUND (no items in recycle bin)
            // 0x80070015 = ERROR_NOT_READY (unmounted or removable drive empty)
            if hr != 0
                && hr_u32 != 0x80004005
                && hr_u32 != 0x8000FFFF
                && hr_u32 != 0x80070002
                && hr_u32 != 0x80070015
            {
                errors.push(format!(
                    "Regola '{}' (SHEmptyRecycleBinW): codice HRESULT 0x{:08X}",
                    rule.name, hr_u32
                ));
            }
        }
    }

    for res in &summary.results {
        for item in &res.files {
            if item.locked_or_denied || is_protected_system_path(&item.path) {
                skipped_locked += 1;
                continue;
            }
            // Ensure read-only flag is cleared before removal so Windows doesn't reject with PermissionDenied
            if let Ok(meta) = item.path.metadata() {
                let mut perms = meta.permissions();
                if perms.readonly() {
                    #[allow(clippy::permissions_set_readonly_false)]
                    perms.set_readonly(false);
                    let _ = fs::set_permissions(&item.path, perms);
                }
            }

            match fs::remove_file(&item.path) {
                Ok(()) => {
                    deleted_files += 1;
                    deleted_bytes += item.size_bytes;
                }
                Err(e) => {
                    skipped_locked += 1;
                    errors.push(format!(
                        "Saltato file bloccato '{}': {}",
                        item.path.display(),
                        e
                    ));
                }
            }
        }
    }

    // Prune leftover empty subdirectories inside recursive rule target directories
    for rule in rules.iter().filter(|r| r.enabled && !r.special_recycle_bin) {
        for target in &rule.targets {
            if target.recursive {
                let base = expand_env_vars(&target.base_dir);
                if !is_protected_system_path(&base) {
                    prune_empty_subdirs(&base);
                }
            }
        }
    }

    summary.total_files_deleted = deleted_files;
    summary.total_bytes_deleted = deleted_bytes;
    summary.total_skipped_locked = skipped_locked;
    summary.errors = errors;
    summary
}

/// Safely prunes empty subdirectories inside `base_dir` bottom-up.
/// Only removes child subdirectories that are completely empty. Never removes `base_dir` itself.
/// Explicitly rejects and never touches protected system paths.
pub fn prune_empty_subdirs(base_dir: &Path) {
    if is_protected_system_path(base_dir) || !base_dir.is_dir() {
        return;
    }
    let mut dirs: Vec<PathBuf> = WalkDir::new(base_dir)
        .min_depth(1)
        .max_depth(16)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_dir())
        .map(|e| e.path().to_path_buf())
        .filter(|p| !is_protected_system_path(p))
        .collect();

    // Sort descending by component depth so deepest subdirectories are deleted first
    dirs.sort_by_key(|a| std::cmp::Reverse(a.components().count()));

    for dir in dirs {
        if !is_protected_system_path(&dir) {
            // fs::remove_dir succeeds strictly on empty directories; fails safely if files remain
            let _ = fs::remove_dir(dir);
        }
    }
}

/// Formats a byte count into human-readable units (B, KB, MB, GB).
pub fn format_bytes(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    const GB: f64 = MB * 1024.0;
    let b = bytes as f64;
    if b >= GB {
        format!("{:.2} GB", b / GB)
    } else if b >= MB {
        format!("{:.2} MB", b / MB)
    } else if b >= KB {
        format!("{:.1} KB", b / KB)
    } else {
        format!("{bytes} B")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_bytes() {
        assert_eq!(format_bytes(512), "512 B");
        assert_eq!(format_bytes(2048), "2.0 KB");
        assert_eq!(format_bytes(5 * 1024 * 1024), "5.00 MB");
    }

    #[test]
    fn test_expand_env_vars() {
        std::env::set_var("RUSTCLEANER_TEST_DIR", r"C:\TempTest");
        let expanded = expand_env_vars(r"%RUSTCLEANER_TEST_DIR%\SubFolder");
        assert_eq!(expanded, PathBuf::from(r"C:\TempTest\SubFolder"));
    }

    #[test]
    fn test_matches_pattern() {
        assert!(matches_pattern("setup.log", "*.log"));
        assert!(matches_pattern("MEMORY.DMP", "*.dmp"));
        assert!(matches_pattern("anyfile.txt", "*.*"));
        assert!(!matches_pattern("keep.exe", "*.tmp"));
    }

    #[test]
    fn test_default_rules_non_empty() {
        let rules = default_clean_rules();
        assert!(rules.len() >= 15);
    }

    #[test]
    fn test_prune_empty_subdirs() {
        let base = std::env::temp_dir().join("rustcleaner_test_prune");
        let empty_sub = base.join("empty1").join("empty2");
        let non_empty_sub = base.join("non_empty");
        let _ = fs::create_dir_all(&empty_sub);
        let _ = fs::create_dir_all(&non_empty_sub);
        let keep_file = non_empty_sub.join("keep.txt");
        let _ = fs::write(&keep_file, "keep");

        prune_empty_subdirs(&base);

        assert!(base.exists(), "base dir should not be deleted");
        assert!(!empty_sub.exists(), "empty nested subdir should be pruned");
        assert!(keep_file.exists(), "file in non-empty subdir should remain");

        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn test_is_protected_system_path_system_directories() {
        assert!(is_protected_system_path(Path::new(r"C:\Windows\System32")));
        assert!(is_protected_system_path(Path::new(r"C:\Windows\System32\cmd.exe")));
        assert!(is_protected_system_path(Path::new(r"C:\Windows\SysWOW64\kernel32.dll")));
        assert!(is_protected_system_path(Path::new(r"C:\Windows\WinSxS\manifest.xml")));
        assert!(is_protected_system_path(Path::new(r"C:\Windows\Boot\DVD\boot.sdi")));
        assert!(is_protected_system_path(Path::new(r"C:\Program Files\Windows NT\Accessories")));
        assert!(is_protected_system_path(Path::new(r"\\.\PhysicalDrive0")));
        assert!(is_protected_system_path(Path::new(r"C:\")));
    }

    #[test]
    fn test_is_protected_system_path_personal_directories() {
        assert!(is_protected_system_path(Path::new(r"C:\Users\Alice\Documents\report.docx")));
        assert!(is_protected_system_path(Path::new(r"C:\Users\Alice\Desktop\shortcut.lnk")));
        assert!(is_protected_system_path(Path::new(r"C:\Users\Alice\Pictures\photo.jpg")));
        assert!(is_protected_system_path(Path::new(r"C:\Users\Alice\Music\song.mp3")));
        assert!(is_protected_system_path(Path::new(r"C:\Users\Alice\Videos\movie.mp4")));
        assert!(is_protected_system_path(Path::new(r"C:\Users\Alice\OneDrive\Documents\sheet.xlsx")));
    }

    #[test]
    fn test_is_protected_system_path_repos_and_boot() {
        assert!(is_protected_system_path(Path::new(r"C:\bootmgr")));
        assert!(is_protected_system_path(Path::new(r"C:\Windows\bootmgr")));
        assert!(is_protected_system_path(Path::new(r"D:\MyProject\.git\config")));
        // Current repo itself
        assert!(is_protected_system_path(Path::new(r"d:\Repo\RustCleaner\src\main.rs")));
    }

    #[test]
    fn test_is_protected_system_path_allows_temp() {
        assert!(!is_protected_system_path(Path::new(r"C:\Windows\Temp\test.tmp")));
        assert!(!is_protected_system_path(Path::new(r"C:\Windows\Logs\cbs.log")));
        assert!(!is_protected_system_path(Path::new(r"C:\Users\Alice\AppData\Local\Temp\cache.tmp")));
    }

    #[test]
    fn test_probe_file_lock_read_only_preserves_attributes() {
        let test_file = std::env::temp_dir().join("rustcleaner_test_probe_readonly.tmp");
        let _ = fs::write(&test_file, "readonly content");
        if let Ok(meta) = test_file.metadata() {
            let mut perms = meta.permissions();
            perms.set_readonly(true);
            let _ = fs::set_permissions(&test_file, perms);
        }

        let (locked, reason) = probe_file_lock(&test_file);
        assert!(!locked, "unlocked readonly file should probe as not locked");
        assert!(reason.is_none());

        // Verify the file's readonly attribute was NOT cleared during probe
        let meta_after = test_file.metadata().expect("file should exist");
        assert!(meta_after.permissions().readonly(), "probe_file_lock must preserve readonly attribute");

        // Clean up
        let mut perms = meta_after.permissions();
        #[allow(clippy::permissions_set_readonly_false)]
        perms.set_readonly(false);
        let _ = fs::set_permissions(&test_file, perms);
        let _ = fs::remove_file(&test_file);
    }
}

