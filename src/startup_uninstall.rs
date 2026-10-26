//! Startup Manager & Software Uninstaller Engine
//! Reconstructed from CCleaner64.exe `.rdata` strings (`0x1a16ae8`..`0x1a184a8`):
//! - Startup sources:
//!   - `HKCU:Run`, `HKCU:RunOnce`, `HKLM:Run`, `HKLM:RunOnce`, `WOW6432Node\...\Run` (`0x1a16ae8`..`0x1a16ee0`)
//!   - `Startup User` (`.Startup`) & `Startup Common` (`.CommonStartup`) (`0x1a16b20`..`0x1a16b80`)
//!   - `Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run`, `Run32`, `StartupFolder` (`0x1a17e90`, `0x1a18400`)
//! - Uninstaller sources:
//!   - `HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall` (`0x19e6100`)
//!   - `HKLM\SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall` (`0x19e6070`)
//!   - `HKCU\Software\Microsoft\Windows\CurrentVersion\Uninstall` (`0x1a171f0`)
//!   - Values: `DisplayName`, `Publisher`, `DisplayVersion`, `EstimatedSize`, `UninstallString`,
//!     `WindowsInstaller`, `SystemComponent` (`0x1a175d8`..`0x1a177b8`)
//!   - MSI command template: `msiexec.exe /I{GUID}` or `/X{GUID}` (`0x1a178e0`).

use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use winreg::enums::*;
use winreg::{RegKey, RegValue};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StartupSource {
    HkcuRun,
    HkcuRunOnce,
    HklmRun,
    HklmWow64Run,
    StartupFolderUser,
    StartupFolderCommon,
}

