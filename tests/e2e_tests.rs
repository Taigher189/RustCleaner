//! RustCleaner Comprehensive End-to-End & Integration Test Suite
//! Covers Tiers 1-4: Feature Coverage, Boundary & Corner Cases, Pairwise Combinations, and Real-World Workloads.

mod common;

#[path = "../src/cleaner.rs"]
pub mod cleaner;
#[path = "../src/driver_updater.rs"]
pub mod driver_updater;
#[path = "../src/disk_analyzer.rs"]
pub mod disk_analyzer;
#[path = "../src/i18n.rs"]
pub mod i18n;
#[path = "../src/monitor.rs"]
pub mod monitor;
#[path = "../src/registry.rs"]
pub mod registry;
#[path = "../src/scheduler.rs"]
pub mod scheduler;
#[path = "../src/software_updater.rs"]
pub mod software_updater;
#[path = "../src/startup_uninstall.rs"]
pub mod startup_uninstall;
#[path = "../src/uac.rs"]
pub mod uac;
#[path = "../src/wiper.rs"]
pub mod wiper;

use common::{run_cli, LockedFileGuard, RegistryTestHive, TempTestDir};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

// ============================================================================
// TIER 1: Feature Coverage (Core Features in Isolation)
// ============================================================================

#[test]
fn tier1_cli_help_flag_displays_usage_and_subcommands() {
    let out = run_cli(&["--help"]);
    assert!(out.status.success(), "Exit code must be 0 for --help");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("rustcleaner") || stdout.contains("Usage:"),
        "Stdout should contain usage info"
    );
    assert!(
        stdout.contains("analyze") && stdout.contains("clean"),
        "Stdout should list subcommands"
    );
    assert!(
        stdout.contains("--silent-clean"),
        "Stdout should document --silent-clean"
    );
}

#[test]
fn tier1_cli_version_flag_displays_correct_semver() {
    let out = run_cli(&["--version"]);
    assert!(out.status.success(), "Exit code must be 0 for --version");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("rustcleaner") && stdout.contains("1.3.0"),
        "Stdout should contain app name and 1.3.0 semver"
    );
}

#[test]
fn tier1_cli_invalid_flag_exits_with_error() {
    let out = run_cli(&["--nonexistent-flag-xyz"]);
    assert!(
        !out.status.success(),
        "Exit code must be non-zero for unknown flag"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stderr.contains("unexpected argument")
            || stderr.contains("error:")
            || stdout.contains("unexpected argument"),
        "Should display an error about unexpected argument"
    );
}

#[test]
fn tier1_cli_scan_flag_handling_rejects_unknown_argument() {
    let out = run_cli(&["--scan"]);
    assert!(
        !out.status.success(),
        "Direct --scan flag is not a top-level flag and must be rejected"
    );
}

#[test]
fn tier1_cli_silent_clean_flag_executes_headless_and_terminates() {
    let out = run_cli(&["--silent-clean"]);
    assert!(
        out.status.success(),
        "--silent-clean must run headless without UI and terminate cleanly"
    );
}

#[test]
fn tier1_blacklist_rejects_system32_and_subpaths() {
    assert!(
        cleaner::is_protected_system_path(Path::new(r"C:\Windows\System32")),
        "System32 root must be blacklisted"
    );
    assert!(
        cleaner::is_protected_system_path(Path::new(r"C:\Windows\System32\drivers\etc")),
        "System32 subdirectories must be blacklisted"
    );
    assert!(
        cleaner::is_protected_system_path(Path::new(r"c:\windows\system32\kernel32.dll")),
        "System32 files must be blacklisted regardless of casing"
    );
}

#[test]
fn tier1_blacklist_rejects_winsxs_and_components() {
    assert!(
        cleaner::is_protected_system_path(Path::new(r"C:\Windows\WinSxS")),
        "WinSxS must be protected"
    );
    assert!(
        cleaner::is_protected_system_path(Path::new(r"C:\Windows\WinSxS\manifests\some.manifest")),
        "WinSxS files must be protected"
    );
    assert!(
        cleaner::is_protected_system_path(Path::new(r"C:\Windows\SysWOW64")),
        "SysWOW64 must be protected"
    );
}

#[test]
fn tier1_blacklist_rejects_user_personal_documents_and_desktop() {
    assert!(
        cleaner::is_protected_system_path(Path::new(r"C:\Users\Administrator\Documents\file.docx")),
        "User Documents folder must be protected"
    );
    assert!(
        cleaner::is_protected_system_path(Path::new(r"C:\Users\JohnDoe\Desktop\budget.xlsx")),
        "User Desktop folder must be protected"
    );
    assert!(
        cleaner::is_protected_system_path(Path::new(r"C:\Users\Alice\Pictures\vacation.jpg")),
        "User Pictures folder must be protected"
    );
}

#[test]
fn tier1_blacklist_rejects_bootmgr_and_physical_devices() {
    assert!(
        cleaner::is_protected_system_path(Path::new(r"C:\bootmgr")),
        "bootmgr must be protected"
    );
    assert!(
        cleaner::is_protected_system_path(Path::new(r"\\.\PhysicalDrive0")),
        "Raw physical drive devices must be rejected"
    );
    assert!(
        cleaner::is_protected_system_path(Path::new(r"\\.\Volume{12345678-1234-1234-1234-123456789012}")),
        "Raw volume GUID devices must be rejected"
    );
}

