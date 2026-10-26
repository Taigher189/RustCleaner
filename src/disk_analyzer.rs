//! High-Performance Disk Space Analyzer, Large File Hunter & Multi-Stage SHA-256 Duplicate Finder.
//!
//! Architecture:
//! 1. **Parallel Directory Traversal**: Walks target directories while skipping Windows reparse points,
//!    system junction loops, and protected OS pseudo-folders (`$Recycle.Bin`, `System Volume Information`, `WinSxS`).
//! 2. **Category Space Map**: Aggregates total bytes and file counts across 6 semantic file categories
//!    (Video/Media, Archives/ISOs, Executables/Installers, Dev/VM Artifacts, Documents/Images, Logs/Dumps/Other).
//! 3. **Top Large Files ("Space Hogs")**: Tracks files exceeding `min_large_file_bytes` (e.g. >= 50 MB)
//!    sorted largest-first.
//! 4. **3-Stage Zero-Waste Duplicate Engine**:
//!    - **Stage 1 (Size Grouping)**: Groups files by exact byte size (`>= min_dup_bytes`). Any file with a
//!      unique size is immediately discarded without disk I/O.
//!    - **Stage 2 (8 KB Boundary Fingerprint)**: Hashes the first 4 KB + last 4 KB of same-size files in
//!      parallel (`rayon`). Files that differ at the header/footer boundary are discarded without reading
//!      the middle of the file.
//!    - **Stage 3 (Full FIPS 180-4 SHA-256 Digest)**: Computes the full cryptographic SHA-256 hash in
//!      parallel (`rayon`) only for files that matched both exact size and boundary fingerprint.

use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use walkdir::WalkDir;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FileCategory {
    VideoMedia,
    ArchivesDiskImages,
    ExecutablesInstallers,
    DevVmArtifacts,
    DocumentsPictures,
    LogsDumpsOther,
}

