//! Windows Registry Cleaner & Mandatory `.reg` Backup Engine
//! Reconstructed from CCleaner64.exe embedded `INI/271/1033` (`winreg.ini` @ 0x21924e8, Revision 2017)
//! and `.rdata` registry scanner targets (`0x1a267b0`..`0x1a26f10`):
//! - `R_SHARED_DLLS` -> `HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\SharedDlls` (0x1a26a00)
//! - `R_APP_PATHS`   -> `Software\Microsoft\Windows\CurrentVersion\App Paths` (0x1a26840)
//! - `R_FILE_EXTS`   -> `Software\Microsoft\Windows\CurrentVersion\Explorer\FileExts\` (0x1a26a70)
//! - `R_MUICACHE`    -> `Software\Classes\Local Settings\Software\Microsoft\Windows\Shell\MuiCache` (0x1a26bd0)
//! - `R_FONTS`       -> `Software\Microsoft\Windows NT\CurrentVersion\Fonts` (0x1a26990)
//! - `R_INSTALLER`   -> `Software\Microsoft\Windows\CurrentVersion\Installer\Folders` (0x1a26dc0)
//! - `R_RUNSTARTUP`  -> `Software\Microsoft\Windows\CurrentVersion\Run` (0x19fc8a0)
//! - `AppCompat`     -> `Software\Microsoft\Windows NT\CurrentVersion\AppCompatFlags\Compatibility Assistant\Persisted` (0x1a268b0)

use chrono::Local;
use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use winreg::enums::*;
use winreg::RegKey;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RegistryCategory {
    SharedDlls,
    AppPaths,
    FileExts,
    MuiCache,
    AppCompatFlags,
    RunStartupOrphans,
    InstallerFolders,
}

