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
use std::path::PathBuf;
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
pub fn delete_startup_entry(entry: &StartupEntry) -> Result<(), String> {
    match entry.source {
        StartupSource::HkcuRun => {
            let key = RegKey::predef(HKEY_CURRENT_USER)
                .open_subkey_with_flags(r"Software\Microsoft\Windows\CurrentVersion\Run", KEY_SET_VALUE)
                .map_err(|e| e.to_string())?;
            key.delete_value(&entry.name).map_err(|e| e.to_string())
        }
        StartupSource::HkcuRunOnce => {
            let key = RegKey::predef(HKEY_CURRENT_USER)
                .open_subkey_with_flags(r"Software\Microsoft\Windows\CurrentVersion\RunOnce", KEY_SET_VALUE)
                .map_err(|e| e.to_string())?;
            key.delete_value(&entry.name).map_err(|e| e.to_string())
        }
        StartupSource::HklmRun => {
            let key = RegKey::predef(HKEY_LOCAL_MACHINE)
                .open_subkey_with_flags(r"SOFTWARE\Microsoft\Windows\CurrentVersion\Run", KEY_SET_VALUE)
                .map_err(|e| format!("Accesso negato a HKLM:Run (avvia come Amministratore): {e}"))?;
            key.delete_value(&entry.name).map_err(|e| e.to_string())
        }
        StartupSource::HklmWow64Run => {
            let key = RegKey::predef(HKEY_LOCAL_MACHINE)
                .open_subkey_with_flags(
                    r"SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Run",
                    KEY_SET_VALUE,
                )
                .map_err(|e| format!("Accesso negato a HKLM:WOW6432Node:Run: {e}"))?;
            key.delete_value(&entry.name).map_err(|e| e.to_string())
        }
        StartupSource::StartupFolderUser | StartupSource::StartupFolderCommon => {
            if let Some(p) = &entry.file_path_if_folder {
                fs::remove_file(p).map_err(|e| format!("Impossibile rimuovere '{}': {e}", p.display()))
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

    apps.sort_by(|a, b| a.display_name.to_lowercase().cmp(&b.display_name.to_lowercase()));
    apps.dedup_by(|a, b| {
        a.display_name.eq_ignore_ascii_case(&b.display_name)
            && a.display_version == b.display_version
    });
    apps
}

/// Parses and launches an application's `UninstallString` via `std::process::Command`,
/// replicating CCleaner's uninstaller dispatcher (`"Uninstall - Executing shell command, file: '{}', params: '{}'"` @ `0x1a17410`).
pub fn launch_uninstaller(app: &InstalledApp) -> Result<String, String> {
    let raw = app.uninstall_string.trim();
    if raw.is_empty() {
        return Err("UninstallString vuoto.".to_string());
    }

    // If WindowsInstaller (`msiexec.exe /I{GUID}` or `/X{GUID}` @ `0x1a178e0`)
    if app.windows_installer || raw.to_lowercase().starts_with("msiexec") {
        let args_str = raw
            .split_once(' ')
            .map(|(_, rest)| rest.trim())
            .unwrap_or("");
        Command::new("msiexec.exe")
            .raw_arg_or_split(args_str)
            .spawn()
            .map_err(|e| format!("Errore avvio msiexec.exe: {e}"))?;
        return Ok(format!("Avviato msiexec.exe {args_str}"));
    }

    // Parse quoted executable `"C:\path\uninst.exe" /arg1 /arg2`
    let (exe_part, args_part) = if let Some(rest) = raw.strip_prefix('"') {
        if let Some(end_q) = rest.find('"') {
            (&rest[..end_q], rest[end_q + 1..].trim())
        } else {
            (raw, "")
        }
    } else if let Some(idx) = raw.to_lowercase().find(".exe") {
        let split_at = idx + 4;
        (raw[..split_at].trim(), raw[split_at..].trim())
    } else {
        (raw, "")
    };

    let mut cmd = Command::new(exe_part);
    if !args_part.is_empty() {
        cmd.raw_arg_or_split(args_part);
    }
    cmd.spawn()
        .map_err(|e| format!("Impossibile avviare '{}': {e}", exe_part))?;

    Ok(format!("Avviato disinstallatore: \"{exe_part}\" {args_part}"))
}

trait RawArgExt {
    fn raw_arg_or_split(&mut self, args_str: &str) -> &mut Self;
}

impl RawArgExt for Command {
    fn raw_arg_or_split(&mut self, args_str: &str) -> &mut Self {
        use std::os::windows::process::CommandExt;
        self.raw_arg(args_str)
    }
}
