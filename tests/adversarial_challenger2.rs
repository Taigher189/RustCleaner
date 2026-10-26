//! Adversarial Challenger 2 Test Suite for Milestone 1 Iteration 2
//! Tests Registry & Cleaner safety invariants:
//! 1. delete_startup_entry .reg file format (Windows Registry Editor Version 5.00) & reg import roundtrip.
//! 2. DWORD value formatting (dword:00000000) & reg import roundtrip.
//! 3. is_protected_system_path against edge cases (Windows root, System32, WinSxS, bootmgr, user desktop & docs, cleanable paths).
//! 4. probe_file_lock read-only idempotency & locked file handling.

mod common;

#[path = "../src/cleaner.rs"]
pub mod cleaner;
#[path = "../src/registry.rs"]
pub mod registry;
#[path = "../src/startup_uninstall.rs"]
pub mod startup_uninstall;

use common::{LockedFileGuard, TempTestDir};
use std::fs;
use std::path::Path;
use std::process::Command;
use winreg::enums::{HKEY_CURRENT_USER, KEY_ALL_ACCESS, KEY_READ, KEY_WRITE};
use winreg::RegKey;

// ============================================================================
// 1. DELETE_STARTUP_ENTRY & REGISTRY EXPORT SPEC CONFORMANCE
// ============================================================================

