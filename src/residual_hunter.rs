//! Deep Residual Hunter (Leftover Hunter) Engine
//! Specialized engine for detecting and nuking orphaned files, folders, and registry keys
//! left behind by standard Windows uninstallers, or orphaned across the system from past uninstalls.
//!
//! Replaces commercial uninstaller utilities (Revo Uninstaller, IObit Uninstaller, BCUninstaller)
//! with a 100% native Rust, zero-dependency, multi-threaded residual scanner.

use chrono::Local;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;
use winreg::enums::*;
use winreg::RegKey;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ConfidenceLevel {
    /// Safe to remove: exact match on app/vendor name in standard application folders or registry hives.
    High,
    /// Moderately safe: fuzzy match or vendor directory containing only this product.
    Medium,
    /// Needs review: shared vendor directory or heuristic match.
    Low,
}

impl ConfidenceLevel {
    pub fn label(&self) -> &'static str {
        match self {
            ConfidenceLevel::High => "Alta Sicurezza",
            ConfidenceLevel::Medium => "Media Sicurezza",
            ConfidenceLevel::Low => "Da Verificare",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ResidualHive {
    Hkcu,
    Hklm,
    HklmWow64,
}

impl ResidualHive {
    pub fn label(&self) -> &'static str {
        match self {
            ResidualHive::Hkcu => "HKEY_CURRENT_USER",
            ResidualHive::Hklm => "HKEY_LOCAL_MACHINE",
            ResidualHive::HklmWow64 => "HKEY_LOCAL_MACHINE (32-bit)",
        }
    }

    pub fn open_root(&self) -> RegKey {
        match self {
            ResidualHive::Hkcu => RegKey::predef(HKEY_CURRENT_USER),
            ResidualHive::Hklm | ResidualHive::HklmWow64 => RegKey::predef(HKEY_LOCAL_MACHINE),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResidualKind {
    Directory(PathBuf),
    File(PathBuf),
    RegistryKey {
        hive: ResidualHive,
        sub_path: String,
    },
    RegistryValue {
        hive: ResidualHive,
        key_path: String,
        value_name: String,
    },
}

#[derive(Debug, Clone)]
pub struct ResidualItem {
    pub id: usize,
    pub kind: ResidualKind,
    pub display_path: String,
    pub size_bytes: u64,
    pub confidence: ConfidenceLevel,
    pub reason: String,
    pub selected: bool,
}

#[derive(Debug, Clone, Default)]
pub struct ResidualScanSummary {
    pub target_app_name: String,
    pub target_publisher: String,
    pub items: Vec<ResidualItem>,
    pub total_bytes: u64,
    pub total_dirs: usize,
    pub total_files: usize,
    pub total_registry_keys: usize,
}

#[derive(Debug, Clone, Default)]
pub struct ResidualDeletionReport {
    pub deleted_items: usize,
    pub freed_bytes: u64,
    pub deleted_reg_keys: usize,
    pub reg_backup_path: Option<PathBuf>,
    pub errors: Vec<String>,
}

/// Generic words to exclude during name tokenization to avoid false positives
const GENERIC_EXCLUSION_WORDS: &[&str] = &[
    "the", "a", "an", "and", "or", "for", "with", "by", "of", "in", "to", "on",
    "app", "apps", "application", "applications", "software", "suite", "tool", "tools",
    "utility", "utilities", "program", "programs", "client", "service", "services",
    "edition", "version", "desktop", "portable", "installer", "setup", "uninstaller",
    "free", "pro", "plus", "premium", "ultimate", "community", "enterprise", "standard",
    "windows", "microsoft", "corporation", "corp", "inc", "incorporated", "ltd", "limited",
    "llc", "gmbh", "technologies", "technology", "systems", "system", "group", "holdings",
    "x86", "x64", "64-bit", "32-bit", "win64", "win32", "update", "updater", "runtime",
    "redistributable", "package", "driver", "drivers", "framework", "component", "components",
];

/// Critical paths that MUST NEVER be matched or deleted under any circumstances
const CRITICAL_SYSTEM_PATH_BLACKLIST: &[&str] = &[
    r"c:\windows",
    r"c:\windows\system32",
    r"c:\windows\syswow64",
    r"c:\windows\winsxs",
    r"c:\program files",
    r"c:\program files (x86)",
    r"c:\users",
    r"c:\programdata",
    r"c:\programdata\microsoft",
    r"c:\program files\common files",
    r"c:\program files (x86)\common files",
];

/// Extracts search tokens from an application's display name and publisher.
pub fn extract_search_tokens(name: &str, publisher: &str) -> Vec<String> {
    let mut tokens = Vec::new();

    let clean_str = |s: &str| -> Vec<String> {
        s.to_lowercase()
            .replace(['(', ')', '[', ']', '{', '}', '-', '_', '.', ',', ':', ';', '/', '\\', '\'', '"'], " ")
            .split_whitespace()
            .map(|w| w.trim().to_string())
            .filter(|w| w.len() >= 3 && !GENERIC_EXCLUSION_WORDS.contains(&w.as_str()))
            .collect()
    };

    let name_tokens = clean_str(name);
    let pub_tokens = clean_str(publisher);

    for t in name_tokens {
        if !tokens.contains(&t) {
            tokens.push(t);
        }
    }
    for t in pub_tokens {
        if !tokens.contains(&t) {
            tokens.push(t);
        }
    }

    // Also include the full compacted alphanumeric name if >= 4 chars
    let compact_name: String = name.chars().filter(|c| c.is_alphanumeric()).collect::<String>().to_lowercase();
    if compact_name.len() >= 4 && !tokens.contains(&compact_name) && !GENERIC_EXCLUSION_WORDS.contains(&compact_name.as_str()) {
        tokens.push(compact_name);
    }

    tokens
}

/// Checks if a filesystem path is a protected system directory that must not be deleted.
pub fn is_protected_path(path: &Path) -> bool {
    let resolved = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    let path_str = resolved.to_string_lossy().replace('/', "\\").to_lowercase();
    let norm = path_str.trim_end_matches(['\\', '/']);

    for blacklisted in CRITICAL_SYSTEM_PATH_BLACKLIST {
        if norm == *blacklisted {
            return true;
        }
    }

    let windir = env::var("WINDIR").or_else(|_| env::var("SystemRoot")).unwrap_or_else(|_| r"C:\Windows".to_string()).replace('/', "\\").to_lowercase();
    let pf = env::var("ProgramFiles").unwrap_or_else(|_| r"C:\Program Files".to_string()).replace('/', "\\").to_lowercase();
    let pf86 = env::var("ProgramFiles(x86)").unwrap_or_else(|_| r"C:\Program Files (x86)".to_string()).replace('/', "\\").to_lowercase();
    let protected_subtrees = [windir.clone(), format!(r"{windir}\system32"), format!(r"{windir}\syswow64"), format!(r"{windir}\winsxs"), format!(r"{windir}\boot"), format!(r"{pf}\windows nt"), format!(r"{pf86}\windows nt"), r"c:\programdata\microsoft".to_string()];
    if protected_subtrees.iter().any(|root| norm == root || norm.starts_with(&format!(r"{root}\"))) { return true; }

    // Root drive protections (e.g. "C:\")
    if norm.len() <= 3 && norm.ends_with(':') || norm.len() == 3 && norm.chars().nth(1) == Some(':') {
        return true;
    }

    // Protect user root folder (e.g. "C:\Users\username")
    if let Ok(user_profile) = env::var("USERPROFILE") {
        let up_norm = user_profile.to_lowercase();
        if norm == up_norm {
            return true;
        }
        // Protect AppData roots themselves
        if norm == format!(r"{up_norm}\appdata")
            || norm == format!(r"{up_norm}\appdata\local")
            || norm == format!(r"{up_norm}\appdata\roaming")
            || norm == format!(r"{up_norm}\appdata\locallow")
            || norm == format!(r"{up_norm}\desktop")
            || norm == format!(r"{up_norm}\documents")
            || norm == format!(r"{up_norm}\downloads")
        {
            return true;
        }
    }

    let personal = ["documents", "desktop", "pictures", "music", "videos"];
    if resolved.components().any(|component| personal.contains(&component.as_os_str().to_string_lossy().to_lowercase().as_str())) { return true; }
    let existing = if resolved.exists() { resolved.as_path() } else { resolved.parent().unwrap_or(resolved.as_path()) };
    if existing.ancestors().any(|ancestor| ancestor.join(".git").exists()) { return true; }
    if resolved.is_dir() && WalkDir::new(&resolved).max_depth(4).into_iter().filter_map(Result::ok).any(|entry| entry.file_name() == ".git") { return true; }
    false
}

/// Calculates the recursive size of a directory on disk.
pub fn compute_directory_size(path: &Path) -> u64 {
    let mut total: u64 = 0;
    for entry in WalkDir::new(path).into_iter().flatten() {
        if let Ok(meta) = entry.metadata() {
            if meta.is_file() {
                total = total.saturating_add(meta.len());
            }
        }
    }
    total
}

/// Scans standard Windows software roots for directories and files matching application tokens.
pub fn scan_filesystem_residuals(
    tokens: &[String],
    app_name: &str,
    install_location: Option<&Path>,
) -> Vec<ResidualItem> {
    let mut items = Vec::new();
    let mut item_id = 1usize;

    // Collect base search directories
    let mut search_bases: Vec<(PathBuf, &'static str)> = Vec::new();

    if let Ok(p) = env::var("LOCALAPPDATA") {
        search_bases.push((PathBuf::from(p), "AppData Local"));
    }
    if let Ok(p) = env::var("APPDATA") {
        search_bases.push((PathBuf::from(p), "AppData Roaming"));
    }
    if let Ok(p) = env::var("ProgramData") {
        search_bases.push((PathBuf::from(p), "ProgramData"));
    }
    if let Ok(p) = env::var("ProgramFiles") {
        search_bases.push((PathBuf::from(p), "Program Files"));
    }
    if let Ok(p) = env::var("ProgramFiles(x86)") {
        search_bases.push((PathBuf::from(p), "Program Files (x86)"));
    }
    if let Ok(p) = env::var("USERPROFILE") {
        search_bases.push((PathBuf::from(&p).join(".config"), "User .config"));
    }

    // Also include specific install location if known and still existing
    if let Some(inst_loc) = install_location {
        if inst_loc.exists() && !is_protected_path(inst_loc) {
            let size = compute_directory_size(inst_loc);
            items.push(ResidualItem {
                id: item_id,
                kind: ResidualKind::Directory(inst_loc.to_path_buf()),
                display_path: inst_loc.display().to_string(),
                size_bytes: size,
                confidence: ConfidenceLevel::High,
                reason: format!("Cartella di installazione originale di '{app_name}'"),
                selected: true,
            });
            item_id += 1;
        }
    }

    let mut seen_paths = std::collections::HashSet::new();
    for (base_dir, source_label) in search_bases {
        if !base_dir.exists() || !base_dir.is_dir() {
            continue;
        }

        let Ok(read_dir) = fs::read_dir(&base_dir) else {
            continue;
        };

        for entry in read_dir.flatten() {
            let path = entry.path();
            if is_protected_path(&path) {
                continue;
            }

            let file_name = entry.file_name().to_string_lossy().to_string();
            let lower_name = file_name.to_lowercase();

            // Check if directory or file matches any tokens
            let mut matched_token: Option<&String> = None;
            for token in tokens {
                if lower_name == *token || lower_name.contains(token) {
                    matched_token = Some(token);
                    break;
                }
            }

            let Some(token) = matched_token else {
                continue;
            };

            let canon = path.canonicalize().unwrap_or_else(|_| path.clone());
            if !seen_paths.insert(canon) {
                continue;
            }

            let is_exact = lower_name == *token || lower_name == app_name.to_lowercase();
            let confidence = if is_exact {
                ConfidenceLevel::High
            } else {
                ConfidenceLevel::Medium
            };

            let size = if path.is_dir() {
                compute_directory_size(&path)
            } else {
                entry.metadata().map(|m| m.len()).unwrap_or(0)
            };

            let kind = if path.is_dir() {
                ResidualKind::Directory(path.clone())
            } else {
                ResidualKind::File(path.clone())
            };

            items.push(ResidualItem {
                id: item_id,
                kind,
                display_path: path.display().to_string(),
                size_bytes: size,
                confidence,
                reason: format!("Cartella residua in {source_label} corrispondente a '{token}'"),
                selected: confidence >= ConfidenceLevel::Medium,
            });
            item_id += 1;
        }
    }

    items
}

/// Scans the Windows Registry for residual keys in `HKCU\Software`, `HKLM\Software`, and `Uninstall`.
pub fn scan_registry_residuals(
    tokens: &[String],
    _app_name: &str,
    reg_key_name: &str,
    start_id: usize,
) -> Vec<ResidualItem> {
    let mut items = Vec::new();
    let mut item_id = start_id;

    let scan_hives = [
        (ResidualHive::Hkcu, "Software"),
        (ResidualHive::Hklm, r"SOFTWARE"),
        (ResidualHive::HklmWow64, r"SOFTWARE\WOW6432Node"),
    ];

    let mut seen_keys = std::collections::HashSet::new();

    // Check if the original Uninstall subkey is still registered
    let uninstall_paths = [
        (ResidualHive::Hklm, r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall"),
        (ResidualHive::HklmWow64, r"SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall"),
        (ResidualHive::Hkcu, r"Software\Microsoft\Windows\CurrentVersion\Uninstall"),
    ];

    if !reg_key_name.is_empty() {
        for (hive, base_path) in uninstall_paths {
            let full_subpath = format!(r"{base_path}\{reg_key_name}");
            let root = hive.open_root();
            if root.open_subkey_with_flags(&full_subpath, KEY_READ).is_ok() {
                items.push(ResidualItem {
                    id: item_id,
                    kind: ResidualKind::RegistryKey {
                        hive,
                        sub_path: full_subpath.clone(),
                    },
                    display_path: format!(r"{}\{}", hive.label(), full_subpath),
                    size_bytes: 0,
                    confidence: ConfidenceLevel::High,
                    reason: format!("Chiave di disinstallazione orfana in '{base_path}'"),
                    selected: true,
                });
                item_id += 1;
                seen_keys.insert((hive, full_subpath.to_lowercase()));
            }
        }
    }

    // Inspect Software trees in HKCU and HKLM
    for (hive, base_subpath) in scan_hives {
        let root = hive.open_root();
        let Ok(software_key) = root.open_subkey_with_flags(base_subpath, KEY_READ) else {
            continue;
        };

        for vendor_key_name in software_key.enum_keys().flatten() {
            let lower_vendor = vendor_key_name.to_lowercase();

            // Skip protected vendor roots
            if lower_vendor == "microsoft"
                || lower_vendor == "classes"
                || lower_vendor == "policies"
                || lower_vendor == "clients"
                || lower_vendor == "registeredapplications"
            {
                continue;
            }

            // Check if vendor matches token
            let vendor_matches = tokens.iter().any(|t| lower_vendor == *t || lower_vendor.contains(t));

            let vendor_full_path = format!(r"{base_subpath}\{vendor_key_name}");

            if vendor_matches {
                if seen_keys.insert((hive, vendor_full_path.to_lowercase())) {
                    items.push(ResidualItem {
                        id: item_id,
                        kind: ResidualKind::RegistryKey {
                            hive,
                            sub_path: vendor_full_path.clone(),
                        },
                        display_path: format!(r"{}\{}", hive.label(), vendor_full_path),
                        size_bytes: 0,
                        confidence: ConfidenceLevel::High,
                        reason: format!("Albero di configurazione software in '{}'", hive.label()),
                        selected: true,
                    });
                    item_id += 1;
                }
            } else {
                // If vendor is not matching, check one level deeper (e.g. `Software\Vendor\<AppName>`)
                if let Ok(vendor_key) = software_key.open_subkey_with_flags(&vendor_key_name, KEY_READ) {
                    for product_key_name in vendor_key.enum_keys().flatten() {
                        let lower_prod = product_key_name.to_lowercase();
                        let prod_matches = tokens.iter().any(|t| lower_prod == *t || lower_prod.contains(t));

                        if prod_matches {
                            let prod_full_path = format!(r"{vendor_full_path}\{product_key_name}");
                            if seen_keys.insert((hive, prod_full_path.to_lowercase())) {
                                items.push(ResidualItem {
                                    id: item_id,
                                    kind: ResidualKind::RegistryKey {
                                        hive,
                                        sub_path: prod_full_path.clone(),
                                    },
                                    display_path: format!(r"{}\{}", hive.label(), prod_full_path),
                                    size_bytes: 0,
                                    confidence: ConfidenceLevel::High,
                                    reason: format!("Sottochiave prodotto in '{vendor_key_name}'"),
                                    selected: true,
                                });
                                item_id += 1;
                            }
                        }
                    }
                }
            }
        }
    }

    items
}

/// Runs a full Deep Residual scan for a specific application.
pub fn scan_application_residuals(
    app_name: &str,
    publisher: &str,
    install_location: Option<&Path>,
    reg_key_name: &str,
) -> ResidualScanSummary {
    let tokens = extract_search_tokens(app_name, publisher);

    let mut items = scan_filesystem_residuals(&tokens, app_name, install_location);
    let next_id = items.len() + 1;
    let reg_items = scan_registry_residuals(&tokens, app_name, reg_key_name, next_id);
    items.extend(reg_items);

    let mut total_bytes = 0u64;
    let mut total_dirs = 0usize;
    let mut total_files = 0usize;
    let mut total_registry_keys = 0usize;

    for item in &items {
        match &item.kind {
            ResidualKind::Directory(_) => {
                total_dirs += 1;
                total_bytes = total_bytes.saturating_add(item.size_bytes);
            }
            ResidualKind::File(_) => {
                total_files += 1;
                total_bytes = total_bytes.saturating_add(item.size_bytes);
            }
            ResidualKind::RegistryKey { .. } | ResidualKind::RegistryValue { .. } => {
                total_registry_keys += 1;
            }
        }
    }

    ResidualScanSummary {
        target_app_name: app_name.to_string(),
        target_publisher: publisher.to_string(),
        items,
        total_bytes,
        total_dirs,
        total_files,
        total_registry_keys,
    }
}

/// Scans standard AppData and ProgramData folders for orphaned directories of applications
/// that are no longer installed on the system (Global Leftover Hunter).
pub fn scan_global_orphans(installed_app_names: &[String]) -> Vec<ResidualItem> {
    let mut items = Vec::new();
    let mut item_id = 1usize;

    // Normalised list of known installed tokens
    let mut installed_tokens: Vec<String> = Vec::new();
    for name in installed_app_names {
        for t in extract_search_tokens(name, "") {
            if !installed_tokens.contains(&t) {
                installed_tokens.push(t);
            }
        }
    }

    let search_bases = [
        (env::var("LOCALAPPDATA").ok(), "AppData Local"),
        (env::var("APPDATA").ok(), "AppData Roaming"),
        (env::var("ProgramData").ok(), "ProgramData"),
    ];

    // Whitelist of legitimate Windows system directories present in AppData/ProgramData
    let system_subfolder_whitelist = [
        "microsoft", "windows", "packages", "temp", "programs", "edge", "system",
        "nvidia", "amd", "intel", "realtek", "usoshared", "crashdumps",
    ];

    for (base_opt, label) in search_bases {
        let Some(base_str) = base_opt else { continue };
        let base_path = PathBuf::from(base_str);
        if !base_path.exists() { continue };

        let Ok(entries) = fs::read_dir(&base_path) else { continue };
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() || is_protected_path(&path) {
                continue;
            }

            let name = entry.file_name().to_string_lossy().to_string();
            let lower = name.to_lowercase();

            // Skip known system folders
            if system_subfolder_whitelist.iter().any(|&s| lower == s || lower.starts_with(s)) {
                continue;
            }

            // Check if this folder matches ANY currently installed app
            let matches_installed = installed_tokens.iter().any(|t| lower == *t || lower.contains(t));

            if !matches_installed {
                let size = compute_directory_size(&path);
                // Only flag if folder actually has files (> 4 KB)
                if size > 4096 {
                    items.push(ResidualItem {
                        id: item_id,
                        kind: ResidualKind::Directory(path.clone()),
                        display_path: path.display().to_string(),
                        size_bytes: size,
                        confidence: ConfidenceLevel::Medium,
                        reason: format!("Cartella orfana in {label} (nessuna applicazione installata corrispondente)"),
                        selected: false, // In global scan, let user review before selecting
                    });
                    item_id += 1;
                }
            }
        }
    }

    items
}

/// Automatically exports a timestamped `.reg` backup before deleting any registry keys.
pub fn export_residual_reg_backup(
    items: &[ResidualItem],
    backup_dir: &Path,
) -> Result<PathBuf, String> {
    fs::create_dir_all(backup_dir)
        .map_err(|e| format!("Impossibile creare cartella backup '{}': {e}", backup_dir.display()))?;

    let timestamp = Local::now().format("%Y%m%d_%H%M%S");
    let file_path = backup_dir.join(format!("residual_backup_{timestamp}.reg"));

    let mut content = String::from("Windows Registry Editor Version 5.00\r\n\r\n");
    content.push_str(&format!(
        "; Backup automatico generato dal Deep Residual Hunter di RustCleaner\r\n; Data: {}\r\n\r\n",
        Local::now().format("%Y-%m-%d %H:%M:%S")
    ));

    let mut has_entries = false;
    for item in items.iter().filter(|i| i.selected) {
        if let ResidualKind::RegistryKey { hive, sub_path } = &item.kind {
            let root = hive.open_root();
            let key = root.open_subkey_with_flags(sub_path, KEY_READ).map_err(|e| format!("Impossibile leggere {}\\{sub_path} per il backup: {e}", hive.label()))?;
            let reg_root = match hive { ResidualHive::Hkcu => "HKEY_CURRENT_USER", _ => "HKEY_LOCAL_MACHINE" };
            append_reg_subtree(&key, &format!("{reg_root}\\{sub_path}"), &mut content)?;
            has_entries = true;
        } else if let ResidualKind::RegistryValue { hive, key_path, value_name } = &item.kind {
            let root = hive.open_root();
            let key = root.open_subkey_with_flags(key_path, KEY_READ).map_err(|e| format!("Impossibile leggere {}\\{key_path} per il backup: {e}", hive.label()))?;
            let value = key.get_raw_value(value_name).map_err(|e| format!("Impossibile leggere il valore '{value_name}' per il backup: {e}"))?;
            let reg_root = match hive { ResidualHive::Hkcu => "HKEY_CURRENT_USER", _ => "HKEY_LOCAL_MACHINE" };
            content.push_str(&format!("[{reg_root}\\{key_path}]\r\n{}\r\n\r\n", format_reg_value(value_name, &value)));
            has_entries = true;
        }
    }

    if !has_entries {
        content.push_str("; Nessuna chiave di registro selezionata per la rimozione.\r\n");
    }

    crate::registry::write_reg_file(&file_path, &content)?;

    Ok(file_path)
}

fn format_reg_value(name: &str, value: &winreg::RegValue) -> String {
    let name = if name.is_empty() { "@".to_string() } else { format!("\"{}\"", name.replace('\\', "\\\\").replace('"', "\\\"")) };
    match value.vtype.clone() {
        REG_SZ | REG_EXPAND_SZ => {
            let kind = if value.vtype == REG_SZ { 1 } else { 2 };
            format!("{name}=hex({kind:x}):{}", value.bytes.iter().map(|b| format!("{b:02x}")).collect::<Vec<_>>().join(","))
        }
        REG_DWORD if value.bytes.len() >= 4 => format!("{name}=dword:{:08x}", u32::from_le_bytes(value.bytes[..4].try_into().unwrap_or([0; 4]))),
        kind => format!("{name}=hex({:x}):{}", kind as u32, value.bytes.iter().map(|b| format!("{b:02x}")).collect::<Vec<_>>().join(",")),
    }
}

fn append_reg_subtree(key: &RegKey, path: &str, content: &mut String) -> Result<(), String> {
    content.push_str(&format!("[{path}]\r\n"));
    for value_result in key.enum_values() {
        let (name, value) = value_result.map_err(|e| format!("Impossibile enumerare valori registry: {e}"))?;
        content.push_str(&format!("{}\r\n", format_reg_value(&name, &value)));
    }
    content.push_str("\r\n");
    for key_result in key.enum_keys() {
        let child = key_result.map_err(|e| format!("Impossibile enumerare sottochiavi registry: {e}"))?;
        let child_key = key.open_subkey_with_flags(&child, KEY_READ).map_err(|e| format!("Impossibile leggere sottochiave registry '{child}': {e}"))?;
        append_reg_subtree(&child_key, &format!("{path}\\{child}"), content)?;
    }
    Ok(())
}

/// Executes the deletion of all selected residual items with automated `.reg` backup.
pub fn delete_selected_residuals(
    items: &[ResidualItem],
    backup_dir: &Path,
) -> ResidualDeletionReport {
    let mut report = ResidualDeletionReport::default();

    // Check if there are any registry items to backup
    let has_reg = items.iter().any(|i| i.selected && matches!(i.kind, ResidualKind::RegistryKey { .. } | ResidualKind::RegistryValue { .. }));
    if has_reg {
        match export_residual_reg_backup(items, backup_dir) {
            Ok(p) => report.reg_backup_path = Some(p),
            Err(e) => { report.errors.push(format!("Eliminazione annullata: backup registro non riuscito: {e}")); return report; }
        }
    }

    for item in items.iter().filter(|i| i.selected) {
        match &item.kind {
            ResidualKind::Directory(path) => {
                if is_protected_path(path) {
                    report.errors.push(format!("Percorso protetto saltato per sicurezza: {}", path.display()));
                    continue;
                }
                match fs::remove_dir_all(path) {
                    Ok(()) => {
                        report.deleted_items += 1;
                        report.freed_bytes = report.freed_bytes.saturating_add(item.size_bytes);
                    }
                    Err(e) => {
                        report.errors.push(format!("Impossibile rimuovere cartella '{}': {e}", path.display()));
                    }
                }
            }
            ResidualKind::File(path) => {
                if is_protected_path(path) {
                    report.errors.push(format!("File protetto saltato per sicurezza: {}", path.display()));
                    continue;
                }
                match fs::remove_file(path) {
                    Ok(()) => {
                        report.deleted_items += 1;
                        report.freed_bytes = report.freed_bytes.saturating_add(item.size_bytes);
                    }
                    Err(e) => {
                        report.errors.push(format!("Impossibile rimuovere file '{}': {e}", path.display()));
                    }
                }
            }
            ResidualKind::RegistryKey { hive, sub_path } => {
                let root = hive.open_root();
                match root.delete_subkey_all(sub_path) {
                    Ok(()) => {
                        report.deleted_items += 1;
                        report.deleted_reg_keys += 1;
                    }
                    Err(e) => {
                        report.errors.push(format!("Impossibile eliminare chiave {}\\{}: {e}", hive.label(), sub_path));
                    }
                }
            }
            ResidualKind::RegistryValue { hive, key_path, value_name } => {
                let root = hive.open_root();
                if let Ok(key) = root.open_subkey_with_flags(key_path, KEY_SET_VALUE) {
                    match key.delete_value(value_name) {
                        Ok(()) => {
                            report.deleted_items += 1;
                            report.deleted_reg_keys += 1;
                        }
                        Err(e) => {
                            report.errors.push(format!("Impossibile eliminare valore {}\\{} -> {}: {e}", hive.label(), key_path, value_name));
                        }
                    }
                } else {
                    report.errors.push(format!("Impossibile aprire {}\\{} per eliminare il valore '{}'.", hive.label(), key_path, value_name));
                }
            }
        }
    }

    report
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_search_tokens() {
        let tokens = extract_search_tokens("Mozilla Firefox (x64 it)", "Mozilla Corporation");
        assert!(tokens.contains(&"firefox".to_string()));
        assert!(tokens.contains(&"mozilla".to_string()));
        assert!(!tokens.contains(&"x64".to_string()));
        assert!(!tokens.contains(&"corporation".to_string()));
    }

    #[test]
    fn test_critical_blacklist_protection() {
        assert!(is_protected_path(Path::new(r"C:\Windows")));
        assert!(is_protected_path(Path::new(r"C:\Windows\System32")));
        assert!(is_protected_path(Path::new(r"C:\Program Files")));
        assert!(is_protected_path(Path::new(r"C:\Program Files (x86)")));
        assert!(is_protected_path(Path::new(r"C:\Users")));
        assert!(is_protected_path(Path::new(r"C:\")));
    }

    #[test]
    fn test_export_residual_reg_backup() {
        let temp_dir = env::temp_dir().join("rustcleaner_residual_test");
        let root = RegKey::predef(HKEY_CURRENT_USER);
        let (key, _) = root.create_subkey(r"Software\TestAppResidual").expect("create temporary registry test key");
        key.set_value("Sample", &"Restorable").expect("create temporary registry test value");
        drop(key);
        let item = ResidualItem {
            id: 1,
            kind: ResidualKind::RegistryKey {
                hive: ResidualHive::Hkcu,
                sub_path: r"Software\TestAppResidual".to_string(),
            },
            display_path: r"HKEY_CURRENT_USER\Software\TestAppResidual".to_string(),
            size_bytes: 0,
            confidence: ConfidenceLevel::High,
            reason: "Chiave orfana test".to_string(),
            selected: true,
        };

        let backup_file = export_residual_reg_backup(&[item], &temp_dir).expect("backup generation");
        assert!(backup_file.exists());
        let bytes = fs::read(&backup_file).expect("read backup");
        let text = String::from_utf16_lossy(&bytes[2..].chunks_exact(2).map(|b| u16::from_le_bytes([b[0], b[1]])).collect::<Vec<_>>());
        assert!(text.contains("Windows Registry Editor Version 5.00"));
        assert!(text.contains("Software\\TestAppResidual"));
        let _ = fs::remove_dir_all(&temp_dir);
        let _ = root.delete_subkey_all(r"Software\TestAppResidual");
    }
}
