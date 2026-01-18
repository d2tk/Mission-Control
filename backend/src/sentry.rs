use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};
use serde::{Deserialize, Serialize};
use walkdir::WalkDir;
use chrono::Local;
use std::hash::{Hash, Hasher};
use std::collections::hash_map::DefaultHasher;

const WORKSPACE: &str = "/home/a2/Desktop/gem";
const STATE_FILE: &str = "/home/a2/Desktop/gem/opb/backend/.sentry_state.json";
const IGNORE_PATTERNS: &[&str] = &[".git", "__pycache__", "node_modules", ".venv", "browser_data", "target"];

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct FileMetadata {
    pub size: u64,
    pub mtime: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SentryState {
    pub timestamp: String,
    pub files: HashMap<u64, FileMetadata>, // Hashed path keys for memory efficiency
}

pub struct AuditFinding {
    pub title: String,
    pub message: String,
    pub severity: String, // "OK", "info", "warning", "critical"
}

pub struct AuditRecommendation {
    pub action: String,
}

pub trait AuditRule {
    fn run(&self, ctx: &SentryAudit) -> (Vec<AuditFinding>, Vec<AuditRecommendation>);
}

pub struct SentryAudit {
    pub workspace: PathBuf,
    pub state_file: PathBuf,
    pub current_snapshot: HashMap<u64, FileMetadata>,
    pub previous_snapshot: HashMap<u64, FileMetadata>,
    pub path_map: HashMap<u64, String>, // Temporary path map for reporting
}

impl SentryAudit {
    pub fn new() -> Self {
        Self {
            workspace: PathBuf::from(WORKSPACE),
            state_file: PathBuf::from(STATE_FILE),
            current_snapshot: HashMap::new(),
            previous_snapshot: HashMap::new(),
            path_map: HashMap::new(),
        }
    }

    fn hash_path(&self, path: &str) -> u64 {
        let mut s = DefaultHasher::new();
        path.hash(&mut s);
        s.finish()
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

                        let h = self.hash_path(&rel_path);
                        self.current_snapshot.insert(h, FileMetadata {
                            size: metadata.len(),
                            mtime,
                        });
                        self.path_map.insert(h, rel_path);
                    }
                }
            }
        }
    }



    pub fn run(&mut self, full_scan: bool) -> String {
        self.load_previous_state();
        self.scan_filesystem();

        let mut rules: Vec<Box<dyn AuditRule>> = vec![
            Box::new(MemoryRule {}),
            Box::new(GitRule {}),
            Box::new(ChurnRule {}),
        ];

        if full_scan {
            rules.push(Box::new(DiskRule {}));
        }

        let mut all_findings = vec![];
        let mut all_recommendations = vec![];

        for rule in rules {
            let (f, r) = rule.run(self);
            all_findings.extend(f);
            all_recommendations.extend(r);
        }

        let report = self.format_report(&all_findings, &all_recommendations);
        self.save_current_state();
        report
    }

    fn format_report(&self, findings: &[AuditFinding], recommendations: &[AuditRecommendation]) -> String {
        let mut lines = vec![];
        lines.push("=== PROJECT SENTRY AUDIT REPORT ===".to_string());
        lines.push(format!("Date: {}", Local::now().format("%Y-%m-%d %H:%M:%S")));
        lines.push(format!("Workspace: {}", self.workspace.display()));
        lines.push("".to_string());

        lines.push("[FINDINGS]".to_string());
        if findings.is_empty() {
            lines.push("- No significant findings.".to_string());
        } else {
            for f in findings {
                let prefix = match f.severity.as_str() {
                    "critical" => "🚨 [CRITICAL]",
                    "warning" => "⚠️ [WARNING]",
                    "info" => "ℹ️ [INFO]",
                    _ => "✅ [OK]",
                };
                lines.push(format!("{} {}: {}", prefix, f.title, f.message));
            }
        }
        lines.push("".to_string());

        if !recommendations.is_empty() {
            lines.push("[RECOMMENDED ACTIONS]".to_string());
            for r in recommendations {
                lines.push(format!("- {}", r.action));
            }
            lines.push("".to_string());
        }

        lines.push("Roger. Over.".to_string());
        lines.join("\n")
    }
}

// --- Specific Rules ---