#[test]
fn test_delete_startup_entry_creates_valid_reg_format_and_imports() {
    let temp = TempTestDir::new("startup_reg_spec");
    let backup_dir = temp.path.join("backups");

    // 1. Setup a test startup value in HKCU Run
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let run_subpath = r"Software\Microsoft\Windows\CurrentVersion\Run";
    let (key, _) = hkcu
        .create_subkey_with_flags(run_subpath, KEY_WRITE | KEY_READ)
        .expect("Must open/create HKCU Run key for testing");

    let val_name = "RustCleaner_Adversarial_Test_Entry";
    let val_cmd = r#""C:\Program Files\TestApp\app.exe" --startup --silent"#;
    key.set_value(val_name, &val_cmd).expect("Must set test startup value");

    let entry = startup_uninstall::StartupEntry {
        name: val_name.to_string(),
        command: val_cmd.to_string(),
        source: startup_uninstall::StartupSource::HkcuRun,
        enabled: true,
        file_path_if_folder: None,
    };

    // 2. Execute delete_startup_entry
    let backup_file = startup_uninstall::delete_startup_entry(&entry, Some(&backup_dir))
        .expect("delete_startup_entry must succeed");

    assert!(backup_file.exists(), "Backup .reg file must exist on disk");

    // 3. Verify original registry value is deleted
    assert!(
        key.get_value::<String, _>(val_name).is_err(),
        "Registry value must be deleted from HKCU Run"
    );

    // 4. Verify .reg file syntax complies strictly with Windows Registry Editor Version 5.00
    let content = fs::read_to_string(&backup_file).expect("Must read backup .reg file");
    assert!(
        content.starts_with("Windows Registry Editor Version 5.00\r\n\r\n"),
        "Must strictly begin with 'Windows Registry Editor Version 5.00\\r\\n\\r\\n'"
    );
    assert!(
        content.contains(r"[HKEY_CURRENT_USER\Software\Microsoft\Windows\CurrentVersion\Run]"),
        "Must contain standard subkey bracket section"
    );
    assert!(
        content.contains(r#""RustCleaner_Adversarial_Test_Entry"="\"C:\\Program Files\\TestApp\\app.exe\" --startup --silent""#),
        "Must contain escaped value name and command string"
    );
    assert!(
        content.ends_with("\r\n\r\n"),
        "Must end with blank CRLF newline"
    );

    // 5. Test empirical restoration using Windows native `reg import`
    let import_out = Command::new("reg")
        .args(["import", backup_file.to_str().unwrap()])
        .output()
        .expect("Must invoke native Windows reg.exe");

    assert!(
        import_out.status.success(),
        "Windows reg.exe import failed on generated .reg file: {:?}",
        String::from_utf8_lossy(&import_out.stderr)
    );

    // 6. Verify value was restored with exact fidelity
    let restored = key.get_value::<String, _>(val_name).expect("Value must be restored");
    assert_eq!(restored, val_cmd, "Restored value must match original exactly");

    // 7. Clean up
    let _ = key.delete_value(val_name);
}

#[test]
fn test_delete_startup_entry_with_special_characters_escaping() {
    let temp = TempTestDir::new("startup_special_chars");
    let backup_dir = temp.path.join("backups");

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let run_subpath = r"Software\Microsoft\Windows\CurrentVersion\Run";
    let (key, _) = hkcu
        .create_subkey_with_flags(run_subpath, KEY_WRITE | KEY_READ)
        .expect("Must open HKCU Run key");

    let val_name = r#"Test"Special"Key\With\Slashes"#;
    let val_cmd = r#"C:\Path With Spaces\"Special Exe"\tool.exe -opt="value" \flag"#;
    key.set_value(val_name, &val_cmd).expect("Set test value");

    let entry = startup_uninstall::StartupEntry {
        name: val_name.to_string(),
        command: val_cmd.to_string(),
        source: startup_uninstall::StartupSource::HkcuRun,
        enabled: true,
        file_path_if_folder: None,
    };

    let backup_file = startup_uninstall::delete_startup_entry(&entry, Some(&backup_dir))
        .expect("delete_startup_entry must succeed");

    assert!(key.get_value::<String, _>(val_name).is_err());

    // Import with reg.exe
    let import_out = Command::new("reg")
        .args(["import", backup_file.to_str().unwrap()])
        .output()
        .expect("Invoke reg.exe");

    assert!(
        import_out.status.success(),
        "reg import must accept special escaped characters: {:?}",
        String::from_utf8_lossy(&import_out.stderr)
    );

    let restored = key.get_value::<String, _>(val_name).expect("Restored special value");
    assert_eq!(restored, val_cmd);

    let _ = key.delete_value(val_name);
}

// ============================================================================
// 2. DWORD VALUE FORMATTING & REG IMPORT SPEC CONFORMANCE
// ============================================================================

#[test]
fn test_dword_value_handling_in_registry_export_and_reg_import() {
    let temp = TempTestDir::new("dword_test");
    let test_hive = common::RegistryTestHive::new("dword_reg").expect("Create test hive");
    let subkey_path = &test_hive.subkey_name;

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let (key, _) = hkcu
        .create_subkey_with_flags(subkey_path, KEY_ALL_ACCESS)
        .expect("Must create test registry subkey");

    // Exercise test hive methods
    test_hive.set_str_value("ProbeKey", "ProbeVal").expect("set str");
    assert_eq!(test_hive.get_str_value("ProbeKey").expect("get str"), "ProbeVal");
    test_hive.delete_value("ProbeKey").expect("del str");

    // Also verify CLI helper is functional
    let cli_out = common::run_cli(&["--version"]);
    assert!(cli_out.status.success());

    // Test a matrix of DWORD values: 0, 1, 42, 0x12345678, u32::MAX
    let dword_cases: &[( &str, u32, &str )] = &[
        ("ZeroVal", 0, "00000000"),
        ("OneVal", 1, "00000001"),
        ("AnswerVal", 42, "0000002a"),
        ("CustomVal", 0x12345678, "12345678"),
        ("MaxVal", 0xFFFFFFFF, "ffffffff"),
    ];

    let mut issues = Vec::new();
    for (name, val, formatted_hex) in dword_cases {
        key.set_value(name, val).expect("Set dword value");
        issues.push(registry::RegistryIssue {
            selected: true,
            category: registry::RegistryCategory::InstallerFolders,
            problem_description: format!("DWORD Test {name}"),
            data_reference: name.to_string(),
            hive: registry::HiveRoot::Hkcu,
            key_path: subkey_path.to_string(),
            action: registry::RegTargetAction::DeleteValue {
                value_name: name.to_string(),
                original_data: formatted_hex.to_string(),
                is_dword: true,
            },
        });
    }

    // Export .reg backup
    let backup_file = registry::export_reg_backup(&issues, &temp.path)
        .expect("export_reg_backup must succeed");

    let content = fs::read_to_string(&backup_file).expect("Read backup file");
    assert!(content.starts_with("Windows Registry Editor Version 5.00\r\n\r\n"));

    for (name, _val, formatted_hex) in dword_cases {
        let expected_line = format!("\"{name}\"=dword:{formatted_hex}");
        assert!(
            content.contains(&expected_line),
            "Content must contain line '{expected_line}'"
        );
    }

    // Delete values from registry to test restore
    for (name, _, _) in dword_cases {
        let _ = key.delete_value(name);
        assert!(key.get_value::<u32, _>(name).is_err());
    }

    // Native Windows reg import
    let import_out = Command::new("reg")
        .args(["import", backup_file.to_str().unwrap()])
        .output()
        .expect("Invoke reg.exe");

    assert!(
        import_out.status.success(),
        "reg import failed on DWORD values: {:?}",
        String::from_utf8_lossy(&import_out.stderr)
    );

    // Verify all DWORD values were restored with exact binary u32 match
    for (name, expected_val, _) in dword_cases {
        let restored_val: u32 = key.get_value(name).expect("Restored DWORD");
        assert_eq!(
            restored_val, *expected_val,
            "DWORD value for '{name}' must match {expected_val:#x}"
        );
    }

    // Clean up
    let _ = hkcu.delete_subkey_all(subkey_path);
}

// ============================================================================
// 3. IS_PROTECTED_SYSTEM_PATH EDGE CASE STRESS TESTING
// ============================================================================

#[test]
fn test_is_protected_system_path_system32_subpaths_and_casing() {
    let system32_cases = [
        r"C:\Windows\System32",
        r"C:\Windows\System32\",
        r"C:\Windows\System32\\",
        r"C:/Windows/System32",
        r"C:/Windows/System32/",
        r"c:\windows\system32",
        r"C:\WINDOWS\SYSTEM32",
        r"c:\wInDoWs\sYsTeM32",
        r"C:\Windows\System32\cmd.exe",
        r"C:\Windows\System32\drivers\etc\hosts",
        r"C:\Windows\System32\config\SAM",
        r"C:\Windows\System32\Printing_Admin_Scripts\en-US\prnport.vbs",
        r"\\?\C:\Windows\System32\notepad.exe",
        // SysWOW64
        r"C:\Windows\SysWOW64",
        r"c:\windows\syswow64\kernel32.dll",
        r"C:\WINDOWS\SYSWOW64\",
        // WinSxS
        r"C:\Windows\WinSxS",
        r"c:\windows\winsxs\",
        r"C:\Windows\WinSxS\amd64_microsoft_windows_manifest.xml",
        r"C:/Windows/WinSxS/catalogs/test.cat",
        // Boot & bootmgr
        r"C:\Windows\Boot",
        r"C:\Windows\Boot\DVD\boot.sdi",
        r"C:\bootmgr",
        r"c:\bootmgr",
        r"D:\bootmgr",
        r"C:\Windows\bootmgr",
        r"bootmgr",
        // Program Files\Windows NT
        r"C:\Program Files\Windows NT",
        r"C:\Program Files\Windows NT\Accessories\wordpad.exe",
        r"C:\Program Files (x86)\Windows NT",
        r"C:\Program Files (x86)\Windows NT\Accessories",
    ];

    for path_str in system32_cases {
        let p = Path::new(path_str);
        assert!(
            cleaner::is_protected_system_path(p),
            "Expected '{path_str}' to be flagged as protected system path"
        );
    }
}

#[test]
fn test_is_protected_system_path_user_desktop_and_docs_matrix() {
    let user_cases = [
        r"C:\Users\Alice\Documents",
        r"c:\users\alice\documents\",
        r"C:\Users\Alice\Documents\report.docx",
        r"C:/Users/Alice/Documents/nested/thesis.pdf",
        r"C:\Users\Alice\Desktop",
        r"C:\Users\Alice\Desktop\shortcut.lnk",
        r"C:\Users\Alice\Pictures\photo.jpg",
        r"C:\Users\Alice\Music\track.mp3",
        r"C:\Users\Alice\Videos\clip.mp4",
        r"C:\Users\Alice\OneDrive\Documents\sheet.xlsx",
        r"C:\Users\Alice\OneDrive\Desktop\work.doc",
        r"C:\Users\Alice\OneDrive\Pictures\img.png",
        // Alternate drive
        r"D:\Users\Bob\Documents\secret.txt",
        r"D:\Users\Bob\Desktop\shortcut.lnk",
    ];

    for path_str in user_cases {
        let p = Path::new(path_str);
        assert!(
            cleaner::is_protected_system_path(p),
            "Expected user personal path '{path_str}' to be protected"
        );
    }
}

#[test]
fn test_is_protected_system_path_allows_cleanable_temp_and_logs() {
    let cleanable_paths = [
        r"C:\Windows\Temp\test.tmp",
        r"C:\Windows\Temp\subfolder\junk.tmp",
        r"C:\Windows\Logs\cbs.log",
        r"C:\Windows\Logs\CBS\cbs.log",
        r"C:\Windows\Panther\setupact.log",
        r"C:\Users\Alice\AppData\Local\Temp\cache.tmp",
        r"C:\Users\Alice\AppData\Local\Microsoft\Windows\Explorer\thumbcache_256.db",
    ];

    for path_str in cleanable_paths {
        let p = Path::new(path_str);
        assert!(
            !cleaner::is_protected_system_path(p),
            "Expected cleanable path '{path_str}' to NOT be blocked by blacklist"
        );
    }
}

#[test]
fn test_is_protected_system_path_drive_roots_and_device_paths() {
    let root_and_devices = [
        r"C:\",
        r"C:",
        r"D:\",
        r"D:",
        r"\",
        r"\\.\PhysicalDrive0",
        r"\\.\PhysicalDrive1",
        r"\\.\C:",
        r"\??\PhysicalDrive0",
        r"\\?\Volume{12345678-1234-1234-1234-123456789abc}\",
    ];

    for path_str in root_and_devices {
        let p = Path::new(path_str);
        assert!(
            cleaner::is_protected_system_path(p),
            "Root / device path '{path_str}' must be protected"
        );
    }
}

// ============================================================================
// 4. PROBE_FILE_LOCK READ-ONLY IDEMPOTENCY
// ============================================================================

#[test]
fn test_probe_file_lock_read_only_idempotency() {
    let temp = TempTestDir::new("probe_idempotency");
    let ro_file = temp.create_readonly_file("important_readonly.log", b"critical log data");

    // Verify initial state
    let meta_before = fs::metadata(&ro_file).expect("metadata before");
    assert!(meta_before.permissions().readonly(), "File must start readonly");
    let len_before = meta_before.len();

    // Scan rule triggers probe_file_lock
    let rule = cleaner::CleanRule {
        id: 9999,
        name: "Test Readonly Probe".to_string(),
        group: cleaner::RuleGroup::Applications,
        description: "Test".to_string(),
        enabled: true,
        requires_admin: false,
        special_recycle_bin: false,
        targets: vec![cleaner::FileTarget {
            base_dir: temp.path.to_string_lossy().to_string(),
            patterns: vec!["*.log".to_string()],
            recursive: false,
        }],
    };

    // Run scan multiple times in succession to verify idempotency
    for i in 1..=5 {
        let scan_result = cleaner::scan_rule(&rule);
        assert_eq!(scan_result.files.len(), 1, "Pass {i}: Must discover file");
        assert!(
            !scan_result.files[0].locked_or_denied,
            "Pass {i}: Unlocked readonly file must probe as not locked"
        );

        // Assert file permissions and contents have NOT been mutated
        let meta_after = fs::metadata(&ro_file).expect("metadata after");
        assert!(
            meta_after.permissions().readonly(),
            "Pass {i}: Readonly permission must remain intact"
        );
        assert_eq!(meta_after.len(), len_before, "Pass {i}: File length must be preserved");
    }

    let content_after = fs::read(&ro_file).expect("read content");
    assert_eq!(content_after, b"critical log data", "File contents must remain unchanged");
}

#[test]
fn test_probe_file_lock_locked_file_non_destructive_and_recovery() {
    let temp = TempTestDir::new("probe_locked");
    let target = temp.path.join("exclusive.log");
    fs::write(&target, b"initial content").expect("create file");

    let rule = cleaner::CleanRule {
        id: 9998,
        name: "Test Locked Probe".to_string(),
        group: cleaner::RuleGroup::Applications,
        description: "Test".to_string(),
        enabled: true,
        requires_admin: false,
        special_recycle_bin: false,
        targets: vec![cleaner::FileTarget {
            base_dir: temp.path.to_string_lossy().to_string(),
            patterns: vec!["*.log".to_string()],
            recursive: false,
        }],
    };

    // 1. Lock file exclusively
    {
        let _guard = LockedFileGuard::new(&target).expect("Must lock file");

        let scan = cleaner::scan_rule(&rule);
        assert_eq!(scan.files.len(), 1);
        assert!(
            scan.files[0].locked_or_denied,
            "File held by exclusive handle must be detected as locked"
        );
        assert!(
            scan.files[0].error_reason.is_some(),
            "Lock reason must be populated"
        );
    }
    // _guard dropped here, releasing exclusive lock

    // 2. Next probe must report file as unlocked without any state leakage
    let scan2 = cleaner::scan_rule(&rule);
    assert_eq!(scan2.files.len(), 1);
    assert!(
        !scan2.files[0].locked_or_denied,
        "File must be unlocked after handle is closed"
    );
    assert!(scan2.files[0].error_reason.is_none());
}
