use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};
use serde::{Deserialize, Serialize};
use walkdir::WalkDir;
use chrono::Local;


const WORKSPACE: &str = "/home/a2/Desktop/gem";
const STATE_FILE: &str = "/home/a2/Desktop/gem/opb/backend/.sentry_state.json";
const IGNORE_PATTERNS: &[&str] = &[".git", "__pycache__", "node_modules", ".venv", "browser_data"];

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct FileMetadata {
    pub size: u64,
    pub mtime: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SentryState {
    pub timestamp: String,
    pub files: HashMap<String, FileMetadata>,
}

pub struct SentryAudit {
    workspace: PathBuf,
    state_file: PathBuf,
    current_snapshot: HashMap<String, FileMetadata>,
    previous_snapshot: HashMap<String, FileMetadata>,
}

impl SentryAudit {
    pub fn new() -> Self {
        Self {
            workspace: PathBuf::from(WORKSPACE),
            state_file: PathBuf::from(STATE_FILE),
            current_snapshot: HashMap::new(),
            previous_snapshot: HashMap::new(),
        }
    }

    pub fn load_previous_state(&mut self) {
        if let Ok(content) = fs::read_to_string(&self.state_file) {
            if let Ok(state) = serde_json::from_str::<SentryState>(&content) {
                self.previous_snapshot = state.files;
            }
        }
    }

    pub fn save_current_state(&self) {
        let state = SentryState {
            timestamp: Local::now().to_rfc3339(),
            files: self.current_snapshot.clone(),
        };
        if let Ok(json) = serde_json::to_string_pretty(&state) {
            let _ = fs::write(&self.state_file, json);
        }
    }

    pub fn scan_filesystem(&mut self) {
        let walker = WalkDir::new(&self.workspace).into_iter();
        for entry in walker.filter_entry(|e| {
            !IGNORE_PATTERNS.iter().any(|p| e.file_name().to_string_lossy() == *p)
        }) {
            if let Ok(entry) = entry {
                if entry.file_type().is_file() {
                    if let Ok(metadata) = entry.metadata() {
                        let rel_path = entry.path().strip_prefix(&self.workspace)
                            .unwrap_or(entry.path())
                            .to_string_lossy().to_string();
                        
                        let mtime = metadata.modified().unwrap_or(SystemTime::now())
                            .duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();

                        self.current_snapshot.insert(rel_path, FileMetadata {
                            size: metadata.len(),
                            mtime,
                        });
                    }
                }
            }
        }
    }

    pub fn analyze_changes(&self) -> serde_json::Value {
        let current_keys: HashSet<_> = self.current_snapshot.keys().collect();
        let previous_keys: HashSet<_> = self.previous_snapshot.keys().collect();

        let created: Vec<_> = current_keys.difference(&previous_keys).map(|k| k.to_string()).collect();
        let deleted: Vec<_> = previous_keys.difference(&current_keys).map(|k| k.to_string()).collect();
        let mut modified = Vec::new();

        for key in current_keys.intersection(&previous_keys) {
            if self.current_snapshot.get(*key) != self.previous_snapshot.get(*key) {
                modified.push(key.to_string());
            }
        }

        let mut total_delta_bytes: i64 = 0;
        for path in &created {
            total_delta_bytes += self.current_snapshot[path].size as i64;
        }
        for path in &deleted {
            total_delta_bytes -= self.previous_snapshot[path].size as i64;
        }

        serde_json::json!({
            "created": created,
            "modified": modified,
            "deleted": deleted,
            "total_delta_mb": total_delta_bytes as f64 / (1024.0 * 1024.0),
            "churn": created.len() + modified.len() + deleted.len()
        })
    }

    pub fn get_git_status(&self) -> serde_json::Value {
        if !self.workspace.join(".git").exists() {
            return serde_json::json!({"available": false});
        }

        let branch = Command::new("git").arg("branch").arg("--show-current").current_dir(&self.workspace).output().ok()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string()).unwrap_or_default();

        let status_out = Command::new("git").arg("status").arg("--porcelain=v1").current_dir(&self.workspace).output().ok()
            .map(|o| String::from_utf8_lossy(&o.stdout).to_string()).unwrap_or_default();

        let lines: Vec<&str> = status_out.lines().collect();
        let modified = lines.iter().filter(|l| l.starts_with(" M")).count();
        let staged = lines.iter().filter(|l| l.starts_with("M ")).count();
        let untracked = lines.iter().filter(|l| l.starts_with("??")).count();

        let last_commit = Command::new("git").arg("log").arg("-1").arg("--format=%h|%cr").current_dir(&self.workspace).output().ok()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string()).unwrap_or_default();
        let commit_parts: Vec<&str> = last_commit.split('|').collect();

        serde_json::json!({
            "available": true,
            "branch": branch,
            "clean": lines.is_empty(),
            "modified": modified,
            "staged": staged,
            "untracked": untracked,
            "last_commit": commit_parts.get(0).unwrap_or(&"N/A"),
            "commit_age": commit_parts.get(1).unwrap_or(&"N/A")
        })
    }

    pub fn get_disk_usage(&self) -> serde_json::Value {
        use sysinfo::Disks;
        let disks = Disks::new_with_refreshed_list();
        
        let mut total_b: u64 = 1;
        let mut avail_b: u64 = 0;
        
        for disk in &disks {
            total_b = disk.total_space();
            avail_b = disk.available_space();
            break; // Use the first disk as in Python shutil.disk_usage
        }
        let used_b = total_b.saturating_sub(avail_b);

        let workspace_bytes = Command::new("du").arg("-sb").arg(&self.workspace).output().ok()
            .and_then(|o| String::from_utf8_lossy(&o.stdout).split_whitespace().next().map(|s| s.parse::<u64>().unwrap_or(0)))
            .unwrap_or(0);

        serde_json::json!({
            "total_gb": total_b / (1024 * 1024 * 1024),
            "used_gb": used_b / (1024 * 1024 * 1024),
            "free_gb": avail_b / (1024 * 1024 * 1024),
            "usage_percent": (used_b as f64 / total_b as f64) * 100.0,
            "workspace_gb": workspace_bytes as f64 / (1024.0 * 1024.0 * 1024.0)
        })
    }

    pub fn generate_report(&self, changes: &serde_json::Value, git: &serde_json::Value, disk: &serde_json::Value) -> String {
        let mut lines = Vec::new();
        lines.push("=== PROJECT SENTRY DAILY REPORT ===".to_string());
        lines.push(format!("Date: {}", Local::now().format("%Y-%m-%d %H:%M:%S")));
        lines.push(format!("Workspace: {}", self.workspace.display()));
        lines.push("".to_string());

        lines.push("[SUMMARY]".to_string());
        let churn = changes["churn"].as_u64().unwrap_or(0);
        let churn_level = if churn > 20 { "HIGH" } else if churn > 5 { "MEDIUM" } else { "LOW" };
        lines.push(format!("File changes: +{} ~{} -{}   (Churn: {})", 
            changes["created"].as_array().map(|a| a.len()).unwrap_or(0),
            changes["modified"].as_array().map(|a| a.len()).unwrap_or(0),
            changes["deleted"].as_array().map(|a| a.len()).unwrap_or(0),
            churn_level));

        if git["available"].as_bool().unwrap_or(false) {
            let status = if git["clean"].as_bool().unwrap_or(false) { "CLEAN" } else { "DIRTY" };
            lines.push(format!("Git status: {} (branch: {})", status, git["branch"].as_str().unwrap_or("")));
        }

        let usage_percent = disk["usage_percent"].as_f64().unwrap_or(0.0);
        let disk_status = if usage_percent > 80.0 { "WARNING" } else { "OK" };
        lines.push(format!("Disk usage: {:.0}% ({})", usage_percent, disk_status));
        lines.append(&mut vec!["".to_string(), "[FILESYSTEM]".to_string()]);
        
        lines.push(format!("Created: {}", changes["created"].as_array().map(|a| a.len()).unwrap_or(0)));
        lines.push(format!("Modified: {}", changes["modified"].as_array().map(|a| a.len()).unwrap_or(0)));
        lines.push(format!("Deleted: {}", changes["deleted"].as_array().map(|a| a.len()).unwrap_or(0)));
        lines.push(format!("Size delta: {:+.1} MB", changes["total_delta_mb"].as_f64().unwrap_or(0.0)));

        if let Some(created) = changes["created"].as_array() {
            if !created.is_empty() {
                lines.push("Top created:".to_string());
                for f in created.iter().take(3) {
                    lines.push(format!("  - {}", f.as_str().unwrap_or("")));
                }
            }
        }
        lines.push("".to_string());

        if git["available"].as_bool().unwrap_or(false) {
            lines.push("[GIT]".to_string());
            lines.push(format!("Branch: {}", git["branch"].as_str().unwrap_or("")));
            lines.push(format!("Modified: {}", git["modified"]));
            lines.push(format!("Staged: {}", git["staged"]));
            lines.push(format!("Untracked: {}", git["untracked"]));
            lines.push(format!("Last commit: {} ({})", git["commit_age"].as_str().unwrap_or(""), git["last_commit"].as_str().unwrap_or("")));
            lines.push("".to_string());
        }

        lines.push("[RESOURCES]".to_string());
        lines.push(format!("Disk: {}G total / {}G free ({:.0}%)", 
            disk["total_gb"], disk["free_gb"], usage_percent));
        lines.push(format!("Workspace size: {:.1}G", disk["workspace_gb"].as_f64().unwrap_or(0.0)));
        lines.push("".to_string());

        let mut alerts = Vec::new();
        if git["available"].as_bool().unwrap_or(false) && !git["clean"].as_bool().unwrap_or(false) {
            alerts.push("Uncommitted changes present");
        }
        if usage_percent > 80.0 {
            alerts.push("Disk usage above 80%");
        }
        if churn > 50 {
            alerts.push("High file churn detected");
        }

        if !alerts.is_empty() {
            lines.push("[ALERTS]".to_string());
            for alert in alerts {
                lines.push(format!("- {}", alert));
            }
            lines.push("".to_string());
        }

        let mut actions = Vec::new();
        if git["available"].as_bool().unwrap_or(false) && !git["clean"].as_bool().unwrap_or(false) {
            actions.push("Commit or stash working tree");
        }
        if usage_percent > 80.0 {
            actions.push("Review disk usage and clean up old files");
        }

        if !actions.is_empty() {
            lines.push("[RECOMMENDED ACTIONS]".to_string());
            for action in actions {
                lines.push(format!("- {}", action));
            }
            lines.push("".to_string());
        }

        lines.push("Roger. Over.".to_string());
        lines.join("\n")
    }

    pub fn run(&mut self) -> String {
        self.load_previous_state();
        self.scan_filesystem();
        let changes = self.analyze_changes();
        let git = self.get_git_status();
        let disk = self.get_disk_usage();
        let report = self.generate_report(&changes, &git, &disk);
        self.save_current_state();
        report
    }
}
