pub mod core;
pub mod state;
pub mod agents;

use tokio::sync::mpsc;
use crate::automation::core::session::SessionPool;
use crate::automation::state::{StateActor, StateClient};
use crate::automation::agents::{AgentContext, execute_chatgpt_task, execute_claude_task};
use crate::models::Message;
use chromiumoxide::BrowserConfig;
use std::path::PathBuf;
use chromiumoxide::browser::HeadlessMode;
use tokio::time::{sleep, Duration};
use reqwest::Client;
use serde::Serialize;
use tokio::sync::oneshot;
use crate::automation::core::BrowserCommand;

const API_BASE: &str = "http://localhost:8000/api";
const POLL_INTERVAL: u64 = 2;

#[derive(Serialize)]
struct MessageEnvelope {
    assigned_to: String,
    input: String,
}

pub struct BridgeManager {
    session_pool: SessionPool,
    state: StateClient,
    http_client: Client,
}

impl BridgeManager {
    pub async fn new() -> (Self, StateActor) {
        let (state_tx, state_rx) = mpsc::channel(100);
        let state_actor = StateActor::new(state_rx);
        let state_client = StateClient::new(state_tx);

        (Self {
            session_pool: SessionPool::new(),
            state: state_client,
            http_client: Client::new(),
        }, state_actor)
    }

