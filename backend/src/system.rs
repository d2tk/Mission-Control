use std::process::Command;
use std::io::{Result, Error, ErrorKind};

#[cfg(target_os = "linux")]
pub fn clean_pacman_cache() -> Result<()> {
    let status = Command::new("sudo")
        .arg("pacman")
        .arg("-Sc")
        .arg("--noconfirm")
        .status()?;

    if status.success() {
        Ok(())
    } else {
        Err(Error::new(ErrorKind::Other, "Pacman cleanup failed"))
    }
}

#[cfg(not(target_os = "linux"))]
pub fn clean_pacman_cache() -> Result<()> {
    Err(Error::new(ErrorKind::Unsupported, "Only supported on Linux"))
}

#[cfg(target_os = "linux")]
pub fn vacuum_system_journal() -> Result<()> {
    let status = Command::new("sudo")
        .arg("journalctl")
        .arg("--vacuum-time=7d")
        .status()?;

    if status.success() {
        Ok(())
    } else {
        Err(Error::new(ErrorKind::Other, "Journal cleanup failed"))
    }
}

#[cfg(not(target_os = "linux"))]
pub fn vacuum_system_journal() -> Result<()> {
    Err(Error::new(ErrorKind::Unsupported, "Only supported on Linux"))
}

// --- Cleanup Logic ---

use crate::models::CleanupItem;
use walkdir::WalkDir;
use std::time::{SystemTime, UNIX_EPOCH};
use std::path::Path;
use crate::storage::{calculate_dir_size, delete_path};

pub fn scan_cleanup_candidates() -> Vec<CleanupItem> {
    let mut candidates = vec![];
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
    let seven_days_sec = 7 * 24 * 60 * 60;
    let threshold_300mb = 300 * 1024 * 1024;

    // 1. .gemini Cleanup
    let gemini_path = "/home/a2/.gemini/antigravity/brain";
    for entry in WalkDir::new(gemini_path).max_depth(1).into_iter().filter_map(|e| e.ok()) {
        if let Ok(metadata) = entry.metadata() {
            if metadata.is_file() || metadata.is_dir() {
                let size = if metadata.is_file() { 
                    metadata.len() 
                } else { 
                    calculate_dir_size(&entry.path().to_string_lossy()) 
                };
                
                if size < threshold_300mb { continue; }

                if let Ok(modified) = metadata.modified() {
                    let mod_sec = modified.duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
                    if now - mod_sec > seven_days_sec {
                        candidates.push(CleanupItem {
                            id: format!("gemini_{}", entry.file_name().to_string_lossy()),
                            name: entry.file_name().to_string_lossy().into(),
                            path: entry.path().to_string_lossy().into(),
                            size,
                            category: "gemini".into(),
                        });
                    }
                }
            }
        }
    }

    // 2. System (Pacman) Cache check
    let pacman_cache = "/var/cache/pacman/pkg/";
    if Path::new(pacman_cache).exists() {
        let size = calculate_dir_size(pacman_cache);
        if size >= threshold_300mb { 
            candidates.push(CleanupItem {
                id: "system_pacman".into(),
                name: "Pacman Package Cache".into(),
                path: pacman_cache.into(),
                size,
                category: "system".into(),
            });
        }
    }

    // 3. System Journal check
    let journal_path = "/var/log/journal/";
    if Path::new(journal_path).exists() {
        let size = calculate_dir_size(journal_path);
        if size >= threshold_300mb {
            candidates.push(CleanupItem {
                id: "system_journal".into(),
                name: "Systemd Journal Logs".into(),
                path: journal_path.into(),
                size,
                category: "system".into(),
            });
        }
    }

    candidates
}

pub fn perform_cleanup(path: &str, category: &str) -> std::io::Result<()> {
    if category == "gemini" {
        delete_path(path)
    } else if category == "system" {
        if path == "/var/cache/pacman/pkg/" {
            clean_pacman_cache()
        } else if path == "/var/log/journal/" {
            vacuum_system_journal()
        } else {
            Err(std::io::Error::new(std::io::ErrorKind::PermissionDenied, "Unsafe system path"))
        }
    } else {
        Err(std::io::Error::new(std::io::ErrorKind::InvalidInput, "Unknown category"))
    }
}