impl FileCategory {
    pub fn all() -> &'static [FileCategory] {
        &[
            FileCategory::VideoMedia,
            FileCategory::ArchivesDiskImages,
            FileCategory::ExecutablesInstallers,
            FileCategory::DevVmArtifacts,
            FileCategory::DocumentsPictures,
            FileCategory::LogsDumpsOther,
        ]
    }

    pub fn from_path(path: &Path) -> Self {
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();

        match ext.as_str() {
            "mp4" | "mkv" | "avi" | "mov" | "wmv" | "flv" | "webm" | "m4v" | "mp3" | "wav"
            | "flac" | "aac" | "ogg" | "m4a" | "opus" => FileCategory::VideoMedia,

            "zip" | "rar" | "7z" | "tar" | "gz" | "bz2" | "xz" | "zst" | "iso" | "img"
            | "vhd" | "vhdx" | "wim" | "esd" | "cab" => FileCategory::ArchivesDiskImages,

            "exe" | "msi" | "msix" | "appx" | "appxbundle" | "dll" | "sys" | "ocx" | "bat"
            | "cmd" | "ps1" => FileCategory::ExecutablesInstallers,

            "rlib" | "rmeta" | "pdb" | "obj" | "o" | "lib" | "a" | "so" | "dylib" | "vmdk"
            | "vdi" | "qcow2" | "OVA" | "ova" | "pack" | "idx" | "asar" | "node" | "pyc"
            | "class" | "jar" | "nupkg" => FileCategory::DevVmArtifacts,

            "pdf" | "doc" | "docx" | "xls" | "xlsx" | "ppt" | "pptx" | "odt" | "ods" | "txt"
            | "md" | "csv" | "json" | "xml" | "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp"
            | "svg" | "ico" | "tiff" | "psd" | "ai" | "raw" | "heic" => {
                FileCategory::DocumentsPictures
            }

            _ => FileCategory::LogsDumpsOther,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CategoryStat {
    pub category: FileCategory,
    pub file_count: usize,
    pub total_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LargeFileEntry {
    pub path: PathBuf,
    pub size_bytes: u64,
    pub category: FileCategory,
    pub selected: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DuplicateFileItem {
    pub path: PathBuf,
    pub selected_for_deletion: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DuplicateGroup {
    pub sha256_hex: String,
    pub each_size_bytes: u64,
    pub wasted_bytes: u64,
    pub files: Vec<DuplicateFileItem>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DiskAnalysisReport {
    pub target_root: String,
    pub total_files_scanned: usize,
    pub total_bytes_scanned: u64,
    pub categories: Vec<CategoryStat>,
    pub large_files: Vec<LargeFileEntry>,
    pub duplicate_groups: Vec<DuplicateGroup>,
    pub total_duplicate_wasted_bytes: u64,
    pub total_duplicate_extra_files: usize,
    pub status_message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScanTargetPreset {
    UserProfileMediaAndDocs,
    DownloadsFolder,
    DocumentsAndDesktop,
    SystemDriveRoot,
    CustomPath,
}

/// Resolves the list of directories to scan for a given preset or custom path.
pub fn resolve_preset_paths(preset: ScanTargetPreset, custom_path: &str) -> Vec<PathBuf> {
    let user_profile = env::var("USERPROFILE").unwrap_or_else(|_| r"C:\Users\Default".to_string());
    let up = PathBuf::from(&user_profile);

    match preset {
        ScanTargetPreset::UserProfileMediaAndDocs => {
            let candidates = [
                up.join("Downloads"),
                up.join("Documents"),
                up.join("Desktop"),
                up.join("Videos"),
                up.join("Pictures"),
                up.join("Music"),
            ];
            let existing: Vec<PathBuf> = candidates.into_iter().filter(|p| p.exists()).collect();
            if existing.is_empty() {
                vec![up]
            } else {
                existing
            }
        }
        ScanTargetPreset::DownloadsFolder => {
            let dl = up.join("Downloads");
            if dl.exists() {
                vec![dl]
            } else {
                vec![up]
            }
        }
        ScanTargetPreset::DocumentsAndDesktop => {
            let mut v = Vec::new();
            let docs = up.join("Documents");
            let desk = up.join("Desktop");
            if docs.exists() {
                v.push(docs);
            }
            if desk.exists() {
                v.push(desk);
            }
            if v.is_empty() {
                v.push(up);
            }
            v
        }
        ScanTargetPreset::SystemDriveRoot => {
            let sys_drive = env::var("SystemDrive").unwrap_or_else(|_| "C:".to_string());
            vec![PathBuf::from(format!(r"{sys_drive}\"))]
        }
        ScanTargetPreset::CustomPath => {
            let expanded = crate::cleaner::expand_env_vars(custom_path.trim());
            vec![expanded]
        }
    }
}

/// Checks if a directory entry should be skipped during disk analysis to avoid OS pseudo-filesystems,
/// WinSxS hardlink storms, or permission traps.
fn should_skip_dir(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
        return false;
    };
    let lower = name.to_ascii_lowercase();
    matches!(
        lower.as_str(),
        "$recycle.bin"
            | "system volume information"
            | "winsxs"
            | "servicing"
            | "csc"
            | "node_modules"
            | ".git"
    )
}

/// Executes a full multi-threaded Disk Space Analysis, Large File Hunt, and 3-Stage SHA-256 Duplicate Scan.
pub fn run_disk_analysis(
    roots: &[PathBuf],
    min_large_file_mb: u64,
    min_duplicate_kb: u64,
    cancel_flag: Arc<AtomicBool>,
) -> DiskAnalysisReport {
    let min_large_bytes = min_large_file_mb.saturating_mul(1024 * 1024);
    let min_dup_bytes = min_duplicate_kb.saturating_mul(1024).max(1024); // Minimum 1 KB for duplicates

    let mut total_files_scanned: usize = 0;
    let mut total_bytes_scanned: u64 = 0;
    let mut cat_map: HashMap<FileCategory, (usize, u64)> = HashMap::new();
    for &c in FileCategory::all() {
        cat_map.insert(c, (0, 0));
    }

    let mut large_files: Vec<LargeFileEntry> = Vec::new();
    let mut size_buckets: HashMap<u64, Vec<PathBuf>> = HashMap::new();

    for root in roots {
        if cancel_flag.load(Ordering::Relaxed) {
            break;
        }
        if !root.exists() {
            continue;
        }

        let walker = WalkDir::new(root)
            .follow_links(false)
            .max_depth(24)
            .into_iter()
            .filter_entry(|e| {
                if e.file_type().is_dir() {
                    !should_skip_dir(e.path())
                } else {
                    true
                }
            });

        for entry in walker.flatten() {
            if cancel_flag.load(Ordering::Relaxed) {
                break;
            }
            if !entry.file_type().is_file() {
                continue;
            }
            let Ok(meta) = entry.metadata() else {
                continue;
            };
            let size = meta.len();
            let path = entry.path().to_path_buf();

            total_files_scanned += 1;
            total_bytes_scanned = total_bytes_scanned.saturating_add(size);

            let cat = FileCategory::from_path(&path);
            if let Some(stat) = cat_map.get_mut(&cat) {
                stat.0 += 1;
                stat.1 = stat.1.saturating_add(size);
            }

            if size >= min_large_bytes {
                large_files.push(LargeFileEntry {
                    path: path.clone(),
                    size_bytes: size,
                    category: cat,
                    selected: false,
                });
            }

            if size >= min_dup_bytes {
                size_buckets.entry(size).or_default().push(path);
            }
        }
    }

    // Sort large files largest-first and keep top 250
    large_files.sort_by_key(|a| std::cmp::Reverse(a.size_bytes));
    if large_files.len() > 250 {
        large_files.truncate(250);
    }

    // Build sorted category stats
    let mut categories: Vec<CategoryStat> = FileCategory::all()
        .iter()
        .map(|&cat| {
            let (file_count, total_bytes) = cat_map.get(&cat).copied().unwrap_or((0, 0));
            CategoryStat {
                category: cat,
                file_count,
                total_bytes,
            }
        })
        .collect();
    categories.sort_by_key(|a| std::cmp::Reverse(a.total_bytes));

    // === 3-STAGE DUPLICATE FINDER ===
    // Stage 1: Keep only size buckets with >= 2 files
    let candidate_size_groups: Vec<(u64, Vec<PathBuf>)> = size_buckets
        .into_iter()
        .filter(|(_, paths)| paths.len() >= 2)
        .collect();

    // Stage 2: Compute fast 8 KB boundary fingerprint (first 4 KB + last 4 KB) in parallel
    // and keep only sub-groups where >= 2 files share the exact boundary fingerprint.
    let full_hash_candidates: Vec<(u64, Vec<PathBuf>)> = candidate_size_groups
        .into_par_iter()
        .flat_map(|(size, paths)| {
            if cancel_flag.load(Ordering::Relaxed) {
                return Vec::new();
            }
            let mut fp_buckets: HashMap<u64, Vec<PathBuf>> = HashMap::new();
            for p in paths {
                if let Some(fp) = compute_boundary_fingerprint(&p, size) {
                    fp_buckets.entry(fp).or_default().push(p);
                }
            }
            fp_buckets
                .into_values()
                .filter_map(|grp| if grp.len() >= 2 { Some((size, grp)) } else { None })
                .collect::<Vec<_>>()
        })
        .collect();

    // Stage 3: Full SHA-256 digest in parallel for files matching both size and boundary fingerprint
    let mut duplicate_groups: Vec<DuplicateGroup> = full_hash_candidates
        .into_par_iter()
        .flat_map(|(size, paths)| {
            if cancel_flag.load(Ordering::Relaxed) {
                return Vec::new();
            }
            let mut sha_buckets: HashMap<String, Vec<PathBuf>> = HashMap::new();
            for p in paths {
                if cancel_flag.load(Ordering::Relaxed) {
                    break;
                }
                if let Some(digest) = compute_file_sha256_hex(&p) {
                    sha_buckets.entry(digest).or_default().push(p);
                }
            }
            sha_buckets
                .into_iter()
                .filter_map(|(sha256_hex, mut files_vec)| {
                    if files_vec.len() >= 2 {
                        files_vec.sort();
                        let extra_copies = (files_vec.len() - 1) as u64;
                        let wasted_bytes = size.saturating_mul(extra_copies);
                        let items = files_vec
                            .into_iter()
                            .enumerate()
                            .map(|(idx, path)| DuplicateFileItem {
                                path,
                                // By default, keep index 0 unselected (original) and do not pre-select until user clicks "Auto-Select" or checks manually
                                selected_for_deletion: idx > 0 && false,
                            })
                            .collect();
                        Some(DuplicateGroup {
                            sha256_hex,
                            each_size_bytes: size,
                            wasted_bytes,
                            files: items,
                        })
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>()
        })
        .collect();

    // Sort duplicate groups by wasted space descending
    duplicate_groups.sort_by_key(|a| std::cmp::Reverse(a.wasted_bytes));
    if duplicate_groups.len() > 250 {
        duplicate_groups.truncate(250);
    }

    let total_duplicate_wasted_bytes: u64 =
        duplicate_groups.iter().map(|g| g.wasted_bytes).sum();
    let total_duplicate_extra_files: usize = duplicate_groups
        .iter()
        .map(|g| g.files.len().saturating_sub(1))
        .sum();

    let target_root = roots
        .iter()
        .map(|p| p.to_string_lossy().to_string())
        .collect::<Vec<_>>()
        .join("; ");

    let status_message = format!(
        "Scansionati {} file ({}) • {} file giganti • {} gruppi di duplicati ({} recuperabili)",
        total_files_scanned,
        crate::cleaner::format_bytes(total_bytes_scanned),
        large_files.len(),
        duplicate_groups.len(),
        crate::cleaner::format_bytes(total_duplicate_wasted_bytes)
    );

    DiskAnalysisReport {
        target_root,
        total_files_scanned,
        total_bytes_scanned,
        categories,
        large_files,
        duplicate_groups,
        total_duplicate_wasted_bytes,
        total_duplicate_extra_files,
        status_message,
    }
}

/// Computes a fast 64-bit FNV-1a fingerprint over the first 4 KB and last 4 KB of a file.
fn compute_boundary_fingerprint(path: &Path, size: u64) -> Option<u64> {
    let mut file = File::open(path).ok()?;
    let mut hash: u64 = 0xcbf29ce484222325;
    let mut buf = [0u8; 4096];

    let n1 = file.read(&mut buf).ok()?;
    for &b in &buf[..n1] {
        hash ^= b as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }

    if size > 4096 {
        let seek_pos = size.saturating_sub(4096);
        file.seek(SeekFrom::Start(seek_pos)).ok()?;
        let n2 = file.read(&mut buf).ok()?;
        for &b in &buf[..n2] {
            hash ^= b as u64;
            hash = hash.wrapping_mul(0x100000001b3);
        }
    }

    Some(hash)
}

// ============================================================================
// PURE-RUST FIPS 180-4 SHA-256 IMPLEMENTATION (Zero External Crypto Crates)
// ============================================================================
const K256: [u32; 64] = [
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
    0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
    0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
    0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
    0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
    0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
    0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
    0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
    0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
    0xc67178f2,
];

pub struct Sha256 {
    state: [u32; 8],
    buffer: [u8; 64],
    buffer_len: usize,
    total_len: u64,
}

impl Default for Sha256 {
    fn default() -> Self {
        Self::new()
    }
}

impl Sha256 {
    pub fn new() -> Self {
        Self {
            state: [
                0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c,
                0x1f83d9ab, 0x5be0cd19,
            ],
            buffer: [0u8; 64],
            buffer_len: 0,
            total_len: 0,
        }
    }

    pub fn update(&mut self, mut data: &[u8]) {
        self.total_len = self.total_len.wrapping_add(data.len() as u64);

        if self.buffer_len > 0 {
            let needed = 64 - self.buffer_len;
            if data.len() < needed {
                self.buffer[self.buffer_len..self.buffer_len + data.len()].copy_from_slice(data);
                self.buffer_len += data.len();
                return;
            } else {
                self.buffer[self.buffer_len..64].copy_from_slice(&data[..needed]);
                let block = self.buffer;
                self.compress_block(&block);
                self.buffer_len = 0;
                data = &data[needed..];
            }
        }

        while data.len() >= 64 {
            let mut block = [0u8; 64];
            block.copy_from_slice(&data[..64]);
            self.compress_block(&block);
            data = &data[64..];
        }

        if !data.is_empty() {
            self.buffer[..data.len()].copy_from_slice(data);
            self.buffer_len = data.len();
        }
    }

    pub fn finalize_hex(mut self) -> String {
        let bit_len = self.total_len.wrapping_mul(8);
        self.buffer[self.buffer_len] = 0x80;
        self.buffer_len += 1;

        if self.buffer_len > 56 {
            for b in &mut self.buffer[self.buffer_len..64] {
                *b = 0;
            }
            let block = self.buffer;
            self.compress_block(&block);
            self.buffer_len = 0;
        }

        for b in &mut self.buffer[self.buffer_len..56] {
            *b = 0;
        }
        self.buffer[56..64].copy_from_slice(&bit_len.to_be_bytes());
        let final_block = self.buffer;
        self.compress_block(&final_block);

        let mut out = String::with_capacity(64);
        for word in &self.state {
            use std::fmt::Write;
            let _ = write!(&mut out, "{word:08x}");
        }
        out
    }

    fn compress_block(&mut self, block: &[u8; 64]) {
        let mut w = [0u32; 64];
        #[allow(clippy::chunks_exact_to_as_chunks)]
        for (i, chunk) in block.chunks_exact(4).enumerate() {
            w[i] = u32::from_be_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }

        let mut a = self.state[0];
        let mut b = self.state[1];
        let mut c = self.state[2];
        let mut d = self.state[3];
        let mut e = self.state[4];
        let mut f = self.state[5];
        let mut g = self.state[6];
        let mut h = self.state[7];

        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let temp1 = h
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K256[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let temp2 = s0.wrapping_add(maj);

            h = g;
            g = f;
            f = e;
            e = d.wrapping_add(temp1);
            d = c;
            c = b;
            b = a;
            a = temp1.wrapping_add(temp2);
        }

        self.state[0] = self.state[0].wrapping_add(a);
        self.state[1] = self.state[1].wrapping_add(b);
        self.state[2] = self.state[2].wrapping_add(c);
        self.state[3] = self.state[3].wrapping_add(d);
        self.state[4] = self.state[4].wrapping_add(e);
        self.state[5] = self.state[5].wrapping_add(f);
        self.state[6] = self.state[6].wrapping_add(g);
        self.state[7] = self.state[7].wrapping_add(h);
    }
}

/// Streams a file in 64 KB chunks and returns its lowercase 64-character SHA-256 hex digest.
pub fn compute_file_sha256_hex(path: &Path) -> Option<String> {
    let mut file = File::open(path).ok()?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 65536];
    loop {
        let n = file.read(&mut buf).ok()?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Some(hasher.finalize_hex())
}

/// Opens Windows File Explorer highlighting the specified file (`explorer.exe /select,"<path>"`).
pub fn reveal_in_explorer(path: &Path) -> Result<(), String> {
    let p_str = path.to_string_lossy().to_string();
    Command::new("explorer.exe")
        .arg(format!("/select,{p_str}"))
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("Impossibile aprire Esplora File: {e}"))
}

/// Deletes a file from disk (clearing the Read-Only attribute first if needed).
pub fn delete_analyzed_file(path: &Path) -> Result<(), String> {
    if let Ok(meta) = fs::metadata(path) {
        let mut perms = meta.permissions();
        #[cfg_attr(windows, allow(clippy::permissions_set_readonly_false))]
        if perms.readonly() {
            perms.set_readonly(false);
            let _ = fs::set_permissions(path, perms);
        }
    }
    fs::remove_file(path).map_err(|e| format!("Impossibile eliminare '{}': {e}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sha256_nist_vectors() {
        let mut h_empty = Sha256::new();
        h_empty.update(b"");
        assert_eq!(
            h_empty.finalize_hex(),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );

        let mut h_abc = Sha256::new();
        h_abc.update(b"abc");
        assert_eq!(
            h_abc.finalize_hex(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn test_file_category_detection() {
        assert_eq!(
            FileCategory::from_path(Path::new("movie.mkv")),
            FileCategory::VideoMedia
        );
        assert_eq!(
            FileCategory::from_path(Path::new("ubuntu.iso")),
            FileCategory::ArchivesDiskImages
        );
        assert_eq!(
            FileCategory::from_path(Path::new("setup.exe")),
            FileCategory::ExecutablesInstallers
        );
        assert_eq!(
            FileCategory::from_path(Path::new("vm.vmdk")),
            FileCategory::DevVmArtifacts
        );
        assert_eq!(
            FileCategory::from_path(Path::new("report.pdf")),
            FileCategory::DocumentsPictures
        );
    }

    #[test]
    fn test_duplicate_detection_in_temp_dir() {
        let tmp = env::temp_dir().join("rustcleaner_dup_test_dir");
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp).unwrap();

        let payload = vec![0x42u8; 4096];
        let f1 = tmp.join("copy_a.bin");
        let f2 = tmp.join("copy_b.bin");
        let f3 = tmp.join("unique.bin");

        fs::write(&f1, &payload).unwrap();
        fs::write(&f2, &payload).unwrap();
        fs::write(&f3, vec![0x99u8; 4096]).unwrap();

        let cancel = Arc::new(AtomicBool::new(false));
        let rep = run_disk_analysis(std::slice::from_ref(&tmp), 100, 1, cancel);

        assert_eq!(rep.duplicate_groups.len(), 1);
        assert_eq!(rep.duplicate_groups[0].files.len(), 2);
        assert_eq!(rep.duplicate_groups[0].wasted_bytes, 4096);

        let _ = fs::remove_dir_all(&tmp);
    }
}
