use tokio::sync::{mpsc, oneshot};
use chromiumoxide::Page;
use crate::automation::core::BrowserCommand;
use crate::automation::state::StateClient;
use reqwest::Client;
use serde_json::json;
use std::collections::HashMap;

pub struct AgentContext {
    pub browser_tx: mpsc::Sender<BrowserCommand>,
    pub state: StateClient,
    pub name: String,
    pub http_client: Client,
    pub api_base: String,
}

pub type AgentResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

impl AgentContext {
    pub async fn update_status(&self, status: &str, task: &str) -> AgentResult<()> {
        if status == "idle" {
            self.state.set_busy(&self.name, false).await;
        } else {
            self.state.set_busy(&self.name, true).await;
        }

        let mut status_map = HashMap::new();
        status_map.insert("status", status.to_string());
        status_map.insert("current_task", if task.is_empty() { "None".to_string() } else { task.to_string() });

        let mut agent_update = HashMap::new();
        agent_update.insert(self.name.clone(), status_map);

        let _ = self.http_client.post(format!("{}/state", self.api_base))
            .json(&json!({ "agents": agent_update }))
            .send()
            .await;
        Ok(())
    }

    pub async fn post_message(&self, text: &str) -> AgentResult<()> {
        let _ = self.http_client.post(format!("{}/messages", self.api_base))
            .json(&json!({
                "sender": self.name,
                "message": text
            }))
            .send()
            .await;
        Ok(())
    }
}

pub async fn get_page(browser_tx: &mpsc::Sender<BrowserCommand>, url: &str) -> AgentResult<Page> {
    let (tx, rx) = oneshot::channel();
    browser_tx.send(BrowserCommand::GetPage { 
        url: url.to_string(), 
        persistent: true, 
        reply: tx 
    }).await?;
    
    rx.await.map_err(|_| "Browser actor dropped reply")?
        .map_err(|e| e.into())
}
