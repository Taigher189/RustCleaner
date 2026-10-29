//! Adversarial Challenge Test Harness for Milestone 1 Iteration 2
//! Stress-tests concurrency, cancellation, and safety invariant boundaries.


#[path = "../src/cleaner.rs"]
pub mod cleaner;
#[path = "../src/wiper.rs"]
pub mod wiper;

use std::collections::HashSet;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

// ============================================================================
// 1. CONCURRENT WIPER STRESS TESTS
// ============================================================================

#[test]
fn challenge_parallel_threads_calling_wipe_free_space_do_not_collide() {
    let drives = wiper::list_drives();
    let Some(first_drive) = drives.first() else {
        eprintln!("Skipping wiper concurrency stress test: no active drive found.");
        return;
    };
    let drive_root = first_drive.root_path.clone();

    const NUM_THREADS: usize = 12;
    let mut handles = Vec::with_capacity(NUM_THREADS);
    let mut progress_trackers = Vec::with_capacity(NUM_THREADS);

    for i in 0..NUM_THREADS {
        let root = drive_root.clone();
        let progress = Arc::new(Mutex::new(wiper::WipeProgress::default()));
        let cancel = Arc::new(AtomicBool::new(false));
        progress_trackers.push(progress.clone());

        let algorithm = match i % 3 {
            0 => wiper::WipeAlgorithm::SimpleOnePassZero,
            1 => wiper::WipeAlgorithm::DodThreePass,
            _ => wiper::WipeAlgorithm::NsaSevenPass,
        };

        let handle = std::thread::spawn(move || {
            wiper::wipe_free_space(
                &root,
                algorithm,
                Some(2), // 2 MB cap for fast parallel stress
                progress,
                cancel,
            )
        });
        handles.push(handle);
    }

    let mut recorded_containers = Vec::new();
    for (i, handle) in handles.into_iter().enumerate() {
        let result = handle.join().expect("Wiper thread panicked");
        assert!(
            result.is_ok(),
            "Thread {i} failed with error: {:?}",
            result.err()
        );
        let written = result.unwrap();
        assert!(written > 0, "Thread {i} should have written bytes");

        let p = progress_trackers[i].lock().unwrap();
        assert!(p.finished, "Thread {i} progress must indicate finished");
        assert!(p.error.is_none(), "Thread {i} should have no error");
        assert!(
            p.container_dir.is_some(),
            "Thread {i} must record its container_dir"
        );
        let c_dir = p.container_dir.clone().unwrap();
        recorded_containers.push(c_dir);
    }

    // Assert that all container dirs were distinct
    let unique_containers: HashSet<_> = recorded_containers.iter().collect();
    assert_eq!(
        unique_containers.len(),
        NUM_THREADS,
        "Every concurrent wiper session must have had a unique isolated container directory"
    );

    // Verify all temporary containers were completely unlinked on disk
    for (i, c_dir) in recorded_containers.iter().enumerate() {
        assert!(
            !c_dir.exists(),
            "Container dir for thread {i} ('{}') must be unlinked after execution",
            c_dir.display()
        );
    }
}

// ============================================================================
// 2. CANCELLATION IMMEDIACY & UNLINKING STRESS TESTS
// ============================================================================

#[test]
fn challenge_concurrent_cancellations_at_various_stages_unlink_all_temp_files() {
    let drives = wiper::list_drives();
    let Some(first_drive) = drives.first() else {
        return;
    };
    let drive_root = first_drive.root_path.clone();

    const NUM_CANCELLATION_THREADS: usize = 12;
    let mut handles = Vec::with_capacity(NUM_CANCELLATION_THREADS);
    let mut progress_trackers = Vec::with_capacity(NUM_CANCELLATION_THREADS);

    for i in 0..NUM_CANCELLATION_THREADS {
        let root = drive_root.clone();
        let progress = Arc::new(Mutex::new(wiper::WipeProgress::default()));
        let cancel = Arc::new(AtomicBool::new(false));
        progress_trackers.push(progress.clone());

        let cancel_clone = cancel.clone();
        let handle = std::thread::spawn(move || {
            let delay_ms = match i % 4 {
                0 => 0,
                1 => 1,
                2 => 5,
                _ => 15,
            };

            if delay_ms == 0 {
                cancel_clone.store(true, Ordering::SeqCst);
            } else {
                let cancel_bg = cancel_clone.clone();
                std::thread::spawn(move || {
                    std::thread::sleep(Duration::from_millis(delay_ms));
                    cancel_bg.store(true, Ordering::SeqCst);
                });
            }

            let start = Instant::now();
            let res = wiper::wipe_free_space(
                &root,
                wiper::WipeAlgorithm::DodThreePass,
                Some(64),
                progress,
                cancel_clone,
            );
            (res, start.elapsed())
        });
        handles.push(handle);
    }

    let mut session_containers = Vec::new();
    for (i, handle) in handles.into_iter().enumerate() {
        let (result, duration) = handle.join().expect("Cancellation thread panicked");
        assert!(
            result.is_ok(),
            "Thread {i} cancellation returned Err: {:?}",
            result.err()
        );
        assert!(
            duration < Duration::from_secs(5),
            "Cancellation took too long ({duration:?}) for thread {i}"
        );

        let p = progress_trackers[i].lock().unwrap();
        assert!(p.finished, "Thread {i} should be marked finished");
        assert!(
            p.status_message.contains("annullata") || p.status_message.contains("completata"),
            "Status message for thread {i} was unexpected: '{}'",
            p.status_message
        );

        if let Some(c_dir) = &p.container_dir {
            session_containers.push(c_dir.clone());
        }
    }

    // Verify all containers belonging to this test are unlinked
    assert_eq!(session_containers.len(), NUM_CANCELLATION_THREADS);
    for (i, c_dir) in session_containers.iter().enumerate() {
        assert!(
            !c_dir.exists(),
            "Temporary container '{}' for cancelled thread {i} must NOT exist on disk",
            c_dir.display()
        );
    }
}