struct MemoryRule;
impl AuditRule for MemoryRule {
    fn run(&self, _ctx: &SentryAudit) -> (Vec<AuditFinding>, Vec<AuditRecommendation>) {
        use sysinfo::System;
        let mut sys = System::new_all();
        sys.refresh_memory();

        let total = sys.total_memory();
        let used = sys.used_memory();
        let pct = (used as f64 / total as f64) * 100.0;
        
        let total_gb = total as f64 / (1024.0 * 1024.0 * 1024.0);
        let used_gb = used as f64 / (1024.0 * 1024.0 * 1024.0);

        let mut findings = vec![];
        let mut recs = vec![];

        let severity = if pct > 80.0 { "critical" } 
                      else if pct > 60.0 { "warning" } 
                      else { "OK" };

        findings.push(AuditFinding {
            title: "Memory Usage".into(),
            message: format!("{:.1}% used ({:.1}GB/{:.1}GB)", pct, used_gb, total_gb),
            severity: severity.into(),
        });

        if pct > 60.0 {
            recs.push(AuditRecommendation { action: "Check for memory leaks or close unused applications.".into() });
        }

        (findings, recs)
    }
}

struct DiskRule;
impl AuditRule for DiskRule {
    fn run(&self, ctx: &SentryAudit) -> (Vec<AuditFinding>, Vec<AuditRecommendation>) {
        let mut findings = vec![];
        let mut recs = vec![];

        // Reuse central storage logic logic
        if let Some(stats) = crate::storage::get_disk_usage(ctx.workspace.to_str().unwrap_or("/")) {
             let severity = if stats.usage_pct > 90.0 { "critical" } 
                           else if stats.usage_pct > 85.0 { "warning" } 
                           else { "OK" };
            
            findings.push(AuditFinding {
                title: "Disk Usage".into(),
                message: format!("{:.1}% used ({:.1}GB/{:.1}GB)", stats.usage_pct, stats.used as f64 / 1e9, stats.total as f64 / 1e9),
                severity: severity.into(),
            });

            if stats.usage_pct > 85.0 {
                recs.push(AuditRecommendation { action: "Run disk cleanup to free up space.".into() });
            }
        } else {
             findings.push(AuditFinding {
                title: "Disk Detection".into(),
                message: "Could not identify primary workspace partition.".into(),
                severity: "warning".into(),
            });
        }

        (findings, recs)
    }
}

struct GitRule;
impl AuditRule for GitRule {
    fn run(&self, ctx: &SentryAudit) -> (Vec<AuditFinding>, Vec<AuditRecommendation>) {
        let mut findings = vec![];
        let mut recs = vec![];

        if !ctx.workspace.join(".git").exists() {
            return (findings, recs);
        }

        let status_out = Command::new("git").arg("status").arg("--porcelain").current_dir(&ctx.workspace).output().ok()
            .map(|o| String::from_utf8_lossy(&o.stdout).to_string()).unwrap_or_default();

        if !status_out.is_empty() {
            let lines: Vec<&str> = status_out.lines().collect();
            findings.push(AuditFinding {
                title: "Git Working Tree".into(),
                message: format!("{} uncommitted changes detected.", lines.len()),
                severity: "warning".into(),
            });
            recs.push(AuditRecommendation { action: "Commit or stash your current changes.".into() });
        } else {
            findings.push(AuditFinding {
                title: "Git Status".into(),
                message: "Working tree is clean.".into(),
                severity: "OK".into(),
            });
        }

        (findings, recs)
    }
}

struct ChurnRule;
impl AuditRule for ChurnRule {
    fn run(&self, ctx: &SentryAudit) -> (Vec<AuditFinding>, Vec<AuditRecommendation>) {
        let mut findings = vec![];
        
        let current_keys: HashSet<_> = ctx.current_snapshot.keys().collect();
        let previous_keys: HashSet<_> = ctx.previous_snapshot.keys().collect();

        let created = current_keys.difference(&previous_keys).count();
        let deleted = previous_keys.difference(&current_keys).count();
        let mut modified = 0;

        for key in current_keys.intersection(&previous_keys) {
            if ctx.current_snapshot.get(*key) != ctx.previous_snapshot.get(*key) {
                modified += 1;
            }
        }

        let churn = created + deleted + modified;
        if churn > 0 {
            let severity = if churn > 50 { "warning" } else { "info" };
            findings.push(AuditFinding {
                title: "File Churn".into(),
                message: format!("{} changes since last audit (+{} ~{} -{}).", churn, created, modified, deleted),
                severity: severity.into(),
            });
        }

        (findings, vec![])
    }
}