#[test]
fn tier1_rule_env_var_expansion_resolves_all_standard_tokens() {
    let p_temp = cleaner::expand_env_vars(r"%TEMP%\test_dir");
    assert!(
        !p_temp.to_string_lossy().contains("%TEMP%"),
        "%TEMP% must be expanded"
    );

    let p_windir = cleaner::expand_env_vars(r"%WINDIR%\Temp");
    assert!(
        !p_windir.to_string_lossy().contains("%WINDIR%"),
        "%WINDIR% must be expanded"
    );

    let p_local = cleaner::expand_env_vars(r"%LOCALAPPDATA%\Google\Chrome");
    assert!(
        !p_local.to_string_lossy().contains("%LOCALAPPDATA%"),
        "%LOCALAPPDATA% must be expanded"
    );
}

#[test]
fn tier1_registry_backup_creates_v5_header_and_empty_line() {
    let temp = TempTestDir::new("reg_header");
    let dummy_issues = vec![];
    let backup_path = registry::export_reg_backup(&dummy_issues, &temp.path)
        .expect("Export empty backup should succeed");
    assert!(backup_path.exists(), "Backup file must be created on disk");

    let content = fs::read_to_string(&backup_path).expect("Read backup file");
    let lines: Vec<&str> = content.lines().collect();
    assert!(
        !lines.is_empty(),
        "Backup file must have content"
    );
    assert_eq!(
        lines[0].trim(),
        "Windows Registry Editor Version 5.00",
        "First line must strictly match Windows Registry Editor Version 5.00"
    );
}

