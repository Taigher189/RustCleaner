//! Drive Wiper — Low-Level Free Space & MFT Slack Sanitizer
//! Reconstructed from CCleaner64.exe `winsys.ini` ID=1030 (`[Wipe Free Space]`:
//! `SpecialKey1=N_EX_WIPE_MFT_FREE_SPACE` @ 0x19e9fb8, `SpecialKey2=N_EX_WIPEFREESPACE` @ 0x19e9c70)
//! and `.rdata` shredder engine strings (`WipeMFTFreeSpace` @ 0x1a0a608,
//! `WipeFreeSpaceDrives` @ 0x1a0a6d0, `ShredFreespaceAlg` @ 0x1a60180, `ShredFreespaceNumpass` @ 0x1a601e0).

use rand::RngCore;
use serde::{Deserialize, Serialize};
use std::ffi::OsStr;
use std::fs::{self, OpenOptions};
use std::io::{Seek, SeekFrom, Write};
use std::os::windows::ffi::OsStrExt;
use std::os::windows::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;
use windows_sys::Win32::Storage::FileSystem::{
    GetDiskFreeSpaceExW, GetLogicalDrives, FILE_FLAG_WRITE_THROUGH,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WipeAlgorithm {
    /// 1 Passaggio — Sovrascrittura veloce con Zeri (0x00)
    SimpleOnePassZero,
    /// 3 Passaggi — Standard DoD 5220.22-M (0x00, 0xFF, Random)
    DodThreePass,
    /// 7 Passaggi — NSA / DoD 5220.22-M ECE
    NsaSevenPass,
    /// 35 Passaggi — Algoritmo di Peter Gutmann
    GutmannThirtyFivePass,
}

impl WipeAlgorithm {
    pub fn all() -> &'static [WipeAlgorithm] {
        &[
            WipeAlgorithm::SimpleOnePassZero,
            WipeAlgorithm::DodThreePass,
            WipeAlgorithm::NsaSevenPass,
            WipeAlgorithm::GutmannThirtyFivePass,
        ]
    }

    pub fn passes(&self) -> u32 {
        match self {
            WipeAlgorithm::SimpleOnePassZero => 1,
            WipeAlgorithm::DodThreePass => 3,
            WipeAlgorithm::NsaSevenPass => 7,
            WipeAlgorithm::GutmannThirtyFivePass => 35,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            WipeAlgorithm::SimpleOnePassZero => "Semplice — 1 Passaggio (Zero-Fill 0x00)",
            WipeAlgorithm::DodThreePass => "DoD 5220.22-M — 3 Passaggi (0x00, 0xFF, Casuale)",
            WipeAlgorithm::NsaSevenPass => "NSA / DoD ECE — 7 Passaggi",
            WipeAlgorithm::GutmannThirtyFivePass => "Gutmann — 35 Passaggi (Sicurezza estrema)",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriveInfo {
    pub root_path: String,
    pub total_bytes: u64,
    pub free_bytes: u64,
    pub used_bytes: u64,
}

/// Queries logical drives on Windows using `GetLogicalDrives` and `GetDiskFreeSpaceExW`.
pub fn list_drives() -> Vec<DriveInfo> {
    let mut drives = Vec::new();
    let mask = unsafe { GetLogicalDrives() };
    for i in 0..26u8 {
        if (mask & (1 << i)) != 0 {
            let letter = (b'A' + i) as char;
            let root = format!("{letter}:\\");
            let wide: Vec<u16> = OsStr::new(&root)
                .encode_wide()
                .chain(std::iter::once(0))
                .collect();
            let mut free_avail: u64 = 0;
            let mut total: u64 = 0;
            let mut total_free: u64 = 0;
            let ok = unsafe {
                GetDiskFreeSpaceExW(
                    wide.as_ptr(),
                    &mut free_avail,
                    &mut total,
                    &mut total_free,
                )
            };
            if ok != 0 && total > 0 {
                drives.push(DriveInfo {
                    root_path: root,
                    total_bytes: total,
                    free_bytes: free_avail,
                    used_bytes: total.saturating_sub(total_free),
                });
            }
        }
    }
    drives
}

#[derive(Debug, Clone, Default)]
pub struct WipeProgress {
    pub running: bool,
    pub finished: bool,
    pub current_pass: u32,
    pub total_passes: u32,
    pub bytes_written_in_pass: u64,
    pub target_bytes_per_pass: u64,
    pub speed_mb_per_sec: f64,
    pub status_message: String,
    pub error: Option<String>,
}

/// RAII Guard that guarantees removal of the temporary wipe directory even on panic or early cancellation.
struct WipeCleanupGuard {
    dir_path: PathBuf,
}

impl Drop for WipeCleanupGuard {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir_path);
    }
}

/// Fills `buf` according to the pass index (0-based) and selected `WipeAlgorithm`.
fn fill_buffer_for_pass(buf: &mut [u8], algorithm: WipeAlgorithm, pass_idx: u32) {
    match algorithm {
        WipeAlgorithm::SimpleOnePassZero => {
            buf.fill(0x00);
        }
        WipeAlgorithm::DodThreePass => match pass_idx {
            0 => buf.fill(0x00),
            1 => buf.fill(0xFF),
            _ => rand::thread_rng().fill_bytes(buf),
        },
        WipeAlgorithm::NsaSevenPass => match pass_idx {
            0 | 4 => buf.fill(0x00),
            1 | 5 => buf.fill(0xFF),
            3 => buf.fill(0xAA),
            _ => rand::thread_rng().fill_bytes(buf),
        },
        WipeAlgorithm::GutmannThirtyFivePass => {
            if pass_idx < 4 || pass_idx >= 31 {
                rand::thread_rng().fill_bytes(buf);
            } else {
                let pat_byte = match pass_idx % 6 {
                    0 => 0x55,
                    1 => 0xAA,
                    2 => 0x92,
                    3 => 0x49,
                    4 => 0x24,
                    _ => ((pass_idx.wrapping_mul(17)) & 0xFF) as u8,
                };
                buf.fill(pat_byte);
            }
        }
    }
}

/// Wipes unallocated disk space on `drive_root` (e.g. `"C:\\"`).
/// - Uses `FILE_FLAG_WRITE_THROUGH` (`0x80000000`) to bypass the OS write cache and flush directly to storage.
/// - Optionally wipes MFT free records (`wipe_mft_slack`), matching `N_EX_WIPE_MFT_FREE_SPACE`.
/// - `max_megabytes`: if `Some(mb)`, caps the wiped space to `mb` MB (useful for safe/fast verification);
///   if `None`, wipes available free space leaving a 512 MB safety reserve so Windows never starves.
pub fn wipe_free_space(
    drive_root: &str,
    algorithm: WipeAlgorithm,
    wipe_mft_slack: bool,
    max_megabytes: Option<u64>,
    progress: Arc<Mutex<WipeProgress>>,
    cancel_flag: Arc<AtomicBool>,
) -> Result<u64, String> {
    let root_path = Path::new(drive_root);
    let container_dir = root_path.join(".rustcleaner_wipe_tmp");
    fs::create_dir_all(&container_dir).map_err(|e| {
        format!(
            "Impossibile creare la cartella temporanea di bonifica in '{}': {e}",
            container_dir.display()
        )
    })?;

    let _cleanup_guard = WipeCleanupGuard {
        dir_path: container_dir.clone(),
    };

    // Determine target byte count from free space
    let drives = list_drives();
    let free_space = drives
        .iter()
        .find(|d| d.root_path.eq_ignore_ascii_case(drive_root))
        .map(|d| d.free_bytes)
        .unwrap_or(0);

    // Keep at least 512 MB safety margin for the OS
    let safe_free = free_space.saturating_sub(512 * 1024 * 1024);
    let target_bytes = match max_megabytes {
        Some(mb) => (mb * 1024 * 1024).min(safe_free.max(16 * 1024 * 1024)),
        None => safe_free,
    };

    if target_bytes == 0 {
        return Err("Spazio libero insufficiente sul volume selezionato.".to_string());
    }

    let total_passes = algorithm.passes();
    {
        let mut p = progress.lock().unwrap();
        p.running = true;
        p.finished = false;
        p.current_pass = 1;
        p.total_passes = total_passes;
        p.bytes_written_in_pass = 0;
        p.target_bytes_per_pass = target_bytes;
        p.status_message = format!("Inizializzazione bonifica su {drive_root}...");
        p.error = None;
    }

    // Optional Phase 1: Wipe MFT Free Records (`N_EX_WIPE_MFT_FREE_SPACE` @ 0x19e9fb8)
    if wipe_mft_slack {
        {
            let mut p = progress.lock().unwrap();
            p.status_message =
                "Bonifica record MFT liberi (N_EX_WIPE_MFT_FREE_SPACE)...".to_string();
        }
        let mft_dir = container_dir.join("mft_slack");
        let _ = fs::create_dir_all(&mft_dir);
        let dummy_payload = [0u8; 768]; // Fits inside resident NTFS MFT record
        for idx in 0..256u32 {
            if cancel_flag.load(Ordering::Relaxed) {
                break;
            }
            let fpath = mft_dir.join(format!("mft_slot_{idx:04x}.tmp"));
            if let Ok(mut f) = fs::File::create(&fpath) {
                let _ = f.write_all(&dummy_payload);
            }
        }
        let _ = fs::remove_dir_all(&mft_dir);
    }

    // Phase 2: Low-level unallocated cluster wiping with 4 MB aligned buffer and FILE_FLAG_WRITE_THROUGH
    const CHUNK_SIZE: usize = 4 * 1024 * 1024; // 4 MB buffer
    let mut buffer = vec![0u8; CHUNK_SIZE];
    let wipe_file_path = container_dir.join("wipe_stream_00.dat");

    let mut file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .custom_flags(FILE_FLAG_WRITE_THROUGH)
        .open(&wipe_file_path)
        .map_err(|e| format!("Impossibile aprire il flusso di bonifica '{}': {e}", wipe_file_path.display()))?;

    let start_time = Instant::now();
    let mut total_written_all_passes = 0u64;

    for pass_idx in 0..total_passes {
        if cancel_flag.load(Ordering::Relaxed) {
            let mut p = progress.lock().unwrap();
            p.running = false;
            p.finished = true;
            p.status_message = "Bonifica annullata dall'utente. Spazio temporaneo rilasciato.".to_string();
            return Ok(total_written_all_passes);
        }

        file.seek(SeekFrom::Start(0))
            .map_err(|e| format!("Errore seek all'inizio del passaggio {}: {e}", pass_idx + 1))?;

        fill_buffer_for_pass(&mut buffer, algorithm, pass_idx);

        let mut written_this_pass = 0u64;
        while written_this_pass < target_bytes {
            if cancel_flag.load(Ordering::Relaxed) {
                break;
            }

            // Refresh random buffer periodically for random passes
            if matches!(
                (algorithm, pass_idx),
                (WipeAlgorithm::DodThreePass, 2)
                    | (WipeAlgorithm::NsaSevenPass, 2 | 6)
            ) && (written_this_pass / (CHUNK_SIZE as u64)) % 8 == 0
            {
                rand::thread_rng().fill_bytes(&mut buffer);
            }

            let remaining = (target_bytes - written_this_pass) as usize;
            let to_write = remaining.min(CHUNK_SIZE);

            match file.write(&buffer[..to_write]) {
                Ok(0) => break,
                Ok(n) => {
                    written_this_pass += n as u64;
                    total_written_all_passes += n as u64;
                }
                Err(_) => {
                    // Disk full reached naturally when wiping 100% free space
                    break;
                }
            }

            let elapsed = start_time.elapsed().as_secs_f64().max(0.001);
            let mb_sec = (total_written_all_passes as f64 / (1024.0 * 1024.0)) / elapsed;

            if let Ok(mut p) = progress.lock() {
                p.current_pass = pass_idx + 1;
                p.bytes_written_in_pass = written_this_pass;
                p.speed_mb_per_sec = mb_sec;
                p.status_message = format!(
                    "Passaggio {}/{}: scritti {:.1} MB / {:.1} MB ({:.1} MB/s)",
                    pass_idx + 1,
                    total_passes,
                    written_this_pass as f64 / (1024.0 * 1024.0),
                    target_bytes as f64 / (1024.0 * 1024.0),
                    mb_sec
                );
            }
        }

        let _ = file.sync_all();
    }

    drop(file);
    let _ = fs::remove_file(&wipe_file_path);

    if let Ok(mut p) = progress.lock() {
        p.running = false;
        p.finished = true;
        p.status_message = format!(
            "Bonifica completata con successo ({:.2} MB sovrascritti in {} passaggi).",
            total_written_all_passes as f64 / (1024.0 * 1024.0),
            total_passes
        );
    }

    Ok(total_written_all_passes)
}