// ============================================================================
// 3. SYSTEM PATH BLACKLIST BOUNDARY & CASING MATRIX
// ============================================================================

#[test]
fn challenge_blacklist_casing_and_trailing_slashes_matrix() {
    let test_cases = [
        // Standard casing
        r"C:\Windows\System32",
        r"C:\Windows\System32\",
        r"C:\Windows\System32/",
        r"C:\Windows\System32\\",
        r"C:/Windows/System32",
        r"C:/Windows/System32/",
        // Lowercase
        r"c:\windows\system32",
        r"c:\windows\system32\",
        r"c:/windows/system32",
        r"c:/windows/system32/",
        // Uppercase
        r"C:\WINDOWS\SYSTEM32",
        r"C:\WINDOWS\SYSTEM32\",
        // Subpaths
        r"C:\Windows\System32\cmd.exe",
        r"C:\Windows\System32\drivers\etc\hosts",
        r"c:\windows\system32\config\sam",
        // SysWOW64
        r"C:\Windows\SysWOW64",
        r"c:\windows\syswow64\",
        r"C:\WINDOWS\SYSWOW64\kernel32.dll",
        // WinSxS
        r"C:\Windows\WinSxS",
        r"c:\windows\winsxs\",
        r"C:\Windows\WinSxS\amd64_microsoft_windows_manifest",
        // Boot & bootmgr
        r"C:\Windows\Boot",
        r"c:\windows\boot\",
        r"C:\bootmgr",
        r"c:\bootmgr",
        r"bootmgr",
        // Program Files\Windows NT
        r"C:\Program Files\Windows NT",
        r"c:\program files\windows nt\",
        r"c:\program files (x86)\windows nt",
        // Root drives & raw devices
        r"C:\",
        r"C:",
        r"\",
        r"\\.\PhysicalDrive0",
        r"\\.\C:",
        r"\\?\Volume{00000000-0000-0000-0000-000000000000}\",
        r"\??\PhysicalDrive0",
        // Verbatim prefix
        r"\\?\C:\Windows\System32",
        r"\\?\c:\windows\system32\",
        r"\\?\C:\Windows\WinSxS",
    ];

    for path_str in test_cases {
        let p = Path::new(path_str);
        assert!(
            cleaner::is_protected_system_path(p),
            "Expected '{}' to be flagged as protected system path!",
            path_str
        );
    }
}

#[test]
fn challenge_blacklist_personal_user_folders_matrix() {
    let user_paths = [
        r"C:\Users\JohnDoe\Documents",
        r"c:\users\johndoe\documents\",
        r"C:/Users/JohnDoe/Documents",
        r"C:\Users\JohnDoe\Desktop",
        r"C:\Users\JohnDoe\Pictures",
        r"C:\Users\JohnDoe\Music",
        r"C:\Users\JohnDoe\Videos",
        r"C:\Users\JohnDoe\OneDrive\Documents",
        r"C:\Users\JohnDoe\OneDrive\Desktop",
        r"C:\Users\Administrator\Documents\secret.docx",
        r"C:\Users\Public\Documents",
    ];

    for path_str in user_paths {
        let p = Path::new(path_str);
        assert!(
            cleaner::is_protected_system_path(p),
            "User personal path '{}' must be protected!",
            path_str
        );
    }
}

#[test]
fn challenge_blacklist_code_repository_matrix() {
    let current_dir = std::env::current_dir().expect("current dir");
    assert!(
        cleaner::is_protected_system_path(&current_dir),
        "Current repository dir '{}' must be recognized as protected code repository",
        current_dir.display()
    );

    let src_main = current_dir.join("src").join("main.rs");
    assert!(
        cleaner::is_protected_system_path(&src_main),
        "File inside repository '{}' must be protected",
        src_main.display()
    );

    let git_dir = current_dir.join(".git");
    assert!(
        cleaner::is_protected_system_path(&git_dir),
        ".git path must be protected"
    );
}

#[test]
fn challenge_wiper_validate_wipe_drive_root_adversarial_inputs() {
    let invalid_inputs = [
        "",
        " ",
        "   ",
        r"\\.\PhysicalDrive0",
        r"\\.\C:",
        r"\\?\Volume{12345}",
        r"\??\C:",
        r"C:\Windows",
        r"C:\Windows\System32",
        r"C:\Users",
        "CC:",
        "1:",
        "C::",
        "/",
        r"\",
        r"C:\Windows\System32\",
        r"..\..\Windows",
        r".\test",
    ];

    for input in invalid_inputs {
        assert!(
            wiper::validate_wipe_drive_root(input).is_err(),
            "Input '{input}' should be rejected by validate_wipe_drive_root"
        );
    }
}

// ============================================================================
// 4. ADVERSARIAL TRAVERSAL / CANONICALIZATION ATTACK SURFACE
// ============================================================================

#[test]
fn challenge_blacklist_dot_dot_traversal_boundaries() {
    // Attack scenario: Can relative traversal evade the blacklist?
    let dot_dot_cases = [
        r"C:\Windows\System32\..\System32",
        r"C:\Windows\SysWOW64\..\SysWOW64",
        r"C:\Windows\..\Windows\System32",
        r"C:\Users\JohnDoe\AppData\..\Documents",
        r"C:\Users\JohnDoe\..\JohnDoe\Desktop",
    ];

    for path_str in dot_dot_cases {
        let p = Path::new(path_str);
        let protected = cleaner::is_protected_system_path(p);
        println!("Checking dot-dot path '{path_str}': protected = {protected}");
    }
}
