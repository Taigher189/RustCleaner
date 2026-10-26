//! Common test utilities, mock fixtures, and registry test hives for RustCleaner E2E tests.

use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};
use winreg::enums::{HKEY_CURRENT_USER, KEY_ALL_ACCESS, KEY_READ, KEY_WRITE};
use winreg::RegKey;

static TEST_COUNTER: AtomicU64 = AtomicU64::new(1);

/// RAII Temporary Directory that cleans up upon Drop.
pub struct TempTestDir {
    pub path: PathBuf,
}

impl TempTestDir {
    pub fn new(prefix: &str) -> Self {
        let id = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
        let pid = std::process::id();
        let mut p = std::env::temp_dir();
        p.push(format!("rc_test_{}_{}_{}", prefix, pid, id));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).expect("Failed to create temp test dir");
        Self { path: p }
    }

    pub fn create_file(&self, relative_path: &str, content: &[u8]) -> PathBuf {
        let fpath = self.path.join(relative_path);
        if let Some(parent) = fpath.parent() {
            fs::create_dir_all(parent).expect("Failed to create parent dirs");
        }
        let mut f = File::create(&fpath).expect("Failed to create file");
        f.write_all(content).expect("Failed to write test file");
        fpath
    }

    pub fn create_readonly_file(&self, relative_path: &str, content: &[u8]) -> PathBuf {
        let fpath = self.create_file(relative_path, content);
        let mut perms = fs::metadata(&fpath).unwrap().permissions();
        perms.set_readonly(true);
        fs::set_permissions(&fpath, perms).expect("Failed to set readonly permissions");
        fpath
    }
}

impl Drop for TempTestDir {
    #[allow(clippy::permissions_set_readonly_false)]
    fn drop(&mut self) {
        if self.path.exists() {
            // Remove readonly flags if any before deleting
            for entry in walkdir::WalkDir::new(&self.path).into_iter().flatten() {
                if let Ok(meta) = entry.metadata() {
                    let mut perms = meta.permissions();
                    if perms.readonly() {
                        perms.set_readonly(false);
                        let _ = fs::set_permissions(entry.path(), perms);
                    }
                }
            }
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}

use std::os::windows::fs::OpenOptionsExt;

/// Holds an exclusively locked file handle to test sharing violation handling.
#[allow(dead_code)]
pub struct LockedFileGuard {
    pub path: PathBuf,
    pub file: File,
}

impl LockedFileGuard {
    pub fn new(path: &Path) -> Result<Self, std::io::Error> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .share_mode(0) // Exclusive access to trigger sharing violation on Windows
            .open(path)?;
        Ok(Self {
            path: path.to_path_buf(),
            file,
        })
    }
}

/// RAII Temporary Registry Hive created under HKCU\Software for rollback and scanning tests.
#[allow(dead_code)]
pub struct RegistryTestHive {
    pub subkey_name: String,
    pub full_key_path: String,
}

impl RegistryTestHive {
    pub fn new(prefix: &str) -> Result<Self, String> {
        let id = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
        let pid = std::process::id();
        let subkey_name = format!(r"Software\RustCleaner_Test_{}_{}_{}", prefix, pid, id);
        let full_key_path = format!(r"HKEY_CURRENT_USER\{}", subkey_name);

        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let (key, _) = hkcu
            .create_subkey_with_flags(&subkey_name, KEY_ALL_ACCESS)
            .map_err(|e| format!("Failed to create test reg subkey '{subkey_name}': {e}"))?;

        // Initialize dummy value
        key.set_value("TestInit", &"Initialized")
            .map_err(|e| format!("Failed to set init value: {e}"))?;

        Ok(Self {
            subkey_name,
            full_key_path,
        })
    }

    pub fn set_str_value(&self, name: &str, val: &str) -> Result<(), String> {
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let key = hkcu
            .open_subkey_with_flags(&self.subkey_name, KEY_WRITE)
            .map_err(|e| e.to_string())?;
        key.set_value(name, &val).map_err(|e| e.to_string())
    }

    #[allow(dead_code)]
    pub fn set_dword_value(&self, name: &str, val: u32) -> Result<(), String> {
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let key = hkcu
            .open_subkey_with_flags(&self.subkey_name, KEY_WRITE)
            .map_err(|e| e.to_string())?;
        key.set_value(name, &val).map_err(|e| e.to_string())
    }

    pub fn get_str_value(&self, name: &str) -> Result<String, String> {
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let key = hkcu
            .open_subkey_with_flags(&self.subkey_name, KEY_READ)
            .map_err(|e| e.to_string())?;
        key.get_value(name).map_err(|e| e.to_string())
    }

    pub fn delete_value(&self, name: &str) -> Result<(), String> {
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let key = hkcu
            .open_subkey_with_flags(&self.subkey_name, KEY_WRITE)
            .map_err(|e| e.to_string())?;
        key.delete_value(name).map_err(|e| e.to_string())
    }
}

impl Drop for RegistryTestHive {
    fn drop(&mut self) {
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let _ = hkcu.delete_subkey_all(&self.subkey_name);
    }
}

/// Invokes the compiled RustCleaner binary with the specified arguments.
pub fn run_cli(args: &[&str]) -> Output {
    let bin_path = env!("CARGO_BIN_EXE_rustcleaner");
    Command::new(bin_path)
        .args(args)
        .output()
        .unwrap_or_else(|e| panic!("Failed to run binary '{bin_path}' with args {args:?}: {e}"))
}
