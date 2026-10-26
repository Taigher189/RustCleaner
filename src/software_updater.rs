//! 1. Software Updater (Winget Wrapper — Zero Proprietary Databases)
//!
//! Executes Microsoft's official `winget` (Windows Package Manager) in the background,
//! parses available software updates, and allows one-click single or batch (`--all --silent`) upgrades.

use serde::{Deserialize, Serialize};
use std::os::windows::process::CommandExt;
use std::process::Command;

const CREATE_NO_WINDOW: u32 = 0x08000000;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WingetPackageUpdate {
    pub name: String,
    pub id: String,
    pub current_version: String,
    pub available_version: String,
    pub source: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WingetScanReport {
    pub winget_available: bool,
    pub winget_version: String,
    pub updates: Vec<WingetPackageUpdate>,
    pub raw_message: String,
}

/// Checks if `winget` is installed and returns its version string (e.g. `v1.9.25200`).
pub fn check_winget_version() -> Option<String> {
    let output = Command::new("winget")
        .arg("--version")
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .ok()?;
    if output.status.success() {
        Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        None
    }
}

/// Runs `winget upgrade --accept-source-agreements` and parses the tabular output into `WingetPackageUpdate` items.
pub fn check_for_updates() -> WingetScanReport {
    let Some(version) = check_winget_version() else {
        return WingetScanReport {
            winget_available: false,
            winget_version: String::new(),
            updates: Vec::new(),
            raw_message: "Winget (Windows Package Manager) non rilevato nel PATH di sistema.".to_string(),
        };
    };

    let output = match Command::new("winget")
        .args(["upgrade", "--accept-source-agreements"])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
    {
        Ok(o) => o,
        Err(e) => {
            return WingetScanReport {
                winget_available: true,
                winget_version: version,
                updates: Vec::new(),
                raw_message: format!("Errore nell'esecuzione di winget upgrade: {e}"),
            }
        }
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    let updates = parse_winget_upgrade_output(&stdout);
    let raw_message = if updates.is_empty() {
        "Tutti i software tracciati da Winget risultano aggiornati!".to_string()
    } else {
        format!(
            "Trovati {} aggiornamenti software disponibili tramite Winget ({version}).",
            updates.len()
        )
    };

    WingetScanReport {
        winget_available: true,
        winget_version: version,
        updates,
        raw_message,
    }
}

/// Alias for `parse_winget_upgrade_output`
pub fn parse_winget_table(stdout: &str) -> Vec<WingetPackageUpdate> {
    parse_winget_upgrade_output(stdout)
}

/// Parses Winget's fixed-width UTF-8 table output (supporting both English and Italian headers).
pub fn parse_winget_upgrade_output(stdout: &str) -> Vec<WingetPackageUpdate> {
    let lines: Vec<&str> = stdout
        .lines()
        .map(|l| l.trim_end_matches('\r'))
        .collect();

    // Find the dashed separator line `--------------------------------...`
    let Some(sep_idx) = lines
        .iter()
        .position(|l| l.trim().len() >= 20 && l.trim().chars().all(|c| c == '-'))
    else {
        return Vec::new();
    };

    if sep_idx == 0 {
        return Vec::new();
    }

    let header_line = lines[sep_idx - 1];
    // Extract column start byte/char indices from the header line by finding transitions from space to non-space
    let mut col_char_starts: Vec<usize> = Vec::new();
    let chars: Vec<char> = header_line.chars().collect();
    let mut in_word = false;
    let mut space_run = 0usize;

    for (i, &ch) in chars.iter().enumerate() {
        if ch != ' ' {
            if !in_word && (i == 0 || space_run >= 1) {
                col_char_starts.push(i);
                in_word = true;
            }
            space_run = 0;
        } else {
            space_run += 1;
            if space_run >= 2 {
                in_word = false;
            }
        }
    }

    let mut updates = Vec::new();

    for &line in &lines[sep_idx + 1..] {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        // Skip footer summary lines like "12 upgrades available." or "12 aggiornamenti disponibili."
        let low = trimmed.to_lowercase();
        if low.contains("upgrades available")
            || low.contains("aggiornamenti disponibili")
            || low.contains("aggiornamento disponibile")
            || low.contains("upgrade available")
            || low.starts_with("the following packages")
            || low.starts_with("i pacchetti seguenti")
            || trimmed.chars().all(|c| c == '-')
        {
            continue;
        }

        // Parse either by column offsets or fallback whitespace splitting from the right
        if let Some(pkg) = parse_single_winget_row(line, &col_char_starts) {
            updates.push(pkg);
        }
    }

    updates
}

fn slice_chars(s: &str, start: usize, end: Option<usize>) -> String {
    let chars: Vec<char> = s.chars().collect();
    if start >= chars.len() {
        return String::new();
    }
    let slice = match end {
        Some(e) if e < chars.len() => &chars[start..e],
        _ => &chars[start..],
    };
    slice.iter().collect::<String>().trim().to_string()
}

fn parse_single_winget_row(line: &str, col_starts: &[usize]) -> Option<WingetPackageUpdate> {
    if col_starts.len() >= 4 {
        let c0 = col_starts[0];
        let c1 = col_starts[1];
        let c2 = col_starts[2];
        let c3 = col_starts[3];
        let c4 = col_starts.get(4).copied();

        let name = slice_chars(line, c0, Some(c1));
        let id = slice_chars(line, c1, Some(c2));
        let current_version = slice_chars(line, c2, Some(c3));
        let (available_version, source) = if let Some(c4_idx) = c4 {
            (
                slice_chars(line, c3, Some(c4_idx)),
                slice_chars(line, c4_idx, None),
            )
        } else {
            let rem = slice_chars(line, c3, None);
            let mut parts = rem.split_whitespace();
            let avail = parts.next().unwrap_or_default().to_string();
            let src = parts.next().unwrap_or("winget").to_string();
            (avail, src)
        };

        if !name.is_empty() && !id.is_empty() && !available_version.is_empty() && id.contains('.') {
            return Some(WingetPackageUpdate {
                name,
                id,
                current_version,
                available_version,
                source: if source.is_empty() {
                    "winget".to_string()
                } else {
                    source
                },
            });
        }
    }

    // Fallback: split tokens from the right (Source, Available, Version, Id, Name...)
    let tokens: Vec<&str> = line.split_whitespace().collect();
    if tokens.len() >= 5 {
        let last = tokens[tokens.len() - 1];
        let has_source = last.eq_ignore_ascii_case("winget") || last.eq_ignore_ascii_case("msstore");
        let (source, avail_idx) = if has_source {
            (last.to_string(), tokens.len() - 2)
        } else {
            ("winget".to_string(), tokens.len() - 1)
        };
        if avail_idx >= 3 {
            let available_version = tokens[avail_idx].to_string();
            let current_version = tokens[avail_idx - 1].to_string();
            let id = tokens[avail_idx - 2].to_string();
            let name = tokens[..avail_idx - 2].join(" ");
            if id.contains('.') && !name.is_empty() {
                return Some(WingetPackageUpdate {
                    name,
                    id,
                    current_version,
                    available_version,
                    source,
                });
            }
        }
    }

    None
}

/// Upgrades a single package via `winget upgrade --id <id> --silent`.
pub fn upgrade_single_package(package_id: &str) -> Result<String, String> {
    let output = Command::new("winget")
        .args([
            "upgrade",
            "--id",
            package_id,
            "--silent",
            "--accept-package-agreements",
            "--accept-source-agreements",
        ])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| format!("Impossibile avviare winget per '{package_id}': {e}"))?;

    if output.status.success() {
        Ok(format!(
            "✅ Pacchetto '{package_id}' aggiornato con successo tramite Winget!"
        ))
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        Err(format!(
            "⚠️ Winget ha restituito codice {:?} per '{package_id}': {} {}",
            output.status.code(),
            stdout.lines().last().unwrap_or_default(),
            stderr.trim()
        ))
    }
}

/// Upgrades all packages via `winget upgrade --all --silent`.
pub fn upgrade_all_packages() -> Result<String, String> {
    let output = Command::new("winget")
        .args([
            "upgrade",
            "--all",
            "--silent",
            "--accept-package-agreements",
            "--accept-source-agreements",
        ])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| format!("Impossibile avviare winget upgrade --all: {e}"))?;

    if output.status.success() {
        Ok("✅ Tutti i software disponibili sono stati aggiornati con successo (winget upgrade --all --silent)!".to_string())
    } else {
        Ok(format!(
            "ℹ️ Operazione winget upgrade --all terminata (codice {:?}). Alcuni pacchetti potrebbero richiedere il riavvio o la chiusura dell'app.",
            output.status.code()
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_winget_table_english_and_italian() {
        let sample = "\
Nome                             Id                          Versione     Disponibile  Origine
----------------------------------------------------------------------------------------------
Mozilla Firefox (x64 it)         Mozilla.Firefox.it          130.0.1      131.0.3      winget
Microsoft Visual Studio Code     Microsoft.VisualStudioCode  1.93.1       1.94.2       winget
2 aggiornamenti disponibili.
";
        let parsed = parse_winget_upgrade_output(sample);
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].id, "Mozilla.Firefox.it");
        assert_eq!(parsed[0].current_version, "130.0.1");
        assert_eq!(parsed[0].available_version, "131.0.3");
        assert_eq!(parsed[1].id, "Microsoft.VisualStudioCode");
    }
}
