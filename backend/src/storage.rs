use std::fs::File;
use std::io::{Read, Write, Result};
use std::path::Path;
use fs2::FileExt;
use sysinfo::Disks;
use walkdir::WalkDir;
use serde::{Serialize, de::DeserializeOwned};
use tempfile::NamedTempFile;

use crate::models::DiskStats;

// --- File I/O ---

pub fn read_json<T: DeserializeOwned>(path: &str) -> Result<T> {
    let mut file = File::open(path)?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;
    let data = serde_json::from_str(&contents)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    Ok(data)
}

pub fn atomic_write_json<T: Serialize>(path: &str, data: &T) -> Result<()> {
    let dir = Path::new(path).parent().unwrap_or_else(|| Path::new("."));
    let mut tmp = NamedTempFile::new_in(dir)?;
    
    let json = serde_json::to_string_pretty(data)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    
    tmp.write_all(json.as_bytes())?;
    tmp.flush()?;
    tmp.as_file().sync_all()?;
    
    tmp.persist(path).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
    Ok(())
}

pub struct FileLock {
    file: File,
}

impl FileLock {
    pub fn new(path: &str) -> Result<Self> {
        let lock_path = format!("{}.lock", path);
        let file = File::create(lock_path)?;
        file.lock_exclusive()?;
        Ok(Self { file })
    }
}

impl Drop for FileLock {
    fn drop(&mut self) {
        let _ = self.file.unlock();
    }
}

// --- Disk Logic ---

pub fn calculate_dir_size(path: &str) -> u64 {
    WalkDir::new(path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter_map(|e| e.metadata().ok())
        .filter(|m| m.is_file())
        .map(|m| m.len())
        .sum()
}

pub fn get_disk_usage(workspace_path: &str) -> Option<DiskStats> {
    let disks = Disks::new_with_refreshed_list();
    if let Some(root) = disks.iter().find(|d| d.mount_point() == Path::new("/")) {
        let total = root.total_space();
        let available = root.available_space();
        let used = total - available;
        let usage_pct = (used as f64 / total as f64) * 100.0;
        
        let workspace_size = calculate_dir_size(workspace_path);

        Some(DiskStats {
            total,
            used,
            free: available,
            usage_pct,
            workspace_size,
        })
    } else {
        None
    }
}

// --- Cleanup Logic ---

pub fn delete_path(path_str: &str) -> std::io::Result<()> {
    // Safety check: ensure we are only deleting from safe paths
    if !path_str.contains(".gemini") {
        return Err(std::io::Error::new(std::io::ErrorKind::PermissionDenied, "Unsafe path"));
    }

    let path = Path::new(path_str);
    if path.is_file() {
        std::fs::remove_file(path)
    } else {
        std::fs::remove_dir_all(path)
    }
}