impl StartupSource {
    pub fn key_label(&self) -> &'static str {
        match self {
            StartupSource::HkcuRun => "HKCU:Run",
            StartupSource::HkcuRunOnce => "HKCU:RunOnce",
            StartupSource::HklmRun => "HKLM:Run",
            StartupSource::HklmWow64Run => "HKLM:Run (32-bit)",
            StartupSource::StartupFolderUser => "Startup User",
            StartupSource::StartupFolderCommon => "Startup Common",
        }
    }

    pub fn requires_admin(&self) -> bool {
        matches!(
            self,
            StartupSource::HklmRun | StartupSource::HklmWow64Run | StartupSource::StartupFolderCommon
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StartupEntry {
    pub name: String,
    pub command: String,
    pub source: StartupSource,
    pub enabled: bool,
    pub file_path_if_folder: Option<PathBuf>,
}

/// Checks Windows `StartupApproved` binary value (`0x1a17e90`).
/// In Windows 10/11 and CCleaner, the first byte of the 12-byte `REG_BINARY` value determines status:
/// - `0x02` or `0x06` (even first byte) => Enabled
/// - `0x03` or `0x01` (odd first byte) => Disabled
fn is_startup_approved(hive: &RegKey, subkey_suffix: &str, item_name: &str) -> bool {
    let path = format!(
        r"Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\{subkey_suffix}"
    );
    if let Ok(key) = hive.open_subkey_with_flags(&path, KEY_READ) {
        if let Ok(raw) = key.get_raw_value(item_name) {
            if let Some(&first_byte) = raw.bytes.first() {
                return first_byte % 2 == 0;
            }
        }
    }
    true
}

/// Lists all Windows Startup entries across Registry `Run` / `RunOnce` keys and Startup folders.
pub fn list_startup_entries() -> Vec<StartupEntry> {
    let mut entries = Vec::new();
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);

    // 1. HKCU\Software\Microsoft\Windows\CurrentVersion\Run
    if let Ok(key) = hkcu.open_subkey_with_flags(
        r"Software\Microsoft\Windows\CurrentVersion\Run",
        KEY_READ,
    ) {
        for (name, _) in key.enum_values().flatten() {
            if let Ok(cmd) = key.get_value::<String, _>(&name) {
                let enabled = is_startup_approved(&hkcu, "Run", &name);
                entries.push(StartupEntry {
                    name,
                    command: cmd,
                    source: StartupSource::HkcuRun,
                    enabled,
                    file_path_if_folder: None,
                });
            }
        }
    }

    // 2. HKCU\Software\Microsoft\Windows\CurrentVersion\RunOnce
    if let Ok(key) = hkcu.open_subkey_with_flags(
        r"Software\Microsoft\Windows\CurrentVersion\RunOnce",
        KEY_READ,
    ) {
        for (name, _) in key.enum_values().flatten() {
            if let Ok(cmd) = key.get_value::<String, _>(&name) {
                entries.push(StartupEntry {
                    name,
                    command: cmd,
                    source: StartupSource::HkcuRunOnce,
                    enabled: true,
                    file_path_if_folder: None,
                });
            }
        }
    }

    // 3. HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Run
    if let Ok(key) = hklm.open_subkey_with_flags(
        r"SOFTWARE\Microsoft\Windows\CurrentVersion\Run",
        KEY_READ,
    ) {
        for (name, _) in key.enum_values().flatten() {
            if let Ok(cmd) = key.get_value::<String, _>(&name) {
                let enabled = is_startup_approved(&hklm, "Run", &name);
                entries.push(StartupEntry {
                    name,
                    command: cmd,
                    source: StartupSource::HklmRun,
                    enabled,
                    file_path_if_folder: None,
                });
            }
        }
    }

    // 4. HKLM\SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Run
    if let Ok(key) = hklm.open_subkey_with_flags(
        r"SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Run",
        KEY_READ,
    ) {
        for (name, _) in key.enum_values().flatten() {
            if let Ok(cmd) = key.get_value::<String, _>(&name) {
                let enabled = is_startup_approved(&hklm, "Run32", &name);
                entries.push(StartupEntry {
                    name,
                    command: cmd,
                    source: StartupSource::HklmWow64Run,
                    enabled,
                    file_path_if_folder: None,
                });
            }
        }
    }

    // 5. User Startup Folder (`%APPDATA%\Microsoft\Windows\Start Menu\Programs\Startup`)
    if let Ok(appdata) = env::var("APPDATA") {
        let user_startup = PathBuf::from(appdata)
            .join(r"Microsoft\Windows\Start Menu\Programs\Startup");
        if let Ok(dir) = fs::read_dir(&user_startup) {
            for entry in dir.flatten() {
                let path = entry.path();
                let Some(fname) = path.file_name().and_then(|n| n.to_str()) else {
                    continue;
                };
                if fname.eq_ignore_ascii_case("desktop.ini") {
                    continue;
                }
                let enabled = is_startup_approved(&hkcu, "StartupFolder", fname);
                entries.push(StartupEntry {
                    name: fname.to_string(),
                    command: path.display().to_string(),
                    source: StartupSource::StartupFolderUser,
                    enabled,
                    file_path_if_folder: Some(path),
                });
            }
        }
    }

    // 6. Common Startup Folder (`%ProgramData%\Microsoft\Windows\Start Menu\Programs\Startup`)
    if let Ok(progdata) = env::var("ProgramData") {
        let common_startup = PathBuf::from(progdata)
            .join(r"Microsoft\Windows\Start Menu\Programs\Startup");
        if let Ok(dir) = fs::read_dir(&common_startup) {
            for entry in dir.flatten() {
                let path = entry.path();
                let Some(fname) = path.file_name().and_then(|n| n.to_str()) else {
                    continue;
                };
                if fname.eq_ignore_ascii_case("desktop.ini") {
                    continue;
                }
                let enabled = is_startup_approved(&hklm, "StartupFolder", fname);
                entries.push(StartupEntry {
                    name: fname.to_string(),
                    command: path.display().to_string(),
                    source: StartupSource::StartupFolderCommon,
                    enabled,
                    file_path_if_folder: Some(path),
                });
            }
        }
    }

    entries
}

/// Toggles a Startup item ON or OFF using Windows `StartupApproved` registry binary flags,
/// matching CCleaner64.exe (`0x1a17e90` & `0x1a18400`).
pub fn set_startup_enabled(entry: &StartupEntry, enable: bool) -> Result<(), String> {
    let (hive, suffix) = match entry.source {
        StartupSource::HkcuRun | StartupSource::HkcuRunOnce => {
            (RegKey::predef(HKEY_CURRENT_USER), "Run")
        }
        StartupSource::HklmRun => (RegKey::predef(HKEY_LOCAL_MACHINE), "Run"),
        StartupSource::HklmWow64Run => (RegKey::predef(HKEY_LOCAL_MACHINE), "Run32"),
        StartupSource::StartupFolderUser => {
            (RegKey::predef(HKEY_CURRENT_USER), "StartupFolder")
        }
        StartupSource::StartupFolderCommon => {
            (RegKey::predef(HKEY_LOCAL_MACHINE), "StartupFolder")
        }
    };

    let approved_path = format!(
        r"Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\{suffix}"
    );
    let (key, _) = hive
        .create_subkey(&approved_path)
        .map_err(|e| format!("Impossibile aprire/creare '{}' (richiede UAC per HKLM): {e}", approved_path))?;

    // 12-byte StartupApproved structure: byte[0] = 0x02 (enabled) or 0x03 (disabled), followed by 11 zero/FILETIME bytes
    let mut bytes = vec![0u8; 12];
    bytes[0] = if enable { 0x02 } else { 0x03 };
    let val = RegValue {
        bytes,
        vtype: REG_BINARY,
    };

    key.set_raw_value(&entry.name, &val)
        .map_err(|e| format!("Impossibile aggiornare lo stato di '{}': {e}", entry.name))
}

/// Deletes a Startup item permanently from the Registry or Startup folder.
/// Mandates `.reg` backup creation before any registry modification or deletion.
pub fn delete_startup_entry(
    entry: &StartupEntry,
    backup_dir: Option<&Path>,
) -> Result<PathBuf, String> {
    let default_backup = std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join("registry_backups");
    let target_dir = backup_dir.unwrap_or(&default_backup);

    match entry.source {
        StartupSource::HkcuRun
        | StartupSource::HkcuRunOnce
        | StartupSource::HklmRun
        | StartupSource::HklmWow64Run => {
            let (hive, subkey_path, key_flags) = match entry.source {
                StartupSource::HkcuRun => (
                    crate::registry::HiveRoot::Hkcu,
                    r"Software\Microsoft\Windows\CurrentVersion\Run",
                    KEY_READ | KEY_SET_VALUE,
                ),
                StartupSource::HkcuRunOnce => (
                    crate::registry::HiveRoot::Hkcu,
                    r"Software\Microsoft\Windows\CurrentVersion\RunOnce",
                    KEY_READ | KEY_SET_VALUE,
                ),
                StartupSource::HklmRun => (
                    crate::registry::HiveRoot::Hklm,
                    r"SOFTWARE\Microsoft\Windows\CurrentVersion\Run",
                    KEY_READ | KEY_SET_VALUE,
                ),
                StartupSource::HklmWow64Run => (
                    crate::registry::HiveRoot::Hklm,
                    r"SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Run",
                    KEY_READ | KEY_SET_VALUE,
                ),
                _ => unreachable!(),
            };

            let predef = match hive {
                crate::registry::HiveRoot::Hkcu => RegKey::predef(HKEY_CURRENT_USER),
                crate::registry::HiveRoot::Hklm => RegKey::predef(HKEY_LOCAL_MACHINE),
            };

            let key = predef
                .open_subkey_with_flags(subkey_path, key_flags)
                .map_err(|e| {
                    format!("Impossibile accedere alla chiave di registro '{subkey_path}': {e}")
                })?;

            // Retrieve original data for backup fidelity
            let orig_data = key
                .get_value::<String, _>(&entry.name)
                .unwrap_or_else(|_| entry.command.clone());

            let issue = crate::registry::RegistryIssue {
                selected: true,
                category: crate::registry::RegistryCategory::RunStartupOrphans,
                problem_description: format!("Eliminazione voce di avvio: {}", entry.name),
                data_reference: orig_data.clone(),
                hive,
                key_path: subkey_path.to_string(),
                action: crate::registry::RegTargetAction::DeleteValue {
                    value_name: entry.name.clone(),
                    original_data: orig_data,
                    is_dword: false,
                },
            };

            // Mandatory Step 1: Export valid .reg backup FIRST!
            let backup_file = crate::registry::export_reg_backup(&[issue], target_dir)?;

            // Mandatory Step 2: Delete value only after backup succeeds
            key.delete_value(&entry.name).map_err(|e| {
                format!(
                    "Backup creato con successo in '{}', ma eliminazione fallita: {e}",
                    backup_file.display()
                )
            })?;

            Ok(backup_file)
        }
        StartupSource::StartupFolderUser | StartupSource::StartupFolderCommon => {
            if let Some(p) = &entry.file_path_if_folder {
                if !p.exists() {
                    return Err(format!("File di avvio non trovato: '{}'", p.display()));
                }
                fs::create_dir_all(target_dir)
                    .map_err(|e| format!("Impossibile creare cartella di backup: {e}"))?;
                let fname = p.file_name().unwrap_or_default().to_string_lossy();
                let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
                let backup_file = target_dir.join(format!("{fname}_{timestamp}.bak"));
                fs::copy(p, &backup_file)
                    .map_err(|e| format!("Impossibile creare backup del file di avvio: {e}"))?;
                fs::remove_file(p)
                    .map_err(|e| format!("Impossibile rimuovere '{}': {e}", p.display()))?;
                Ok(backup_file)
            } else {
                Err("Percorso file di avvio mancante.".to_string())
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstalledApp {
    pub display_name: String,
    pub publisher: String,
    pub display_version: String,
    pub install_date: String,
    pub estimated_size_kb: Option<u32>,
    pub uninstall_string: String,
    pub windows_installer: bool,
    pub reg_key_name: String,
}

/// Scans `Uninstall` registry keys across HKLM (64-bit), HKLM (WOW6432Node 32-bit), and HKCU,
/// matching CCleaner64.exe (`0x19e6070`, `0x19e6100`, `0x1a171f0`, `0x1a175d8`..`0x1a177b8`).
pub fn list_installed_apps() -> Vec<InstalledApp> {
    let mut apps = Vec::new();
    let sources = [
        (
            RegKey::predef(HKEY_LOCAL_MACHINE),
            r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall",
        ),
        (
            RegKey::predef(HKEY_LOCAL_MACHINE),
            r"SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall",
        ),
        (
            RegKey::predef(HKEY_CURRENT_USER),
            r"Software\Microsoft\Windows\CurrentVersion\Uninstall",
        ),
    ];

    for (root, path) in sources {
        let Ok(uninst_root) = root.open_subkey_with_flags(path, KEY_READ) else {
            continue;
        };
        for sub_name in uninst_root.enum_keys().flatten() {
            let Ok(subkey) = uninst_root.open_subkey_with_flags(&sub_name, KEY_READ) else {
                continue;
            };

            // Filter out SystemComponent == 1 (`0x1a175f8`) and child updates (`ParentKeyName` @ `0x1a176f0`)
            if subkey.get_value::<u32, _>("SystemComponent").unwrap_or(0) == 1 {
                continue;
            }
            if subkey
                .get_value::<String, _>("ParentKeyName")
                .map(|s| !s.is_empty())
                .unwrap_or(false)
            {
                continue;
            }

            let display_name = subkey
                .get_value::<String, _>("DisplayName")
                .unwrap_or_default()
                .trim()
                .to_string();
            let uninstall_string = subkey
                .get_value::<String, _>("UninstallString")
                .unwrap_or_default()
                .trim()
                .to_string();

            if display_name.is_empty() || uninstall_string.is_empty() {
                continue;
            }

            let publisher = subkey
                .get_value::<String, _>("Publisher")
                .unwrap_or_default();
            let display_version = subkey
                .get_value::<String, _>("DisplayVersion")
                .unwrap_or_default();
            let install_date = subkey
                .get_value::<String, _>("InstallDate")
                .unwrap_or_default();
            let estimated_size_kb = subkey.get_value::<u32, _>("EstimatedSize").ok();
            let windows_installer =
                subkey.get_value::<u32, _>("WindowsInstaller").unwrap_or(0) == 1;

            apps.push(InstalledApp {
                display_name,
                publisher,
                display_version,
                install_date,
                estimated_size_kb,
                uninstall_string,
                windows_installer,
                reg_key_name: sub_name,
            });
        }
    }

    apps.sort_by_key(|a| a.display_name.to_lowercase());
    apps.dedup_by(|a, b| {
        a.display_name.eq_ignore_ascii_case(&b.display_name)
            && a.display_version == b.display_version
    });
    apps
}

/// Parses and launches an application's `UninstallString` via Windows `ShellExecuteW`
/// (with `Command` fallback), replicating CCleaner's uninstaller dispatcher
/// (`"Uninstall - Executing shell command, file: '{}', params: '{}'"` @ `0x1a17410`).
/// Using `ShellExecuteW` ensures Windows automatically shows the UAC elevation prompt
/// when an uninstaller (`unins000.exe`, `setup.exe`, etc.) requires administrator privileges.
pub fn launch_uninstaller(app: &InstalledApp) -> Result<String, String> {
    let raw_unexpanded = app.uninstall_string.trim();
    if raw_unexpanded.is_empty() {
        return Err("UninstallString vuoto.".to_string());
    }

    let expanded_buf = crate::cleaner::expand_env_vars(raw_unexpanded);
    let raw = expanded_buf.to_string_lossy().to_string();
    let raw = raw.trim();

    // Case 1: MSI package (`msiexec.exe /I{GUID}` or `/X{GUID}` @ `0x1a178e0`)
    if raw.to_lowercase().starts_with("msiexec") {
        let mut args_str = raw
            .split_once(' ')
            .map(|(_, rest)| rest.trim().to_string())
            .unwrap_or_default();

        // Convert `/I{GUID}` maintenance flag to `/X{GUID}` uninstall flag when appropriate
        if args_str.contains("/I{") {
            args_str = args_str.replace("/I{", "/X{");
        } else if args_str.contains("/i{") {
            args_str = args_str.replace("/i{", "/X{");
        }

        shell_execute_command("msiexec.exe", &args_str)?;
        return Ok(format!(
            "Avviato Windows Installer per '{}': msiexec.exe {args_str}",
            app.display_name
        ));
    }

    // Case 2: Standard or quoted executable (`"C:\path\uninst.exe" /arg1 /arg2` or `C:\path\uninst.exe /S`)
    let (exe_part, args_part) = split_executable_and_args(raw);

    shell_execute_command(exe_part, args_part)?;

    Ok(format!(
        "Avviato disinstallatore per '{}': \"{exe_part}\" {args_part}",
        app.display_name
    ))
}

/// Splits a raw Windows command string into `(executable_path, arguments)`.
pub fn split_executable_and_args(raw: &str) -> (&str, &str) {
    let trimmed = raw.trim();
    if let Some(rest) = trimmed.strip_prefix('"') {
        if let Some(end_q) = rest.find('"') {
            return (&rest[..end_q], rest[end_q + 1..].trim());
        }
    }
    let lower = trimmed.to_lowercase();
    for ext in [".exe", ".msi", ".bat", ".cmd"] {
        let mut search_slice = lower.as_str();
        while let Some(idx) = search_slice.rfind(ext) {
            let split_at = idx + ext.len();
            let is_boundary = split_at == trimmed.len()
                || matches!(
                    trimmed.as_bytes().get(split_at),
                    Some(b' ' | b'\t' | b'/' | b'-' | b',')
                );
            if is_boundary {
                return (trimmed[..split_at].trim(), trimmed[split_at..].trim());
            }
            search_slice = &search_slice[..idx];
        }
    }
    match trimmed.split_once(' ') {
        Some((exe, args)) => (exe.trim(), args.trim()),
        None => (trimmed, ""),
    }
}

fn to_wide_null(s: &str) -> Vec<u16> {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    OsStr::new(s).encode_wide().chain(std::iter::once(0)).collect()
}

/// Executes `file` with `params` via Win32 `ShellExecuteW`, automatically triggering
/// the Windows UAC elevation dialog if the uninstaller requires Administrator rights.
fn shell_execute_command(file: &str, params: &str) -> Result<(), String> {
    use std::ptr;
    use windows_sys::Win32::Foundation::HWND;
    use windows_sys::Win32::UI::Shell::ShellExecuteW;
    use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

    let verb_open = to_wide_null("open");
    let file_w = to_wide_null(file);
    let params_w = to_wide_null(params);

    let res = unsafe {
        ShellExecuteW(
            ptr::null_mut() as HWND,
            verb_open.as_ptr(),
            file_w.as_ptr(),
            if params.is_empty() {
                ptr::null()
            } else {
                params_w.as_ptr()
            },
            ptr::null(),
            SW_SHOWNORMAL,
        )
    };

    if (res as isize) > 32 {
        return Ok(());
    }

    // If access denied (SE_ERR_ACCESSDENIED == 5), retry with `"runas"` verb to force UAC prompt
    if (res as isize) == 5 {
        let verb_runas = to_wide_null("runas");
        let res_elev = unsafe {
            ShellExecuteW(
                ptr::null_mut() as HWND,
                verb_runas.as_ptr(),
                file_w.as_ptr(),
                if params.is_empty() {
                    ptr::null()
                } else {
                    params_w.as_ptr()
                },
                ptr::null(),
                SW_SHOWNORMAL,
            )
        };
        if (res_elev as isize) > 32 {
            return Ok(());
        }
    }

    // Fallback to `cmd.exe /C start` if ShellExecuteW returned <= 32
    let mut cmd = Command::new(file);
    if !params.is_empty() {
        use std::os::windows::process::CommandExt;
        cmd.raw_arg(params);
    }
    cmd.spawn().map(|_| ()).map_err(|e| {
        format!(
            "Impossibile avviare '{}' (ShellExecuteW code {}): {}",
            file, res as isize, e
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_split_executable_and_args() {
        // Quoted path
        let (exe, args) = split_executable_and_args(r#""C:\Program Files\App\uninst.exe" /S"#);
        assert_eq!(exe, r#"C:\Program Files\App\uninst.exe"#);
        assert_eq!(args, "/S");

        // Unquoted path with .exe in folder name
        let (exe, args) = split_executable_and_args(r#"C:\Games\Game.exe.backup\launcher.exe -silent"#);
        assert_eq!(exe, r#"C:\Games\Game.exe.backup\launcher.exe"#);
        assert_eq!(args, "-silent");

        // Simple unquoted executable with no args
        let (exe, args) = split_executable_and_args(r#"C:\Windows\System32\cmd.exe"#);
        assert_eq!(exe, r#"C:\Windows\System32\cmd.exe"#);
        assert_eq!(args, "");

        // Batch file with switch
        let (exe, args) = split_executable_and_args(r#"C:\Scripts\cleanup.bat /all"#);
        assert_eq!(exe, r#"C:\Scripts\cleanup.bat"#);
        assert_eq!(args, "/all");
    }

    #[test]
    fn test_startup_source_requires_admin() {
        assert!(StartupSource::HklmRun.requires_admin());
        assert!(StartupSource::HklmWow64Run.requires_admin());
        assert!(StartupSource::StartupFolderCommon.requires_admin());
        assert!(!StartupSource::HkcuRun.requires_admin());
        assert!(!StartupSource::StartupFolderUser.requires_admin());
    }

    #[test]
    fn test_delete_startup_entry_folder_creates_backup() {
        let temp_dir = std::env::temp_dir().join("rustcleaner_test_startup_folder");
        let _ = fs::create_dir_all(&temp_dir);
        let shortcut_file = temp_dir.join("test_app.lnk");
        let _ = fs::write(&shortcut_file, "mock shortcut content");

        let backup_dir = temp_dir.join("backups");
        let entry = StartupEntry {
            name: "TestApp".to_string(),
            command: shortcut_file.to_string_lossy().to_string(),
            source: StartupSource::StartupFolderUser,
            enabled: true,
            file_path_if_folder: Some(shortcut_file.clone()),
        };

        let backup_file =
            delete_startup_entry(&entry, Some(&backup_dir)).expect("should delete and backup");
        assert!(backup_file.exists(), "backup file should exist");
        assert!(!shortcut_file.exists(), "original shortcut should be deleted");

        let _ = fs::remove_dir_all(&temp_dir);
    }
}