impl RegistryCategory {
    pub fn all() -> &'static [RegistryCategory] {
        &[
            RegistryCategory::SharedDlls,
            RegistryCategory::AppPaths,
            RegistryCategory::FileExts,
            RegistryCategory::MuiCache,
            RegistryCategory::AppCompatFlags,
            RegistryCategory::RunStartupOrphans,
            RegistryCategory::InstallerFolders,
        ]
    }

    pub fn code(&self) -> &'static str {
        match self {
            RegistryCategory::SharedDlls => "R_SHARED_DLLS",
            RegistryCategory::AppPaths => "R_APP_PATHS",
            RegistryCategory::FileExts => "R_FILE_EXTS",
            RegistryCategory::MuiCache => "R_MUICACHE",
            RegistryCategory::AppCompatFlags => "R_APP_OPENWITH",
            RegistryCategory::RunStartupOrphans => "R_RUNSTARTUP",
            RegistryCategory::InstallerFolders => "R_INSTALLER",
        }
    }

    pub fn friendly_title(&self) -> &'static str {
        match self {
            RegistryCategory::SharedDlls => "DLL Condivise",
            RegistryCategory::AppPaths => "Percorsi Applicazioni",
            RegistryCategory::FileExts => "Estensioni File",
            RegistryCategory::MuiCache => "Cache MUI",
            RegistryCategory::AppCompatFlags => "Compatibilità Applicazioni",
            RegistryCategory::RunStartupOrphans => "Voci di Avvio Orfane",
            RegistryCategory::InstallerFolders => "Cartelle di Installazione",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            RegistryCategory::SharedDlls => "DLL Condivise Mancanti (R_SHARED_DLLS)",
            RegistryCategory::AppPaths => "Percorsi Applicazioni Errati (R_APP_PATHS)",
            RegistryCategory::FileExts => "Estensioni File Non Valide (R_FILE_EXTS)",
            RegistryCategory::MuiCache => "Cache MUI Orfana (R_MUICACHE)",
            RegistryCategory::AppCompatFlags => "Cache Compatibilità Applicazioni (AppCompat)",
            RegistryCategory::RunStartupOrphans => "Voci di Avvio Orfane (R_RUNSTARTUP)",
            RegistryCategory::InstallerFolders => "Cartelle Installer Mancanti (R_INSTALLER)",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HiveRoot {
    Hkcu,
    Hklm,
}

impl HiveRoot {
    pub fn prefix(&self) -> &'static str {
        match self {
            HiveRoot::Hkcu => "HKEY_CURRENT_USER",
            HiveRoot::Hklm => "HKEY_LOCAL_MACHINE",
        }
    }

    pub fn short_prefix(&self) -> &'static str {
        match self {
            HiveRoot::Hkcu => "HKCU",
            HiveRoot::Hklm => "HKLM",
        }
    }

    fn open_root(&self) -> RegKey {
        match self {
            HiveRoot::Hkcu => RegKey::predef(HKEY_CURRENT_USER),
            HiveRoot::Hklm => RegKey::predef(HKEY_LOCAL_MACHINE),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RegTargetAction {
    /// Delete a specific value inside `key_path`
    DeleteValue {
        value_name: String,
        original_data: String,
        is_dword: bool,
    },
    /// Delete an entire empty/orphaned subkey at `key_path`
    DeleteSubkey { default_value: Option<String> },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryIssue {
    pub selected: bool,
    pub category: RegistryCategory,
    pub problem_description: String,
    pub data_reference: String,
    pub hive: HiveRoot,
    pub key_path: String,
    pub action: RegTargetAction,
}

impl RegistryIssue {
    pub fn full_reg_path(&self) -> String {
        format!("{}\\{}", self.hive.short_prefix(), self.key_path)
    }
}

/// Normalizes a command line or registry file reference into a concrete filesystem path for existence checking.
pub fn extract_executable_path(raw: &str) -> Option<PathBuf> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }

    // Expand environment variables like %SystemRoot% or %ProgramFiles%
    let mut s = trimmed.to_string();
    if let Ok(windir) = env::var("WINDIR") {
        s = s.replace("%SystemRoot%", &windir)
            .replace("%systemroot%", &windir)
            .replace("%WINDIR%", &windir)
            .replace("%windir%", &windir);
    }
    if let Ok(pf) = env::var("ProgramFiles") {
        s = s.replace("%ProgramFiles%", &pf).replace("%programfiles%", &pf);
    }
    if let Ok(pfx86) = env::var("ProgramFiles(x86)") {
        s = s.replace("%ProgramFiles(x86)%", &pfx86);
    }
    if let Ok(localapp) = env::var("LOCALAPPDATA") {
        s = s.replace("%LOCALAPPDATA%", &localapp).replace("%LocalAppData%", &localapp);
    }
    if let Ok(appdata) = env::var("APPDATA") {
        s = s.replace("%APPDATA%", &appdata).replace("%AppData%", &appdata);
    }
    if let Ok(up) = env::var("USERPROFILE") {
        s = s.replace("%USERPROFILE%", &up).replace("%UserProfile%", &up);
    }

    // Strip MuiCache suffixes like `.FriendlyAppName` or `.ApplicationCompany`
    for suffix in [".FriendlyAppName", ".ApplicationCompany"] {
        if let Some(stripped) = s.strip_suffix(suffix) {
            s = stripped.to_string();
        }
    }

    // Strip leading `@` used in some MUI strings
    if let Some(stripped) = s.strip_prefix('@') {
        s = stripped.to_string();
    }

    // If quoted `"C:\path\to\app.exe" --arg`
    if let Some(rest) = s.strip_prefix('"') {
        if let Some(end_q) = rest.find('"') {
            let candidate = &rest[..end_q];
            return Some(PathBuf::from(candidate));
        }
    }

    // Check if there is a comma for resource index e.g. `C:\path\app.exe,-101`
    if let Some((before_comma, _)) = s.split_once(',') {
        let b = before_comma.trim().trim_matches('"');
        if b.len() > 3 && b.as_bytes().get(1) == Some(&b':') {
            return Some(PathBuf::from(b));
        }
    }

    // Otherwise look for `.exe`, `.dll`, `.ocx`, `.hlp`, `.chm` boundary from right to left
    let lower = s.to_lowercase();
    for ext in [".exe", ".dll", ".ocx", ".sys", ".chm", ".hlp"] {
        let mut search_slice = lower.as_str();
        while let Some(idx) = search_slice.rfind(ext) {
            let end_idx = idx + ext.len();
            // Extension must be at the end of the string or followed by a delimiter (space, switch, comma, quote)
            let is_boundary = end_idx == search_slice.len()
                || matches!(
                    search_slice.as_bytes().get(end_idx),
                    Some(b' ' | b'\t' | b'/' | b'-' | b',' | b'"')
                );
            if is_boundary {
                let candidate = s[..end_idx].trim().trim_matches('"');
                if candidate.len() > 3 && candidate.as_bytes().get(1) == Some(&b':') {
                    return Some(PathBuf::from(candidate));
                }
            }
            search_slice = &search_slice[..idx];
        }
    }

    if s.len() > 3 && s.as_bytes().get(1) == Some(&b':') {
        Some(PathBuf::from(s))
    } else {
        None
    }
}

/// Scans the Windows Registry across the enabled categories and returns all detected orphaned/invalid entries.
pub fn scan_registry(enabled_categories: &[RegistryCategory]) -> Vec<RegistryIssue> {
    let mut issues = Vec::new();

    // 1. R_SHARED_DLLS (`HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\SharedDlls`)
    if enabled_categories.contains(&RegistryCategory::SharedDlls) {
        let key_path = r"SOFTWARE\Microsoft\Windows\CurrentVersion\SharedDlls";
        if let Ok(key) = HiveRoot::Hklm.open_root().open_subkey_with_flags(key_path, KEY_READ) {
            for item in key.enum_values().flatten() {
                let (val_name, reg_val) = item;
                if let Some(p) = extract_executable_path(&val_name) {
                    if !p.exists() {
                        let dword_str = key
                            .get_value::<u32, _>(&val_name)
                            .map(|v| format!("{v:08x}"))
                            .unwrap_or_else(|_| format!("{:?}", reg_val.bytes));
                        issues.push(RegistryIssue {
                            selected: true,
                            category: RegistryCategory::SharedDlls,
                            problem_description: "DLL condivisa mancante".to_string(),
                            data_reference: val_name.clone(),
                            hive: HiveRoot::Hklm,
                            key_path: key_path.to_string(),
                            action: RegTargetAction::DeleteValue {
                                value_name: val_name,
                                original_data: dword_str,
                                is_dword: true,
                            },
                        });
                    }
                }
            }
        }
    }

    // 2. R_APP_PATHS (`HKCU` & `HKLM` `Software\Microsoft\Windows\CurrentVersion\App Paths`)
    if enabled_categories.contains(&RegistryCategory::AppPaths) {
        let base_path = r"Software\Microsoft\Windows\CurrentVersion\App Paths";
        for hive in [HiveRoot::Hkcu, HiveRoot::Hklm] {
            if let Ok(app_paths) = hive.open_root().open_subkey_with_flags(base_path, KEY_READ) {
                for sub_name in app_paths.enum_keys().flatten() {
                    let sub_full = format!("{base_path}\\{sub_name}");
                    if let Ok(subkey) = app_paths.open_subkey_with_flags(&sub_name, KEY_READ) {
                        if let Ok(default_val) = subkey.get_value::<String, _>("") {
                            if let Some(exe_path) = extract_executable_path(&default_val) {
                                if !exe_path.exists() {
                                    issues.push(RegistryIssue {
                                        selected: true,
                                        category: RegistryCategory::AppPaths,
                                        problem_description: "Percorso applicazione inesistente".to_string(),
                                        data_reference: format!("{} -> {}", sub_name, exe_path.display()),
                                        hive,
                                        key_path: sub_full,
                                        action: RegTargetAction::DeleteSubkey {
                                            default_value: Some(default_val),
                                        },
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // 3. R_MUICACHE (`HKCU\Software\Classes\Local Settings\Software\Microsoft\Windows\Shell\MuiCache`)
    if enabled_categories.contains(&RegistryCategory::MuiCache) {
        let key_path = r"Software\Classes\Local Settings\Software\Microsoft\Windows\Shell\MuiCache";
        if let Ok(key) = HiveRoot::Hkcu.open_root().open_subkey_with_flags(key_path, KEY_READ) {
            for (val_name, _) in key.enum_values().flatten() {
                if val_name == "LangID" {
                    continue;
                }
                if let Some(exe_path) = extract_executable_path(&val_name) {
                    if !exe_path.exists() {
                        let orig = key.get_value::<String, _>(&val_name).unwrap_or_default();
                        issues.push(RegistryIssue {
                            selected: true,
                            category: RegistryCategory::MuiCache,
                            problem_description: "Riferimento MUI Cache a eseguibile rimosso".to_string(),
                            data_reference: format!("{} ({})", exe_path.display(), orig),
                            hive: HiveRoot::Hkcu,
                            key_path: key_path.to_string(),
                            action: RegTargetAction::DeleteValue {
                                value_name: val_name,
                                original_data: orig,
                                is_dword: false,
                            },
                        });
                    }
                }
            }
        }
    }

    // 4. R_APP_OPENWITH / AppCompatFlags (`HKCU\Software\Microsoft\Windows NT\CurrentVersion\AppCompatFlags\Compatibility Assistant\Persisted`)
    if enabled_categories.contains(&RegistryCategory::AppCompatFlags) {
        let key_path = r"Software\Microsoft\Windows NT\CurrentVersion\AppCompatFlags\Compatibility Assistant\Persisted";
        if let Ok(key) = HiveRoot::Hkcu.open_root().open_subkey_with_flags(key_path, KEY_READ) {
            for (val_name, _) in key.enum_values().flatten() {
                if let Some(exe_path) = extract_executable_path(&val_name) {
                    if !exe_path.exists() {
                        let orig = key
                            .get_value::<u32, _>(&val_name)
                            .map(|v| format!("{v:08x}"))
                            .unwrap_or_else(|_| "00000001".to_string());
                        issues.push(RegistryIssue {
                            selected: true,
                            category: RegistryCategory::AppCompatFlags,
                            problem_description: "Voce Compatibility Assistant per programma rimosso".to_string(),
                            data_reference: val_name.clone(),
                            hive: HiveRoot::Hkcu,
                            key_path: key_path.to_string(),
                            action: RegTargetAction::DeleteValue {
                                value_name: val_name,
                                original_data: orig,
                                is_dword: true,
                            },
                        });
                    }
                }
            }
        }
    }

    // 5. R_FILE_EXTS (`HKCU\Software\Microsoft\Windows\CurrentVersion\Explorer\FileExts`)
    if enabled_categories.contains(&RegistryCategory::FileExts) {
        let base_path = r"Software\Microsoft\Windows\CurrentVersion\Explorer\FileExts";
        if let Ok(file_exts) = HiveRoot::Hkcu.open_root().open_subkey_with_flags(base_path, KEY_READ) {
            for ext_name in file_exts.enum_keys().flatten() {
                if !ext_name.starts_with('.') {
                    continue;
                }
                let ext_subpath = format!("{base_path}\\{ext_name}");
                if let Ok(ext_key) = file_exts.open_subkey_with_flags(&ext_name, KEY_READ) {
                    // Check if subkey has OpenWithList and OpenWithProgids that are both completely empty
                    // and no UserChoice
                    let has_user_choice = ext_key.open_subkey("UserChoice").is_ok();
                    if has_user_choice {
                        continue;
                    }
                    let owl_empty = match ext_key.open_subkey("OpenWithList") {
                        Ok(k) => k.enum_values().count() == 0 && k.enum_keys().count() == 0,
                        Err(_) => true,
                    };
                    let owp_empty = match ext_key.open_subkey("OpenWithProgids") {
                        Ok(k) => k.enum_values().count() == 0 && k.enum_keys().count() == 0,
                        Err(_) => true,
                    };
                    let root_vals = ext_key.enum_values().count();
                    let sub_count = ext_key.enum_keys().count();
                    if owl_empty && owp_empty && root_vals == 0 && sub_count <= 2 {
                        // Also verify if HKCR\<ext> does not exist
                        let hkcr = RegKey::predef(HKEY_CLASSES_ROOT);
                        if hkcr.open_subkey(&ext_name).is_err() {
                            issues.push(RegistryIssue {
                                selected: true,
                                category: RegistryCategory::FileExts,
                                problem_description: "Estensione file inutilizzata senza ProgID associato".to_string(),
                                data_reference: ext_name,
                                hive: HiveRoot::Hkcu,
                                key_path: ext_subpath,
                                action: RegTargetAction::DeleteSubkey { default_value: None },
                            });
                        }
                    }
                }
            }
        }
    }

    // 6. R_RUNSTARTUP (`HKCU` & `HKLM` `Software\Microsoft\Windows\CurrentVersion\Run`)
    if enabled_categories.contains(&RegistryCategory::RunStartupOrphans) {
        let run_path = r"Software\Microsoft\Windows\CurrentVersion\Run";
        for hive in [HiveRoot::Hkcu, HiveRoot::Hklm] {
            if let Ok(key) = hive.open_root().open_subkey_with_flags(run_path, KEY_READ) {
                for (val_name, _) in key.enum_values().flatten() {
                    if let Ok(cmd) = key.get_value::<String, _>(&val_name) {
                        if let Some(exe_path) = extract_executable_path(&cmd) {
                            if !exe_path.exists() {
                                issues.push(RegistryIssue {
                                    selected: true,
                                    category: RegistryCategory::RunStartupOrphans,
                                    problem_description: "Programma di avvio automatico non più presente su disco".to_string(),
                                    data_reference: format!("{val_name} -> {cmd}"),
                                    hive,
                                    key_path: run_path.to_string(),
                                    action: RegTargetAction::DeleteValue {
                                        value_name: val_name,
                                        original_data: cmd,
                                        is_dword: false,
                                    },
                                });
                            }
                        }
                    }
                }
            }
        }
    }

    // 7. R_INSTALLER (`HKLM\Software\Microsoft\Windows\CurrentVersion\Installer\Folders`)
    if enabled_categories.contains(&RegistryCategory::InstallerFolders) {
        let key_path = r"Software\Microsoft\Windows\CurrentVersion\Installer\Folders";
        if let Ok(key) = HiveRoot::Hklm.open_root().open_subkey_with_flags(key_path, KEY_READ) {
            for (folder_path, _) in key.enum_values().flatten() {
                let p = Path::new(&folder_path);
                if folder_path.len() > 3 && !p.exists() {
                    let orig = key.get_value::<String, _>(&folder_path).unwrap_or_default();
                    issues.push(RegistryIssue {
                        selected: true,
                        category: RegistryCategory::InstallerFolders,
                        problem_description: "Cartella Windows Installer non più esistente".to_string(),
                        data_reference: folder_path.clone(),
                        hive: HiveRoot::Hklm,
                        key_path: key_path.to_string(),
                        action: RegTargetAction::DeleteValue {
                            value_name: folder_path,
                            original_data: orig,
                            is_dword: false,
                        },
                    });
                }
            }
        }
    }

    issues
}

fn escape_reg_str(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

/// Exports the selected registry issues into a standard `Windows Registry Editor Version 5.00` `.reg` backup file
/// BEFORE any modification is made to the Windows Registry.
pub fn export_reg_backup(issues: &[RegistryIssue], backup_dir: &Path) -> Result<PathBuf, String> {
    fs::create_dir_all(backup_dir)
        .map_err(|e| format!("Impossibile creare la cartella di backup '{}': {e}", backup_dir.display()))?;

    let timestamp = Local::now().format("%Y%m%d_%H%M%S");
    let file_path = backup_dir.join(format!("cc_registry_backup_{timestamp}.reg"));

    let mut content = String::from("Windows Registry Editor Version 5.00\r\n\r\n");
    content.push_str(&format!(
        "; Backup generato automaticamente da RustCleaner (RE di CCleaner)\r\n; Data: {}\r\n\r\n",
        Local::now().format("%Y-%m-%d %H:%M:%S")
    ));

    for issue in issues.iter().filter(|i| i.selected) {
        content.push_str(&format!(
            "; Problema: {} ({})\r\n",
            issue.problem_description,
            issue.category.code()
        ));
        content.push_str(&format!("[{}\\{}]\r\n", issue.hive.prefix(), issue.key_path));
        match &issue.action {
            RegTargetAction::DeleteValue {
                value_name,
                original_data,
                is_dword,
            } => {
                let is_default_val = value_name.is_empty() || value_name == "@";
                if *is_dword {
                    if is_default_val {
                        content.push_str(&format!("@=dword:{original_data}\r\n\r\n"));
                    } else {
                        let escaped_name = escape_reg_str(value_name);
                        content.push_str(&format!("\"{escaped_name}\"=dword:{original_data}\r\n\r\n"));
                    }
                } else {
                    let escaped_val = escape_reg_str(original_data);
                    if is_default_val {
                        content.push_str(&format!("@=\"{escaped_val}\"\r\n\r\n"));
                    } else {
                        let escaped_name = escape_reg_str(value_name);
                        content.push_str(&format!("\"{escaped_name}\"=\"{escaped_val}\"\r\n\r\n"));
                    }
                }
            }
            RegTargetAction::DeleteSubkey { default_value } => {
                if let Some(def) = default_value {
                    content.push_str(&format!("@=\"{}\"\r\n\r\n", escape_reg_str(def)));
                } else {
                    content.push_str("\r\n");
                }
            }
        }
    }

    fs::write(&file_path, content.as_bytes())
        .map_err(|e| format!("Errore nel salvataggio del backup .reg '{}': {e}", file_path.display()))?;

    Ok(file_path)
}

#[derive(Debug, Clone)]
pub struct RegistryFixReport {
    pub backup_file: PathBuf,
    pub fixed_count: usize,
    pub failed_count: usize,
    pub errors: Vec<String>,
}

/// Always creates a `.reg` backup first, and only if the backup succeeds, cleans the selected Registry issues.
pub fn backup_and_fix_registry(
    issues: &[RegistryIssue],
    backup_dir: &Path,
) -> Result<RegistryFixReport, String> {
    let selected: Vec<&RegistryIssue> = issues.iter().filter(|i| i.selected).collect();
    if selected.is_empty() {
        return Err("Nessuna voce selezionata per la riparazione.".to_string());
    }

    // Mandatory Step 1: Export .reg backup before touching the Registry!
    let backup_file = export_reg_backup(issues, backup_dir)?;

    let mut fixed_count = 0usize;
    let mut failed_count = 0usize;
    let mut errors = Vec::new();

    for issue in selected {
        let root = issue.hive.open_root();
        match &issue.action {
            RegTargetAction::DeleteValue { value_name, .. } => {
                match root.open_subkey_with_flags(&issue.key_path, KEY_SET_VALUE) {
                    Ok(subkey) => match subkey.delete_value(value_name) {
                        Ok(()) => fixed_count += 1,
                        Err(e) => {
                            failed_count += 1;
                            errors.push(format!(
                                "Impossibile eliminare il valore '{}' in {}: {e}",
                                value_name,
                                issue.full_reg_path()
                            ));
                        }
                    },
                    Err(e) => {
                        failed_count += 1;
                        errors.push(format!(
                            "Accesso negato a {} (avvia come Amministratore): {e}",
                            issue.full_reg_path()
                        ));
                    }
                }
            }
            RegTargetAction::DeleteSubkey { .. } => {
                match root.delete_subkey_all(&issue.key_path) {
                    Ok(()) => fixed_count += 1,
                    Err(e) => {
                        failed_count += 1;
                        errors.push(format!(
                            "Impossibile eliminare la sottochiave {}: {e}",
                            issue.full_reg_path()
                        ));
                    }
                }
            }
        }
    }

    Ok(RegistryFixReport {
        backup_file,
        fixed_count,
        failed_count,
        errors,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_executable_path() {
        let p1 = extract_executable_path(r#""C:\Program Files\App\app.exe" --minimized"#).unwrap();
        assert_eq!(p1, PathBuf::from(r"C:\Program Files\App\app.exe"));

        let p2 = extract_executable_path(r"C:\Windows\System32\notepad.exe /A file.txt").unwrap();
        assert_eq!(p2, PathBuf::from(r"C:\Windows\System32\notepad.exe"));

        // Nested directory with .exe in folder name
        let p3 = extract_executable_path(r"C:\Games\Game.exe.backup\launcher.exe -silent").unwrap();
        assert_eq!(p3, PathBuf::from(r"C:\Games\Game.exe.backup\launcher.exe"));
    }

    #[test]
    fn test_escape_reg_str() {
        assert_eq!(
            escape_reg_str(r#"C:\Program Files\"Test"\app.exe"#),
            r#"C:\\Program Files\\\"Test\"\\app.exe"#
        );
    }

    #[test]
    fn test_export_reg_backup_creates_valid_file() {
        let temp_dir = std::env::temp_dir().join("rustcleaner_test_backup");
        let issue1 = RegistryIssue {
            selected: true,
            category: RegistryCategory::SharedDlls,
            problem_description: "DLL condivisa mancante".to_string(),
            data_reference: r"C:\Missing\lib.dll".to_string(),
            hive: HiveRoot::Hklm,
            key_path: r"Software\Microsoft\Windows\CurrentVersion\SharedDLLs".to_string(),
            action: RegTargetAction::DeleteValue {
                value_name: r"C:\Missing\lib.dll".to_string(),
                original_data: "00000001".to_string(),
                is_dword: true,
            },
        };
        // Test default value formatting (@="...")
        let issue2 = RegistryIssue {
            selected: true,
            category: RegistryCategory::AppPaths,
            problem_description: "Percorso applicazione orfano".to_string(),
            data_reference: r"C:\NonExistent\app.exe".to_string(),
            hive: HiveRoot::Hkcu,
            key_path: r"Software\Microsoft\Windows\CurrentVersion\App Paths\test.exe".to_string(),
            action: RegTargetAction::DeleteValue {
                value_name: String::new(),
                original_data: r"C:\NonExistent\app.exe".to_string(),
                is_dword: false,
            },
        };
        let path = export_reg_backup(&[issue1, issue2], &temp_dir).expect("backup should succeed");
        let text = fs::read_to_string(&path).expect("read backup");
        assert!(text.starts_with("Windows Registry Editor Version 5.00"));
        assert!(text.contains(r#""C:\\Missing\\lib.dll"=dword:00000001"#));
        assert!(text.contains(r#"@="C:\\NonExistent\\app.exe""#));
        let _ = fs::remove_dir_all(&temp_dir);
    }
}

