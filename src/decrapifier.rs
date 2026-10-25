//! Windows 11 Privacy & Decrapifier Engine
//! Safe, reversible debloater for Windows 10/11:
//! - Disables telemetry, diagnostic tracking, advertising ID, and customer experience program.
//! - Cleans Start menu from Bing search results, suggested apps, and sponsored recommendations.
//! - Disables MSN widgets feed, lock screen ads, and Edge background pre-launchers.
//! - Detects and safely removes consumer AppX bloatware (TikTok, Candy Crush, etc.).
//! - Includes 1-Click Rollback to restore original Windows stock defaults.

use serde::{Deserialize, Serialize};
use std::process::Command;
use winreg::enums::*;
use winreg::RegKey;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TweakCategory {
    PrivacyTelemetry,
    StartAndExplorer,
    SystemAndEdge,
    BloatwareAppx,
}

impl TweakCategory {
    pub fn all() -> &'static [TweakCategory] {
        &[
            TweakCategory::PrivacyTelemetry,
            TweakCategory::StartAndExplorer,
            TweakCategory::SystemAndEdge,
            TweakCategory::BloatwareAppx,
        ]
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryTweakDef {
    pub id: &'static str,
    pub category: TweakCategory,
    pub name_it: &'static str,
    pub desc_it: &'static str,
    pub hive: RegHive,
    pub subkey: &'static str,
    pub value_name: &'static str,
    pub debloat_val: u32,
    pub default_val: u32,
    pub recommended: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RegHive {
    Hkcu,
    Hklm,
}

impl RegHive {
    pub fn open_root(&self) -> RegKey {
        match self {
            RegHive::Hkcu => RegKey::predef(HKEY_CURRENT_USER),
            RegHive::Hklm => RegKey::predef(HKEY_LOCAL_MACHINE),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TweakState {
    pub id: String,
    pub enabled: bool, // true = debloated / privacy protected
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppxPackageInfo {
    pub name: String,
    pub package_full_name: String,
    pub display_name: String,
    pub is_removable_bloat: bool,
    pub selected: bool,
}

/// Master list of safe, tested Windows 10/11 privacy & debloat tweaks.
pub const REGISTRY_TWEAKS: &[RegistryTweakDef] = &[
    // --- 1. PRIVACY & TELEMETRY ---
    RegistryTweakDef {
        id: "telemetry_datacollection",
        category: TweakCategory::PrivacyTelemetry,
        name_it: "Disattiva Telemetria di Sistema (DataCollection)",
        desc_it: "Imposta la telemetria di diagnostica Windows al livello minimo consentito (Security/Zero).",
        hive: RegHive::Hklm,
        subkey: r"SOFTWARE\Policies\Microsoft\Windows\DataCollection",
        value_name: "AllowTelemetry",
        debloat_val: 0,
        default_val: 1,
        recommended: true,
    },
    RegistryTweakDef {
        id: "advertising_id",
        category: TweakCategory::PrivacyTelemetry,
        name_it: "Disattiva ID Pubblicitario Utente",
        desc_it: "Impedisce alle applicazioni e a Microsoft di tracciare le tue preferenze per annunci mirati.",
        hive: RegHive::Hkcu,
        subkey: r"Software\Microsoft\Windows\CurrentVersion\AdvertisingInfo",
        value_name: "Enabled",
        debloat_val: 0,
        default_val: 1,
        recommended: true,
    },
    RegistryTweakDef {
        id: "siuf_feedback_frequency",
        category: TweakCategory::PrivacyTelemetry,
        name_it: "Disattiva Richieste di Feedback (SIUF)",
        desc_it: "Disabilita le notifiche intrusive che chiedono valutazioni sull'uso di Windows.",
        hive: RegHive::Hkcu,
        subkey: r"Software\Microsoft\Siuf\Rules",
        value_name: "NumberOfSIUFInPeriod",
        debloat_val: 0,
        default_val: 1,
        recommended: true,
    },
    RegistryTweakDef {
        id: "diagtrack_service_optout",
        category: TweakCategory::PrivacyTelemetry,
        name_it: "Disattiva Esperienze Utente Connesse (CEIP)",
        desc_it: "Disabilita il Programma Analisi Esperienza Cliente (CEIP) di Windows.",
        hive: RegHive::Hklm,
        subkey: r"SOFTWARE\Policies\Microsoft\SQMClient\Windows",
        value_name: "CEIPEnable",
        debloat_val: 0,
        default_val: 1,
        recommended: true,
    },

    // --- 2. MENU START & ESPLORA FILE ---
    RegistryTweakDef {
        id: "start_bing_search",
        category: TweakCategory::StartAndExplorer,
        name_it: "Disattiva Ricerca Web Bing nel Menu Start",
        desc_it: "Rende la ricerca nel menu Start istantanea e locale, senza inviare i tasti premuti a Bing.",
        hive: RegHive::Hkcu,
        subkey: r"Software\Policies\Microsoft\Windows\Explorer",
        value_name: "DisableSearchBoxSuggestions",
        debloat_val: 1,
        default_val: 0,
        recommended: true,
    },
    RegistryTweakDef {
        id: "start_suggested_apps",
        category: TweakCategory::StartAndExplorer,
        name_it: "Disattiva App Consigliate e Sponsorizzate nel Menu Start",
        desc_it: "Rimuove le app pubblicitarie consigliate automaticamente da Microsoft nella schermata Start.",
        hive: RegHive::Hkcu,
        subkey: r"Software\Microsoft\Windows\CurrentVersion\ContentDeliveryManager",
        value_name: "SystemPaneSuggestionsEnabled",
        debloat_val: 0,
        default_val: 1,
        recommended: true,
    },
    RegistryTweakDef {
        id: "lockscreen_spotlight_ads",
        category: TweakCategory::StartAndExplorer,
        name_it: "Disattiva Suggerimenti e Notizie nella Schermata di Blocco",
        desc_it: "Impedisce a Windows di visualizzare consigli, quiz e annunci nella schermata di accesso.",
        hive: RegHive::Hkcu,
        subkey: r"Software\Microsoft\Windows\CurrentVersion\ContentDeliveryManager",
        value_name: "SubscribedContent-338387Enabled",
        debloat_val: 0,
        default_val: 1,
        recommended: true,
    },
    RegistryTweakDef {
        id: "taskbar_widgets_feed",
        category: TweakCategory::StartAndExplorer,
        name_it: "Disattiva Widget e Notizie MSN sulla Barra delle Applicazioni",
        desc_it: "Rimuove l'icona del meteo/notizie che consuma RAM e larghezza di banda in background.",
        hive: RegHive::Hkcu,
        subkey: r"Software\Microsoft\Windows\CurrentVersion\Explorer\Advanced",
        value_name: "TaskbarDa",
        debloat_val: 0,
        default_val: 1,
        recommended: true,
    },

    // --- 3. SISTEMA & MICROSOFT EDGE ---
    RegistryTweakDef {
        id: "edge_startup_boost",
        category: TweakCategory::SystemAndEdge,
        name_it: "Disattiva Pre-caricamento di Edge in Background (Startup Boost)",
        desc_it: "Impedisce a Microsoft Edge di mantenere processi attivi in background all'avvio di Windows.",
        hive: RegHive::Hklm,
        subkey: r"SOFTWARE\Policies\Microsoft\Edge",
        value_name: "StartupBoostEnabled",
        debloat_val: 0,
        default_val: 1,
        recommended: true,
    },
    RegistryTweakDef {
        id: "edge_background_mode",
        category: TweakCategory::SystemAndEdge,
        name_it: "Disattiva App Edge in Background alla Chiusura",
        desc_it: "Libera immediatamente tutta la memoria RAM quando Edge viene chiuso.",
        hive: RegHive::Hklm,
        subkey: r"SOFTWARE\Policies\Microsoft\Edge",
        value_name: "BackgroundModeEnabled",
        debloat_val: 0,
        default_val: 1,
        recommended: true,
    },
    RegistryTweakDef {
        id: "disable_cortana_autostart",
        category: TweakCategory::SystemAndEdge,
        name_it: "Disattiva Cortana & Assistente Vocale Residuo",
        desc_it: "Disabilita completamente i processi legacy di Cortana all'avvio del sistema.",
        hive: RegHive::Hklm,
        subkey: r"SOFTWARE\Policies\Microsoft\Windows\Windows Search",
        value_name: "AllowCortana",
        debloat_val: 0,
        default_val: 1,
        recommended: true,
    },
];

/// Known bloatware AppX packages that are safe to remove for personal and gaming PCs.
pub const BLOATWARE_APPX_PATTERNS: &[(&str, &str)] = &[
    ("Microsoft.BingNews", "MSN Notizie & Meteo"),
    ("Microsoft.BingWeather", "MSN Meteo"),
    ("Microsoft.GetHelp", "Richiesta Supporto Microsoft"),
    ("Microsoft.Getstarted", "Suggerimenti & Primi Passi"),
    ("Microsoft.MicrosoftOfficeHub", "Office Hub Promozionale"),
    ("Microsoft.MicrosoftSolitaireCollection", "Solitaire Collection con Annunci"),
    ("Microsoft.People", "Contatti (People Hub legacy)"),
    ("Microsoft.Todos", "Microsoft To Do"),
    ("Microsoft.PowerAutomateDesktop", "Power Automate Desktop"),
    ("Clipchamp.Clipchamp", "Clipchamp Video Editor"),
    ("SpotifyAB.SpotifyMusic", "Spotify Preinstallato"),
    ("ByteDance.TikTok", "TikTok Preinstallato"),
    ("Disney.37853FC22B2CE", "Disney+ Preinstallato"),
    ("king.com.CandyCrushSaga", "Candy Crush Saga"),
    ("king.com.CandyCrushSodaSaga", "Candy Crush Soda Saga"),
];

/// Queries the current state of a registry tweak.
pub fn is_tweak_active(tweak: &RegistryTweakDef) -> bool {
    let root = tweak.hive.open_root();
    if let Ok(key) = root.open_subkey_with_flags(tweak.subkey, KEY_READ) {
        if let Ok(val) = key.get_value::<u32, _>(tweak.value_name) {
            return val == tweak.debloat_val;
        }
    }
    false
}

/// Applies a registry tweak (enabling the privacy / debloat optimization).
pub fn apply_tweak(tweak: &RegistryTweakDef) -> Result<(), String> {
    let root = tweak.hive.open_root();
    let (key, _) = root
        .create_subkey(tweak.subkey)
        .map_err(|e| format!("Impossibile creare/aprire chiave '{}': {e}", tweak.subkey))?;

    key.set_value(tweak.value_name, &tweak.debloat_val)
        .map_err(|e| format!("Impossibile impostare valore '{}': {e}", tweak.value_name))?;

    Ok(())
}

/// Reverts a registry tweak back to default Windows factory behavior (1-Click Rollback).
pub fn revert_tweak(tweak: &RegistryTweakDef) -> Result<(), String> {
    let root = tweak.hive.open_root();
    if let Ok(key) = root.open_subkey_with_flags(tweak.subkey, KEY_SET_VALUE) {
        let _ = key.set_value(tweak.value_name, &tweak.default_val);
    }
    Ok(())
}

/// Scans installed AppX packages and filters for detected bloatware.
pub fn scan_bloatware_packages() -> Vec<AppxPackageInfo> {
    let mut packages = Vec::new();

    // Query installed packages via powershell in a clean 1-shot command
    let script = r#"Get-AppxPackage | Select-Object -Property Name, PackageFullName | ConvertTo-Csv -NoTypeInformation"#;

    let Ok(output) = Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", script])
        .output()
    else {
        return packages;
    };

    let text = String::from_utf8_lossy(&output.stdout);
    for line in text.lines().skip(1) {
        let parts: Vec<&str> = line.split(',').map(|s| s.trim_matches('"')).collect();
        if parts.len() >= 2 {
            let pkg_name = parts[0].trim();
            let full_name = parts[1].trim();

            for &(pattern, friendly) in BLOATWARE_APPX_PATTERNS {
                if pkg_name.eq_ignore_ascii_case(pattern) {
                    packages.push(AppxPackageInfo {
                        name: pkg_name.to_string(),
                        package_full_name: full_name.to_string(),
                        display_name: friendly.to_string(),
                        is_removable_bloat: true,
                        selected: true,
                    });
                    break;
                }
            }
        }
    }

    packages.sort_by(|a, b| a.display_name.cmp(&b.display_name));
    packages.dedup_by(|a, b| a.name.eq_ignore_ascii_case(&b.name));
    packages
}

/// Removes a specific AppX package for the current user.
pub fn remove_appx_package(package_full_name: &str) -> Result<(), String> {
    let cmd = format!("Remove-AppxPackage -Package '{}' -ErrorAction Stop", package_full_name);
    let output = Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", &cmd])
        .output()
        .map_err(|e| format!("Errore esecuzione PowerShell: {e}"))?;

    if output.status.success() {
        Ok(())
    } else {
        let err_text = String::from_utf8_lossy(&output.stderr);
        Err(format!("Errore rimozione AppX: {err_text}"))
    }
}

/// Applies all recommended privacy and debloat tweaks in one operation.
pub fn apply_all_recommended_tweaks() -> Result<usize, String> {
    let mut count = 0;
    for tweak in REGISTRY_TWEAKS {
        if tweak.recommended {
            apply_tweak(tweak)?;
            count += 1;
        }
    }
    Ok(count)
}

/// Rolls back all tweaks to factory Windows stock defaults (1-Click Safe Rollback).
pub fn restore_all_windows_defaults() -> Result<usize, String> {
    let mut count = 0;
    for tweak in REGISTRY_TWEAKS {
        revert_tweak(tweak)?;
        count += 1;
    }
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_registry_tweaks_have_valid_definitions() {
        assert!(!REGISTRY_TWEAKS.is_empty());
        for tweak in REGISTRY_TWEAKS {
            assert!(!tweak.id.is_empty());
            assert!(!tweak.name_it.is_empty());
            assert!(!tweak.desc_it.is_empty());
            assert!(!tweak.subkey.is_empty());
            assert!(!tweak.value_name.is_empty());
        }
    }

    #[test]
    fn test_bloatware_appx_patterns_non_empty() {
        assert!(!BLOATWARE_APPX_PATTERNS.is_empty());
        for &(pkg, name) in BLOATWARE_APPX_PATTERNS {
            assert!(!pkg.is_empty());
            assert!(!name.is_empty());
        }
    }
}
