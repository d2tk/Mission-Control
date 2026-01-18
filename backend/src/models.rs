use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Message {
    pub id: Option<usize>,
    pub sender: String,
    pub message: String,
    pub timestamp: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AgentState {
    pub status: String,
    pub current_task: Option<String>,
    pub last_task: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MissionState {
    pub mission_id: String,
    pub status: String,
    pub agents: HashMap<String, AgentState>,
    pub current_task: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SystemStatus {
    pub name: String,
    pub status: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Activity {
    pub time: String,
    pub agent: String,
    pub action: String,
    #[serde(rename = "type")]
    pub activity_type: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Project {
    pub name: String,
    pub description: String,
    pub status: String,
    pub tags: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DocInfo {
    pub path: String,
    pub added: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DiskStats {
    pub total: u64,
    pub used: u64,
    pub free: u64,
    pub usage_pct: f64,
    pub workspace_size: u64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CleanupItem {
    pub id: String,
    pub name: String,
    pub path: String,
    pub size: u64,
    pub category: String, // "system" | "gemini"
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct DashboardData {
    pub global_status: String,
    pub systems: Vec<SystemStatus>,
    pub activities: Vec<Activity>,
    pub projects: Vec<Project>,
    pub metrics: Vec<serde_json::Value>,
    #[serde(default)]
    pub docs: Vec<DocInfo>,
    #[serde(default)]
    pub agents: HashMap<String, AgentState>,
    pub all_systems_go: bool,
    pub disk: Option<DiskStats>,
}