    pub fn create_default_config(&self, name: &str) -> BrowserConfig {
        let data_dir = format!("./isolated_data/{}", name.to_lowercase());
        BrowserConfig::builder()
            .user_data_dir(PathBuf::from(data_dir))
            .headless_mode(HeadlessMode::False)
            // HBI Phase 8: Hyprland Compatibility - Remove fixed size for dynamic tiling
            // .window_size(1920, 1080)
            .arg("--start-maximized")
            // HBI Phase 8: Extreme Suppression & Banner Removal
            .arg("--disable-blink-features=AutomationControlled")
            .arg("--exclude-switches=enable-automation")
            .arg("--test-type")
            .arg("--no-first-run")
            .arg("--no-default-browser-check")
            .arg("--use-fake-ui-for-media-stream")
            .arg("--disable-infobars")
            .arg("--no-sandbox")
            .arg("--disable-setuid-sandbox")
            .arg("--user-agent=Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
            .build()
            .unwrap()
    }

    pub async fn run(&mut self) {
        println!("🚀 BridgeManager operational (Actor Edition).");
        self.sync_initial_state().await;

        loop {
            if let Err(e) = self.check_messages().await {
                eprintln!("Error checking messages: {}", e);
            }
            sleep(Duration::from_secs(POLL_INTERVAL)).await;
        }
    }

    async fn sync_initial_state(&self) {
        // ... omitted ...
    }

    fn translate_to_consultant_prompt(&self, text: &str) -> String {
        format!(
            "AI Consultant Service: Please analyze and provide professional advice on the following request from the Commander. Ensure high accuracy and actionable insights.\n\nCommander's Request: {}",
            text
        )
    }

    async fn verify_browser_health(&mut self, agent_name: &str) -> bool {
        let config = self.create_default_config(agent_name);
        match self.session_pool.get_or_create(agent_name, || config.clone()).await {
            Ok(tx) => {
                let (reply_tx, reply_rx) = oneshot::channel();
                if let Ok(_) = tx.send(BrowserCommand::Ping { reply: reply_tx }).await {
                    match tokio::time::timeout(Duration::from_secs(5), reply_rx).await {
                        Ok(Ok(healthy)) => healthy,
                        _ => {
                            println!("[Bridge] {} health check TIMEOUT/ERR. Purging session.", agent_name);
                            self.session_pool.purge(agent_name).await;
                            false
                        }
                    }
                } else {
                    false
                }
            }
            Err(_) => false,
        }
    }

    fn strip_routing_tags(&self, text: &str) -> String {
        let mut cleaned = text.to_string();
        for tag in &["@chatgpt", "@claude", "!gpt"] {
            let re = regex::Regex::new(&format!(r"(?i){}\s*", tag)).unwrap();
            cleaned = re.replace_all(&cleaned, "").to_string();
        }
        cleaned.trim().to_string()
    }

    async fn handle_gpt_command(&self, text: &str) -> String {
        let parts: Vec<&str> = text.split_whitespace().collect();
        if parts.len() < 2 {
            let env = MessageEnvelope {
                assigned_to: "ChatGPT".to_string(),
                input: "Usage: !gpt <filename> [optional request] OR !gpt <your question>".to_string(),
            };
            return serde_json::to_string(&env).unwrap_or_default();
        }

        let first_arg = parts[1];
        let remaining = parts[2..].join(" ");

        let is_likely_file = first_arg.contains('.');

        let input_content = if is_likely_file {
            let mut found_path = None;
            for entry in walkdir::WalkDir::new("/home/a2/Desktop/gem")
                .into_iter()
                .filter_entry(|e| {
                    let name = e.file_name().to_string_lossy();
                    name != "target" && name != ".git" && name != "node_modules"
                })
            {
                if let Ok(entry) = entry {
                    if entry.file_name().to_string_lossy() == first_arg {
                        found_path = Some(entry.path().to_path_buf());
                        break;
                    }
                }
            }

            if let Some(path) = found_path {
                match std::fs::read_to_string(&path) {
                    Ok(content) => {
                        format!(
                            "Commander requested a review for file: `{}`\n\n```rust\n{}\n```\n\nRequest: {}", 
                            first_arg, 
                            content, 
                            if remaining.is_empty() { "Comprehensive code review and optimization suggestions." } else { &remaining }
                        )
                    }
                    Err(e) => format!("Error reading {}: {}", first_arg, e),
                }
            } else {
                self.translate_to_consultant_prompt(&self.strip_routing_tags(text))
            }
        } else {
            self.translate_to_consultant_prompt(&self.strip_routing_tags(text))
        };

        let env = MessageEnvelope {
            assigned_to: "ChatGPT".to_string(),
            input: input_content,
        };
        serde_json::to_string(&env).unwrap_or_default()
    }

    async fn check_messages(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let resp = self.http_client.get(format!("{}/messages", API_BASE)).send().await?;
        let messages: Vec<Message> = resp.json().await?;

        for msg in messages {
            let msg_id = match msg.id {
                Some(id) => id,
                None => continue,
            };

            if self.state.check_processed(msg_id).await { continue; }
            self.state.set_last_message_id(msg_id).await;

            let text = msg.message.trim();
            let lower_text = text.to_lowercase();

            if (lower_text.contains("@chatgpt") || lower_text.contains("!gpt")) && msg.sender != "ChatGPT" {
                let cleaned = if lower_text.starts_with("!gpt") {
                    self.handle_gpt_command(text).await
                } else {
                    self.strip_routing_tags(text)
                };
                self.dispatch_task("ChatGPT", &cleaned).await?;
                self.state.save_processed(msg_id).await;
            } else if lower_text.contains("@claude") && msg.sender != "Claude" {
                let cleaned = self.strip_routing_tags(text);
                self.dispatch_task("Claude", &cleaned).await?;
                self.state.save_processed(msg_id).await;
            }
        }
        Ok(())
    }

    async fn dispatch_task(&mut self, agent_name: &str, prompt: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if self.state.is_busy(agent_name).await {
            return Ok(());
        }

        // Phase 7: Strict Browser Health Check before dispatch
        if !self.verify_browser_health(agent_name).await {
            println!("[Bridge] Attempting browser recovery for {}...", agent_name);
            // Recovery is handled by verify_browser_health purging the session
            // The next get_or_create will launch a fresh one.
        }

        let config = self.create_default_config(agent_name);
        let browser_tx = self.session_pool.get_or_create(agent_name, || config.clone()).await?;
        let ctx = AgentContext {
            browser_tx,
            state: self.state.clone(),
            name: agent_name.to_string(),
            http_client: self.http_client.clone(),
            api_base: API_BASE.to_string(),
        };

        let prompt_owned = prompt.to_string();
        if agent_name == "ChatGPT" {
            tokio::spawn(async move {
                let _ = execute_chatgpt_task(&ctx, &prompt_owned).await;
            });
        } else if agent_name == "Claude" {
            tokio::spawn(async move {
                let _ = execute_claude_task(&ctx, &prompt_owned).await;
            });
        }

        Ok(())
    }
}