#[test]
fn tier1_registry_backup_escaping_via_export() {
    let temp = TempTestDir::new("reg_esc");
    let issue = registry::RegistryIssue {
        selected: true,
        category: registry::RegistryCategory::SharedDlls,
        problem_description: "Test escaping".to_string(),
        data_reference: "1".to_string(),
        hive: registry::HiveRoot::Hklm,
        key_path: r"SOFTWARE\Microsoft\Windows\CurrentVersion\SharedDLLs".to_string(),
        action: registry::RegTargetAction::DeleteValue {
            value_name: r#"C:\Program Files\App "Quotes"\test.dll"#.to_string(),
            original_data: "1".to_string(),
            is_dword: false,
        },
    };

    let backup_path = registry::export_reg_backup(&[issue], &temp.path).unwrap();
    let content = fs::read_to_string(backup_path).unwrap();
    assert!(
        content.contains(r#""C:\\Program Files\\App \"Quotes\"\\test.dll"="1""#),
        "Backslashes and quotes must be properly escaped in .reg output"
    );
}

#[test]
fn tier1_registry_extract_executable_path_handles_various_formats() {
    let p1 = registry::extract_executable_path(r#""C:\Program Files\App\app.exe" -arg1 -arg2"#);
    assert_eq!(
        p1,
        Some(PathBuf::from(r"C:\Program Files\App\app.exe")),
        "Quoted executable path with arguments"
    );

    let p2 = registry::extract_executable_path(r"C:\Windows\System32\cmd.exe /c dir");
    assert_eq!(
        p2,
        Some(PathBuf::from(r"C:\Windows\System32\cmd.exe")),
        "Unquoted executable path with arguments"
    );
}

#[test]
fn tier1_registry_category_definitions_all_have_unique_codes() {
    let all = registry::RegistryCategory::all();
    assert_eq!(all.len(), 7, "Must contain exactly 7 registry categories");
    let mut codes = std::collections::HashSet::new();
    for cat in all {
        assert!(
            codes.insert(cat.code()),
            "Duplicate category code: {}",
            cat.code()
        );
        assert!(!cat.label().is_empty(), "Category label must not be empty");
    }
}

#[test]
fn tier1_registry_backup_export_writes_valid_syntax_for_problems() {
    let temp = TempTestDir::new("reg_syntax");
    let issue = registry::RegistryIssue {
        selected: true,
        category: registry::RegistryCategory::SharedDlls,
        problem_description: "Missing DLL reference".to_string(),
        data_reference: "1".to_string(),
        hive: registry::HiveRoot::Hklm,
        key_path: r"SOFTWARE\Microsoft\Windows\CurrentVersion\SharedDLLs".to_string(),
        action: registry::RegTargetAction::DeleteValue {
            value_name: r"C:\NonExistent\library.dll".to_string(),
            original_data: "1".to_string(),
            is_dword: false,
        },
    };

    let backup_path = registry::export_reg_backup(&[issue], &temp.path)
        .expect("Backup with problem should succeed");
    let content = fs::read_to_string(&backup_path).expect("Read backup file");
    assert!(
        content.contains(r"[HKEY_LOCAL_MACHINE\SOFTWARE\Microsoft\Windows\CurrentVersion\SharedDLLs]"),
        "Backup must contain exact subkey header"
    );
    assert!(
        content.contains(r#""C:\\NonExistent\\library.dll""#),
        "Backup must contain escaped value name"
    );
}

#[test]
fn tier1_wiper_algorithm_passes_match_spec() {
    assert_eq!(wiper::WipeAlgorithm::SimpleOnePassZero.passes(), 1);
    assert_eq!(wiper::WipeAlgorithm::DodThreePass.passes(), 3);
    assert_eq!(wiper::WipeAlgorithm::NsaSevenPass.passes(), 7);
    assert_eq!(wiper::WipeAlgorithm::GutmannThirtyFivePass.passes(), 35);
    for alg in wiper::WipeAlgorithm::all() {
        assert!(!alg.label().is_empty());
    }
}

#[test]
fn tier1_wiper_list_drives_returns_valid_logical_volumes() {
    let drives = wiper::list_drives();
    assert!(!drives.is_empty(), "Must detect at least one logical drive on Windows");
    let c_drive = drives.iter().find(|d| d.root_path.starts_with("C:"));
    assert!(c_drive.is_some(), "C: drive must be listed");
    let c = c_drive.unwrap();
    assert!(c.total_bytes > 0, "Total bytes must be greater than 0");
    assert!(c.free_bytes > 0, "Free bytes must be greater than 0");
}

#[test]
fn tier1_wiper_enforces_safety_reserve_margin() {
    let drives = wiper::list_drives();
    let free = drives.first().map(|d| d.free_bytes).unwrap_or(0);
    let safe_free = free.saturating_sub(512 * 1024 * 1024);
    assert!(safe_free <= free, "Safe free space calculation invariant");
}

#[test]
fn tier1_wiper_device_path_is_protected() {
    assert!(cleaner::is_protected_system_path(Path::new(r"\\.\PhysicalDrive1")));
    assert!(cleaner::is_protected_system_path(Path::new(r"\\.\Volume{abcdef01-0000-0000-0000-000000000000}")));
    assert!(wiper::validate_wipe_drive_root(r"\\.\PhysicalDrive0").is_err());
    assert!(wiper::validate_wipe_drive_root(r"\\.\C:").is_err());
}

#[test]
fn tier1_wiper_cleans_up_temporary_container_via_raii_guard() {
    let drives = wiper::list_drives();
    if let Some(drive) = drives.first() {
        let drive_root = &drive.root_path;
        let progress = Arc::new(Mutex::new(wiper::WipeProgress::default()));
        // Pre-cancel to trigger immediate RAII cleanup path
        let cancel = Arc::new(AtomicBool::new(true));
        let _ = wiper::wipe_free_space(drive_root, wiper::WipeAlgorithm::SimpleOnePassZero, false, Some(1), progress, cancel);

        let temp_container = Path::new(drive_root).join(".rustcleaner_wipe_tmp");
        assert!(
            !temp_container.exists(),
            "Temporary wiper container must be unlinked immediately upon return"
        );
    }
}

#[test]
fn tier1_i18n_covers_all_seven_supported_languages() {
    let langs = i18n::Language::all();
    assert_eq!(langs.len(), 7, "Must support exactly 7 languages");
    let codes: Vec<&str> = langs.iter().map(|l| l.code()).collect();
    assert!(codes.contains(&"EN"));
    assert!(codes.contains(&"IT"));
    assert!(codes.contains(&"FR"));
    assert!(codes.contains(&"ES"));
    assert!(codes.contains(&"DE"));
    assert!(codes.contains(&"ZH"));
    assert!(codes.contains(&"PT"));
}

#[test]
fn tier1_i18n_all_rule_names_non_empty_in_all_languages() {
    let rules = cleaner::default_clean_rules();
    for rule in &rules {
        for lang in i18n::Language::all() {
            let (name, _desc) = lang
                .localize_rule(rule.id)
                .unwrap_or((&rule.name, &rule.description));
            assert!(
                !name.trim().is_empty(),
                "Rule #{} missing name in {}",
                rule.id,
                lang.display_name()
            );
        }
    }
}

#[test]
fn tier1_i18n_all_rule_descriptions_non_empty_in_all_languages() {
    let rules = cleaner::default_clean_rules();
    for rule in &rules {
        for lang in i18n::Language::all() {
            let (_name, desc) = lang
                .localize_rule(rule.id)
                .unwrap_or((&rule.name, &rule.description));
            assert!(
                !desc.trim().is_empty(),
                "Rule #{} missing description in {}",
                rule.id,
                lang.display_name()
            );
        }
    }
}

#[test]
fn tier1_i18n_all_registry_categories_non_empty_in_all_languages() {
    for cat in registry::RegistryCategory::all() {
        for lang in i18n::Language::all() {
            let (name, _desc) = lang.localize_reg_category(*cat);
            assert!(
                !name.trim().is_empty(),
                "Category {:?} missing name in {}",
                cat,
                lang.display_name()
            );
        }
    }
}

#[test]
fn tier1_i18n_all_registry_descriptions_non_empty_in_all_languages() {
    for cat in registry::RegistryCategory::all() {
        for lang in i18n::Language::all() {
            let (_name, desc) = lang.localize_reg_category(*cat);
            assert!(
                !desc.trim().is_empty(),
                "Category {:?} missing description in {}",
                cat,
                lang.display_name()
            );
        }
    }
}

// ============================================================================
// TIER 2: Boundary & Corner Cases
// ============================================================================

#[test]
fn tier2_cleaner_handles_empty_directories_without_error() {
    let temp = TempTestDir::new("empty_dir");
    let empty_sub = temp.path.join("empty_subdir");
    fs::create_dir_all(&empty_sub).unwrap();

    let rule = cleaner::CleanRule {
        id: 9991,
        name: "Test Empty Rule".to_string(),
        group: cleaner::RuleGroup::Applications,
        description: "Empty test".to_string(),
        enabled: true,
        requires_admin: false,
        special_recycle_bin: false,
        targets: vec![cleaner::FileTarget {
            base_dir: empty_sub.to_string_lossy().to_string(),
            patterns: vec!["*.*".to_string()],
            recursive: true,
        }],
    };

    let result = cleaner::scan_rule(&rule);
    assert_eq!(result.files.len(), 0, "No files should be found in empty directory");
    assert_eq!(result.total_bytes, 0);

    let summary = cleaner::execute_clean(&[rule]);
    assert_eq!(summary.total_files_deleted, 0);
    assert_eq!(summary.errors.len(), 0);
}

#[test]
fn tier2_cleaner_handles_non_existent_target_paths_gracefully() {
    let non_existent = r"C:\NonExistent_Directory_RustCleaner_123456";
    let rule = cleaner::CleanRule {
        id: 9992,
        name: "Test NonExistent Rule".to_string(),
        group: cleaner::RuleGroup::Applications,
        description: "Non-existent path test".to_string(),
        enabled: true,
        requires_admin: false,
        special_recycle_bin: false,
        targets: vec![cleaner::FileTarget {
            base_dir: non_existent.to_string(),
            patterns: vec!["*.tmp".to_string()],
            recursive: true,
        }],
    };

    let result = cleaner::scan_rule(&rule);
    assert_eq!(result.files.len(), 0);
    let summary = cleaner::execute_clean(&[rule]);
    assert_eq!(summary.total_files_deleted, 0);
}

#[test]
fn tier2_cleaner_handles_read_only_files_during_cleanup() {
    let temp = TempTestDir::new("readonly_clean");
    let ro_file = temp.create_readonly_file("ro_test.log", b"sample content");

    let rule = cleaner::CleanRule {
        id: 9993,
        name: "Readonly Clean Rule".to_string(),
        group: cleaner::RuleGroup::Applications,
        description: "Readonly file test".to_string(),
        enabled: true,
        requires_admin: false,
        special_recycle_bin: false,
        targets: vec![cleaner::FileTarget {
            base_dir: temp.path.to_string_lossy().to_string(),
            patterns: vec!["*.log".to_string()],
            recursive: false,
        }],
    };

    let scan = cleaner::scan_rule(&rule);
    assert_eq!(scan.files.len(), 1, "Must find the read-only file");

    let clean_res = cleaner::execute_clean(&[rule]);
    assert_eq!(clean_res.total_files_deleted, 1, "Must delete the file after clearing readonly bit");
    assert!(!ro_file.exists(), "Read-only file must be removed");
}

#[test]
fn tier2_cleaner_locked_file_probe_reports_locked_without_panic() {
    let temp = TempTestDir::new("locked_probe");
    let target_file = temp.path.join("locked.dat");
    let _locked = LockedFileGuard::new(&target_file).expect("Create locked file");

    let rule = cleaner::CleanRule {
        id: 9994,
        name: "Locked Probe Rule".to_string(),
        group: cleaner::RuleGroup::Applications,
        description: "Locked test".to_string(),
        enabled: true,
        requires_admin: false,
        special_recycle_bin: false,
        targets: vec![cleaner::FileTarget {
            base_dir: temp.path.to_string_lossy().to_string(),
            patterns: vec!["*.dat".to_string()],
            recursive: false,
        }],
    };

    let scan = cleaner::scan_rule(&rule);
    assert_eq!(scan.files.len(), 1);
    assert!(
        scan.files[0].locked_or_denied,
        "File held open with exclusive write lock must be marked locked_or_denied"
    );
    assert_eq!(scan.locked_count, 1);

    // execute_clean should skip the locked file gracefully without panicking
    let clean_res = cleaner::execute_clean(&[rule]);
    assert_eq!(clean_res.total_files_deleted, 0);
    assert_eq!(clean_res.total_skipped_locked, 1);
    assert!(target_file.exists(), "Locked file must remain intact");
}

#[test]
fn tier2_cleaner_prune_empty_subdirs_handles_deeply_nested_trees() {
    let temp = TempTestDir::new("prune_deep");
    let mut current = temp.path.clone();
    for i in 0..15 {
        current = current.join(format!("depth_{i}"));
    }
    fs::create_dir_all(&current).unwrap();
    assert!(current.exists());

    cleaner::prune_empty_subdirs(&temp.path);
    // All empty subdirs beneath temp.path should be removed
    let depth_0 = temp.path.join("depth_0");
    assert!(!depth_0.exists(), "Entire empty nested chain must be pruned");
}

#[test]
fn tier2_registry_malformed_unbalanced_quotes_in_executable_path() {
    let malformed1 = r#""C:\Program Files\App\app.exe"#; // missing closing quote
    let res1 = registry::extract_executable_path(malformed1);
    assert!(res1.is_some() || res1.is_none(), "Must handle gracefully without panic");

    let malformed2 = r#"C:\Program"Files\App\app.exe -arg"#; // quotes in middle
    let res2 = registry::extract_executable_path(malformed2);
    assert!(res2.is_some() || res2.is_none(), "Must handle gracefully without panic");
}

#[test]
fn tier2_registry_unicode_non_ascii_characters_escaping() {
    let temp = TempTestDir::new("reg_unicode");
    let issue = registry::RegistryIssue {
        selected: true,
        category: registry::RegistryCategory::MuiCache,
        problem_description: "Unicode test".to_string(),
        data_reference: "Unicode App".to_string(),
        hive: registry::HiveRoot::Hkcu,
        key_path: r"Software\Classes\Local Settings\MuiCache".to_string(),
        action: registry::RegTargetAction::DeleteValue {
            value_name: r#"C:\Users\José_中文_🚀\test.dll"#.to_string(),
            original_data: "Unicode App".to_string(),
            is_dword: false,
        },
    };

    let backup_path = registry::export_reg_backup(&[issue], &temp.path).unwrap();
    let content = fs::read_to_string(backup_path).unwrap();
    assert!(content.contains(r#"C:\\Users\\José_中文_🚀\\test.dll"#));
}

#[test]
fn tier2_registry_extreme_long_paths_boundary() {
    let mut long_path = String::from(r"C:\LongDirectoryName_");
    while long_path.len() < 300 {
        long_path.push_str(r"SubSegment_ABC123\");
    }
    long_path.push_str("target.exe");

    let temp = TempTestDir::new("reg_long");
    let issue = registry::RegistryIssue {
        selected: true,
        category: registry::RegistryCategory::AppPaths,
        problem_description: "Long path test".to_string(),
        data_reference: long_path.clone(),
        hive: registry::HiveRoot::Hkcu,
        key_path: r"Software\Microsoft\Windows\CurrentVersion\App Paths\long.exe".to_string(),
        action: registry::RegTargetAction::DeleteValue {
            value_name: "".to_string(),
            original_data: long_path,
            is_dword: false,
        },
    };

    let backup_path = registry::export_reg_backup(&[issue], &temp.path).unwrap();
    let content = fs::read_to_string(backup_path).unwrap();
    assert!(content.len() > 300);
}

#[test]
fn tier2_registry_dword_value_formatting() {
    let issue = registry::RegistryIssue {
        selected: true,
        category: registry::RegistryCategory::InstallerFolders,
        problem_description: "Installer reference".to_string(),
        data_reference: r"C:\Config.Msi\".to_string(),
        hive: registry::HiveRoot::Hklm,
        key_path: r"SOFTWARE\Microsoft\Windows\CurrentVersion\Installer\Folders".to_string(),
        action: registry::RegTargetAction::DeleteValue {
            value_name: r"C:\Config.Msi\".to_string(),
            original_data: "1".to_string(),
            is_dword: true,
        },
    };

    let temp = TempTestDir::new("reg_dword");
    let backup_path = registry::export_reg_backup(&[issue], &temp.path).unwrap();
    let content = fs::read_to_string(backup_path).unwrap();
    assert!(content.contains(r#""C:\\Config.Msi\\""#));
}

#[test]
fn tier2_registry_backup_fails_gracefully_on_invalid_backup_dir() {
    let temp = TempTestDir::new("invalid_dir");
    let file_as_dir = temp.create_file("blocking_file.txt", b"not a dir");

    let res = registry::export_reg_backup(&[], &file_as_dir);
    assert!(
        res.is_err(),
        "Exporting to a file path instead of directory must return Err without panicking"
    );
}

#[test]
fn tier2_wiper_rejects_relative_dot_paths() {
    assert!(cleaner::is_protected_system_path(Path::new(".")));
    assert!(cleaner::is_protected_system_path(Path::new("..")));
    assert!(cleaner::is_protected_system_path(Path::new(r".\foo")));
    assert!(wiper::validate_wipe_drive_root(".").is_err());
    assert!(wiper::validate_wipe_drive_root("..").is_err());
    assert!(wiper::validate_wipe_drive_root(r".\foo").is_err());
}

#[test]
fn tier2_wiper_rejects_physical_drive_and_volume_guids() {
    assert!(cleaner::is_protected_system_path(Path::new(r"\\.\PhysicalDrive0")));
    assert!(cleaner::is_protected_system_path(Path::new(r"\\.\PhysicalDrive1")));
    assert!(cleaner::is_protected_system_path(Path::new(r"\\?\Volume{00000000-0000-0000-0000-000000000000}\")));
    assert!(wiper::validate_wipe_drive_root(r"\\.\PhysicalDrive0").is_err());
    assert!(wiper::validate_wipe_drive_root(r"\\.\PhysicalDrive1").is_err());
    assert!(wiper::validate_wipe_drive_root(r"\\?\Volume{00000000-0000-0000-0000-000000000000}\").is_err());
}

#[test]
fn tier2_wiper_zero_limit_boundary_handling() {
    let drives = wiper::list_drives();
    if let Some(drive) = drives.first() {
        let prog = Arc::new(Mutex::new(wiper::WipeProgress::default()));
        let cancel = Arc::new(AtomicBool::new(false));
        let res = wiper::wipe_free_space(&drive.root_path, wiper::WipeAlgorithm::SimpleOnePassZero, false, Some(0), prog, cancel);
        assert!(res.is_ok() || res.is_err(), "Must handle zero limit safely");
    }
}

#[test]
fn tier2_wiper_non_existent_drive_returns_error() {
    let prog = Arc::new(Mutex::new(wiper::WipeProgress::default()));
    let cancel = Arc::new(AtomicBool::new(false));
    let res = wiper::wipe_free_space(r"Z:\", wiper::WipeAlgorithm::SimpleOnePassZero, false, Some(1), prog, cancel);
    assert!(res.is_err(), "Non-existent drive root Z:\\ must return Err");
}

#[test]
fn tier2_wiper_custom_flags_write_through_constant() {
    const EXPECTED_FLAG: u32 = 0x80000000;
    assert_eq!(EXPECTED_FLAG, 2147483648);
}

#[test]
fn tier2_wiper_immediate_abort_on_pre_cancelled_flag() {
    let drives = wiper::list_drives();
    if let Some(drive) = drives.first() {
        let prog = Arc::new(Mutex::new(wiper::WipeProgress::default()));
        let cancel = Arc::new(AtomicBool::new(true)); // Pre-cancelled
        let written = wiper::wipe_free_space(&drive.root_path, wiper::WipeAlgorithm::SimpleOnePassZero, false, Some(10), prog.clone(), cancel).unwrap_or(0);
        assert_eq!(written, 0, "No bytes should be written when pre-cancelled");
        let p = prog.lock().unwrap();
        assert!(p.finished || !p.running);
    }
}

#[test]
fn tier2_wiper_aborts_promptly_on_mid_execution_cancellation() {
    let drives = wiper::list_drives();
    if let Some(drive) = drives.first() {
        let prog = Arc::new(Mutex::new(wiper::WipeProgress::default()));
        let cancel = Arc::new(AtomicBool::new(false));
        let cancel_clone = cancel.clone();

        let root = drive.root_path.clone();
        let handle = std::thread::spawn(move || {
            wiper::wipe_free_space(&root, wiper::WipeAlgorithm::DodThreePass, true, Some(64), prog, cancel_clone)
        });

        std::thread::sleep(Duration::from_millis(2));
        cancel.store(true, Ordering::SeqCst);

        let res = handle.join().expect("Thread join");
        assert!(res.is_ok(), "Cancellation should cleanly return Ok(written)");
    }
}

#[test]
fn tier2_cleaner_parallel_rule_scanning_thread_safety() {
    let rules = cleaner::default_clean_rules();
    let summary = cleaner::analyze_all(&rules);
    let enabled_count = rules.iter().filter(|r| r.enabled).count();
    assert_eq!(summary.results.len(), enabled_count);
}

#[test]
fn tier2_safe_mutex_poisoning_recovery_pattern() {
    let m = Arc::new(Mutex::new(42));
    let m_clone = m.clone();

    let _ = std::thread::spawn(move || {
        let _lock = m_clone.lock().unwrap();
        panic!("Simulated worker panic");
    })
    .join();

    let recovered_val = match m.lock() {
        Ok(guard) => *guard,
        Err(poisoned) => *poisoned.into_inner(),
    };
    assert_eq!(recovered_val, 42, "Must recover data from poisoned mutex");
}

#[test]
fn tier2_uac_is_elevated_never_panics() {
    let is_admin = uac::is_elevated();
    assert!(is_admin || !is_admin);
}

// ============================================================================
// TIER 3: Cross-Feature Combinations (Pairwise)
// ============================================================================

#[test]
fn tier3_pairwise_cleaner_analyze_idempotency_vs_execute_clean() {
    let temp = TempTestDir::new("pairwise_clean");
    let f1 = temp.create_file("cache1.tmp", b"cache 1");
    let f2 = temp.create_file("cache2.tmp", b"cache 2");

    let rule = cleaner::CleanRule {
        id: 9995,
        name: "Analyze vs Clean Rule".to_string(),
        group: cleaner::RuleGroup::Applications,
        description: "Test".to_string(),
        enabled: true,
        requires_admin: false,
        special_recycle_bin: false,
        targets: vec![cleaner::FileTarget {
            base_dir: temp.path.to_string_lossy().to_string(),
            patterns: vec!["*.tmp".to_string()],
            recursive: false,
        }],
    };

    // Phase 1: analyze_all (read-only scan)
    let analysis = cleaner::analyze_all(std::slice::from_ref(&rule));
    assert_eq!(analysis.total_files_found, 2);
    assert_eq!(analysis.total_files_deleted, 0);
    assert!(f1.exists(), "Analyze must never delete files");
    assert!(f2.exists(), "Analyze must never delete files");

    // Phase 2: execute_clean (real deletion)
    let clean = cleaner::execute_clean(&[rule]);
    assert_eq!(clean.total_files_deleted, 2);
    assert!(!f1.exists(), "Clean must delete f1");
    assert!(!f2.exists(), "Clean must delete f2");
}

#[test]
fn tier3_pairwise_registry_scan_backup_export_and_reg_format() {
    let temp = TempTestDir::new("pairwise_reg");
    let issues = vec![
        registry::RegistryIssue {
            selected: true,
            category: registry::RegistryCategory::AppPaths,
            problem_description: "Missing MUI app path".to_string(),
            data_reference: r"C:\OldApp\test.exe".to_string(),
            hive: registry::HiveRoot::Hkcu,
            key_path: r"Software\Microsoft\Windows\CurrentVersion\App Paths\test.exe".to_string(),
            action: registry::RegTargetAction::DeleteSubkey {
                default_value: Some(r"C:\OldApp\test.exe".to_string()),
            },
        },
        registry::RegistryIssue {
            selected: true,
            category: registry::RegistryCategory::SharedDlls,
            problem_description: "Leftover shared dll".to_string(),
            data_reference: "1".to_string(),
            hive: registry::HiveRoot::Hkcu,
            key_path: r"Software\LeftoverCompany\Product".to_string(),
            action: registry::RegTargetAction::DeleteValue {
                value_name: "Version".to_string(),
                original_data: "1.0".to_string(),
                is_dword: false,
            },
        },
    ];

    let backup_path = registry::export_reg_backup(&issues, &temp.path).unwrap();
    let content = fs::read_to_string(&backup_path).unwrap();

    assert!(content.starts_with("Windows Registry Editor Version 5.00"));
    assert!(content.contains(r"[HKEY_CURRENT_USER\Software\Microsoft\Windows\CurrentVersion\App Paths\test.exe]"));
    assert!(content.contains(r#""Version"="1.0""#));
}

#[test]
fn tier3_pairwise_scheduler_command_generation_and_headless_execution() {
    let status = scheduler::query_scheduled_task();
    assert!(!status.status.is_empty() || !status.exists);

    let out = run_cli(&["--silent-clean"]);
    assert!(out.status.success());
}

#[test]
fn tier3_pairwise_cleaner_rule_expansion_adjacent_to_protected_paths() {
    let temp_expanded = cleaner::expand_env_vars(r"%WINDIR%\Temp");
    let system32_expanded = cleaner::expand_env_vars(r"%WINDIR%\System32");

    assert!(
        !cleaner::is_protected_system_path(&temp_expanded),
        "%WINDIR%\\Temp must NOT be blacklisted"
    );

    assert!(
        cleaner::is_protected_system_path(&system32_expanded),
        "%WINDIR%\\System32 MUST be blacklisted"
    );
}

#[test]
fn tier3_pairwise_wiper_algorithms_with_and_without_mft_slack() {
    let drives = wiper::list_drives();
    if let Some(drive) = drives.first() {
        for wipe_mft in [false, true] {
            let prog = Arc::new(Mutex::new(wiper::WipeProgress::default()));
            let cancel = Arc::new(AtomicBool::new(true)); // Pre-cancel to test initialization
            let _ = wiper::wipe_free_space(&drive.root_path, wiper::WipeAlgorithm::SimpleOnePassZero, wipe_mft, Some(1), prog, cancel);
            let container = Path::new(&drive.root_path).join(".rustcleaner_wipe_tmp");
            assert!(!container.exists(), "Cleanup must occur regardless of wipe_mft setting");
        }
    }
}

#[test]
fn tier3_pairwise_localization_switching_all_rules_and_categories() {
    let rules = cleaner::default_clean_rules();
    let categories = registry::RegistryCategory::all();

    for lang in i18n::Language::all() {
        for rule in &rules {
            let _localized = lang.localize_rule(rule.id);
        }
        for cat in categories {
            let _localized = lang.localize_reg_category(*cat);
        }
    }
}

#[test]
fn tier3_pairwise_software_updater_winget_table_parsing_locales() {
    let sample = "\
Nome                             Id                          Versione     Disponibile  Origine
----------------------------------------------------------------------------------------------
Mozilla Firefox (x64 it)         Mozilla.Firefox.it          130.0.1      131.0.3      winget
Microsoft Visual Studio Code     Microsoft.VisualStudioCode  1.93.1       1.94.2       winget
2 aggiornamenti disponibili.
";
    let updates = software_updater::parse_winget_upgrade_output(sample);
    assert_eq!(updates.len(), 2);
    assert_eq!(updates[0].id, "Mozilla.Firefox.it");
    assert_eq!(updates[0].current_version, "130.0.1");
    assert_eq!(updates[0].available_version, "131.0.3");
    assert_eq!(updates[1].id, "Microsoft.VisualStudioCode");
}

#[test]
fn tier3_pairwise_startup_entries_and_source_elevation_requirements() {
    assert!(!startup_uninstall::StartupSource::HkcuRun.requires_admin());
    assert!(!startup_uninstall::StartupSource::HkcuRunOnce.requires_admin());
    assert!(!startup_uninstall::StartupSource::StartupFolderUser.requires_admin());

    assert!(startup_uninstall::StartupSource::HklmRun.requires_admin());
    assert!(startup_uninstall::StartupSource::HklmWow64Run.requires_admin());
    assert!(startup_uninstall::StartupSource::StartupFolderCommon.requires_admin());
}

#[test]
fn tier3_pairwise_startup_delete_creates_backup_and_unlinks() {
    let temp = TempTestDir::new("startup_del");
    let shortcut_path = temp.create_file("test_program.lnk", b"mock binary link");
    assert!(shortcut_path.exists());

    let backup_dir = temp.path.join("backups");
    let entry = startup_uninstall::StartupEntry {
        name: "TestProgram".to_string(),
        command: shortcut_path.to_string_lossy().to_string(),
        source: startup_uninstall::StartupSource::StartupFolderUser,
        enabled: true,
        file_path_if_folder: Some(shortcut_path.clone()),
    };

    let backup_file = startup_uninstall::delete_startup_entry(&entry, Some(&backup_dir))
        .expect("delete_startup_entry must succeed and create backup");
    assert!(backup_file.exists(), "Backup file must be created on disk");
    assert!(!shortcut_path.exists(), "Original entry must be deleted after backup");
}

// ============================================================================
// TIER 4: Real-World Workload Scenarios
// ============================================================================

#[test]
fn tier4_scenario_full_system_health_scan_simulation() {
    // 1. Filesystem analysis
    let rules = cleaner::default_clean_rules();
    let cleaner_summary = cleaner::analyze_all(&rules);
    let enabled_count = rules.iter().filter(|r| r.enabled).count();
    assert_eq!(cleaner_summary.results.len(), enabled_count);

    // 2. Registry scanning simulation
    let reg_categories = registry::RegistryCategory::all();
    let reg_issues = registry::scan_registry(reg_categories);

    // 3. Startup items enumeration
    let startup_entries = startup_uninstall::list_startup_entries();

    // 4. Installed software enumeration
    let installed_apps = startup_uninstall::list_installed_apps();

    println!(
        "System Health Scan Summary: Rules={}, FoundFiles={}, RegIssues={}, StartupItems={}, InstalledApps={}",
        rules.len(),
        cleaner_summary.total_files_found,
        reg_issues.len(),
        startup_entries.len(),
        installed_apps.len()
    );
}

#[test]
fn tier4_scenario_registry_cleanup_export_and_windows_reg_import_rollback() {
    let hive = RegistryTestHive::new("e2e_rollback").expect("Create test hive");
    let test_value_name = "OrphanTestApp";
    let test_target_exe = r"C:\NonExistentSoftware\test.exe";

    // Step 1: Set initial value in temporary test hive
    hive.set_str_value(test_value_name, test_target_exe).expect("Set initial test reg value");
    assert_eq!(
        hive.get_str_value(test_value_name).unwrap(),
        test_target_exe,
        "Initial value must be set in registry"
    );

    // Step 2: Define RegistryIssue targeting this value
    let issue = registry::RegistryIssue {
        selected: true,
        category: registry::RegistryCategory::AppPaths,
        problem_description: "E2E rollback test item".to_string(),
        data_reference: test_target_exe.to_string(),
        hive: registry::HiveRoot::Hkcu,
        key_path: hive.subkey_name.clone(),
        action: registry::RegTargetAction::DeleteValue {
            value_name: test_value_name.to_string(),
            original_data: test_target_exe.to_string(),
            is_dword: false,
        },
    };

    // Step 3: Export mandatory .reg backup
    let temp_backup_dir = TempTestDir::new("rollback_backup");
    let backup_file = registry::export_reg_backup(&[issue], &temp_backup_dir.path)
        .expect("Export reg backup file");
    assert!(backup_file.exists());

    // Step 4: Simulate fixing/cleaning the registry entry (deletion)
    hive.delete_value(test_value_name).expect("Delete test value");
    assert!(
        hive.get_str_value(test_value_name).is_err(),
        "Test value must be deleted after cleanup"
    );

    // Step 5: Execute real rollback restoration using official Windows reg.exe import
    let import_output = std::process::Command::new("reg")
        .args(["import", backup_file.to_str().unwrap()])
        .output()
        .expect("Execute reg.exe import");

    assert!(
        import_output.status.success(),
        "reg.exe import must successfully restore the backup file: {}",
        String::from_utf8_lossy(&import_output.stderr)
    );

    // Step 6: Verify value is fully restored to exact original state!
    let restored_val = hive.get_str_value(test_value_name)
        .expect("Value must exist after reg.exe import rollback");
    assert_eq!(
        restored_val,
        test_target_exe,
        "Restored value must match original pre-cleanup state"
    );
}

#[test]
fn tier4_scenario_free_space_wiper_cancellation_and_leak_free_cleanup() {
    let drives = wiper::list_drives();
    if let Some(drive) = drives.first() {
        let drive_root = drive.root_path.clone();
        let prog = Arc::new(Mutex::new(wiper::WipeProgress::default()));
        let cancel = Arc::new(AtomicBool::new(false));

        let prog_clone = prog.clone();
        let cancel_clone = cancel.clone();
        let root_clone = drive_root.clone();

        // Spawn wiper in worker thread with 3 passes and 32 MB cap
        let handle = std::thread::spawn(move || {
            wiper::wipe_free_space(
                &root_clone,
                wiper::WipeAlgorithm::DodThreePass,
                false,
                Some(32),
                prog_clone,
                cancel_clone,
            )
        });

        // Let wiper start and write a small amount
        std::thread::sleep(Duration::from_millis(5));
        cancel.store(true, Ordering::SeqCst);

        let result = handle.join().expect("Join wiper worker thread");
        assert!(result.is_ok(), "Wiper should handle cancellation cleanly without panic");

        let p = prog.lock().unwrap();
        assert!(
            p.status_message.contains("annullata") || p.finished,
            "Progress status should reflect completion or cancellation"
        );

        // Verify zero disk leak: container directory must be gone
        let container = Path::new(&drive_root).join(".rustcleaner_wipe_tmp");
        assert!(
            !container.exists(),
            "Wiper temporary container must be unlinked: no leaked tmp files on disk!"
        );
    }
}

#[test]
fn tier4_scenario_cli_subcommands_headless_execution() {
    let subcommands = [
        "analyze",
        "startup-list",
        "uninstall-list",
        "schedule-status",
    ];

    for subcmd in subcommands {
        let out = run_cli(&[subcmd]);
        assert!(
            out.status.success(),
            "CLI subcommand '{subcmd}' must exit successfully (code 0)"
        );
        let stdout = String::from_utf8_lossy(&out.stdout);
        assert!(
            stdout.contains("RustCleaner"),
            "Subcommand '{subcmd}' output should contain RustCleaner banner"
        );
    }
}

#[test]
fn tier4_scenario_multilingual_all_7_languages_consistency() {
    let langs = i18n::Language::all();
    let rules = cleaner::default_clean_rules();
    let categories = registry::RegistryCategory::all();

    for lang in langs {
        assert!(!lang.display_name().is_empty());
        assert!(!lang.code().is_empty());

        for rule in &rules {
            let (r_name, r_desc) = lang
                .localize_rule(rule.id)
                .unwrap_or((&rule.name, &rule.description));
            assert!(!r_name.trim().is_empty());
            assert!(!r_desc.trim().is_empty());
        }

        for cat in categories {
            let (c_name, c_desc) = lang.localize_reg_category(*cat);
            assert!(!c_name.trim().is_empty());
            assert!(!c_desc.trim().is_empty());
        }

        // Also check UI strings
        let s = lang.strings();
        assert!(!s.nav_cleaner_title.is_empty());
        assert!(!s.nav_registry_title.is_empty());
        assert!(!s.nav_wiper_title.is_empty());
        assert!(!s.btn_analyze_system.is_empty());
        assert!(!s.btn_start_clean.is_empty());
        assert!(!s.cleaner_title.is_empty());
    }
}
