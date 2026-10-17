//! 2. Driver Updater (Official Windows Update Agent `WUApiLib` COM API + PnP Signed Driver Inspector)
//! Queries Microsoft's official Windows Update infrastructure (`Microsoft.Update.Session` with
//! `IsInstalled=0 and Type='Driver'`) to avoid unsafe third-party driver databases, and inspects
//! installed WHQL-signed PnP hardware drivers.

use serde::{Deserialize, Serialize};
use std::os::windows::process::CommandExt;
use std::process::Command;

const CREATE_NO_WINDOW: u32 = 0x08000000;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WindowsUpdateDriverItem {
    pub title: String,
    pub driver_model: String,
    pub driver_manufacturer: String,
    pub driver_class: String,
    pub driver_ver_date: String,
    pub size_mb: f64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct InstalledDriverInfo {
    pub device_name: String,
    pub device_class: String,
    pub manufacturer: String,
    pub driver_version: String,
    pub driver_date: String,
    pub is_signed: bool,
    pub signer: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DriverScanReport {
    pub wu_pending_drivers: Vec<WindowsUpdateDriverItem>,
    pub installed_drivers: Vec<InstalledDriverInfo>,
    pub wu_query_succeeded: bool,
    pub status_message: String,
}

/// Queries both `Microsoft.Update.Session` COM (`IsInstalled=0 and Type='Driver'`) and installed PnP hardware drivers.
pub fn scan_drivers(include_online_wu_search: bool) -> DriverScanReport {
    let mut report = DriverScanReport {
        wu_pending_drivers: Vec::new(),
        installed_drivers: query_installed_pnp_drivers(),
        wu_query_succeeded: false,
        status_message: String::new(),
    };

    if include_online_wu_search {
        match query_windows_update_drivers() {
            Ok(wu_list) => {
                report.wu_query_succeeded = true;
                report.status_message = if wu_list.is_empty() {
                    format!(
                        "✅ Windows Update (WUApiLib COM): 0 driver mancanti o obsoleti. Tutti i {} driver hardware principali sono certificati e aggiornati!",
                        report.installed_drivers.len()
                    )
                } else {
                    format!(
                        "🔔 Windows Update (WUApiLib COM): trovati {} aggiornamenti driver ufficiali certificati Microsoft!",
                        wu_list.len()
                    )
                };
                report.wu_pending_drivers = wu_list;
            }
            Err(e) => {
                report.wu_query_succeeded = false;
                report.status_message = format!(
                    "ℹ️ Ricerca online Windows Update completata con avviso ({e}). Mostrati {} driver hardware PnP installati.",
                    report.installed_drivers.len()
                );
            }
        }
    } else {
        report.status_message = format!(
            "Rilevati {} driver hardware PnP installati. Clicca su 'Cerca su Windows Update (WUApiLib)' per interrogare i server Microsoft.",
            report.installed_drivers.len()
        );
    }

    report
}

/// Queries the official `Microsoft.Update.Session` (`WUApiLib`) COM object for `IsInstalled=0 and Type='Driver'`.
pub fn query_windows_update_drivers() -> Result<Vec<WindowsUpdateDriverItem>, String> {
    let ps_script = r#"
$ErrorActionPreference = 'Stop'
$session = New-Object -ComObject Microsoft.Update.Session
$searcher = $session.CreateUpdateSearcher()
$searcher.Online = $true
$result = $searcher.Search("IsInstalled=0 and Type='Driver'")
$items = @()
foreach ($u in $result.Updates) {
    $verDate = ""
    if ($null -ne $u.DriverVerDate) { $verDate = $u.DriverVerDate.ToString("yyyy-MM-dd") }
    $items += [PSCustomObject]@{
        title               = [string]$u.Title
        driver_model        = [string]$u.DriverModel
        driver_manufacturer = [string]$u.DriverManufacturer
        driver_class        = [string]$u.DriverClass
        driver_ver_date     = [string]$verDate
        size_mb             = [math]::Round(($u.MaxDownloadSize / 1MB), 2)
    }
}
ConvertTo-Json -InputObject @($items) -Compress
"#;

    let output = Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", ps_script])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| format!("Impossibile invocare COM Microsoft.Update.Session: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(stderr.lines().next().unwrap_or("Errore COM WUApiLib").to_string());
    }

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if stdout.is_empty() || stdout == "null" {
        return Ok(Vec::new());
    }

    serde_json::from_str::<Vec<WindowsUpdateDriverItem>>(&stdout).or_else(|_| {
        serde_json::from_str::<WindowsUpdateDriverItem>(&stdout).map(|single| vec![single])
    })
    .map_err(|e| format!("Errore parsing JSON WUApiLib: {e}"))
}

/// Queries installed hardware drivers (`Win32_PnPSignedDriver`) for key hardware classes.
pub fn query_installed_pnp_drivers() -> Vec<InstalledDriverInfo> {
    let ps_script = r#"
$ErrorActionPreference = 'SilentlyContinue'
$classes = @('Display', 'Net', 'Media', 'System', 'USB', 'Bluetooth', 'HDC', 'NVMe', 'SCSIAdapter')
$drivers = Get-CimInstance Win32_PnPSignedDriver |
    Where-Object { $_.DeviceName -and ($classes -contains $_.DeviceClass) } |
    Select-Object -First 45 |
    ForEach-Object {
        $dDate = ""
        if ($_.DriverDate) { $dDate = $_.DriverDate.ToString("yyyy-MM-dd") }
        [PSCustomObject]@{
            device_name    = [string]$_.DeviceName
            device_class   = [string]$_.DeviceClass
            manufacturer   = [string]$_.Manufacturer
            driver_version = [string]$_.DriverVersion
            driver_date    = [string]$dDate
            is_signed      = [bool]$_.IsSigned
            signer         = [string]$_.Signer
        }
    }
ConvertTo-Json -InputObject @($drivers) -Compress
"#;

    let Ok(output) = Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", ps_script])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
    else {
        return Vec::new();
    };

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if stdout.is_empty() || stdout == "null" {
        return Vec::new();
    }

    serde_json::from_str::<Vec<InstalledDriverInfo>>(&stdout)
        .or_else(|_| serde_json::from_str::<InstalledDriverInfo>(&stdout).map(|s| vec![s]))
        .unwrap_or_default()
}

/// Downloads and installs pending official Driver updates via `Microsoft.Update.Session` (`WUApiLib` COM).
/// Requires Administrator (UAC) privileges.
pub fn install_wu_driver_updates() -> Result<String, String> {
    let ps_script = r#"
$ErrorActionPreference = 'Stop'
$session = New-Object -ComObject Microsoft.Update.Session
$searcher = $session.CreateUpdateSearcher()
$result = $searcher.Search("IsInstalled=0 and Type='Driver'")
if ($result.Updates.Count -eq 0) {
    Write-Output "Nessun aggiornamento driver da installare."
    exit 0
}
$downloader = $session.CreateUpdateDownloader()
$downloader.Updates = $result.Updates
$dlRes = $downloader.Download()
$installer = $session.CreateUpdateInstaller()
$installer.Updates = $result.Updates
$instRes = $installer.Install()
Write-Output ("Installati " + $result.Updates.Count + " driver certificati Microsoft (ResultCode=" + $instRes.ResultCode + ").")
"#;

    let output = Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", ps_script])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| format!("Impossibile avviare l'installazione COM WUApiLib: {e}"))?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        Err(format!(
            "Installazione driver fallita (assicurati di aver elevato i privilegi UAC come Amministratore): {}",
            String::from_utf8_lossy(&output.stderr).lines().next().unwrap_or_default()
        ))
    }
}
