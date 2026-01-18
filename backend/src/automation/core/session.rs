use std::collections::HashMap;
use tokio::sync::mpsc;
use crate::automation::core::BrowserCommand;
use chromiumoxide::BrowserConfig;
use crate::automation::core::browser::BrowserActor;

pub struct SessionEntry {
    pub tx: mpsc::Sender<BrowserCommand>,
    pub task_count: usize,
    pub created_at: std::time::Instant,
}

pub struct SessionPool {
    sessions: HashMap<String, SessionEntry>,
}

impl SessionPool {
    pub fn new() -> Self {
        Self {
            sessions: HashMap::new(),
        }
    }

    pub async fn get_or_create(&mut self, name: &str, config_fn: impl Fn() -> BrowserConfig) -> Result<mpsc::Sender<BrowserCommand>, String> {
        if let Some(entry) = self.sessions.get(name) {
            // Check limits (e.g., 50 tasks or 1 hour)
            let age = entry.created_at.elapsed();
            if entry.task_count < 50 && age < std::time::Duration::from_secs(3600) {
                return Ok(entry.tx.clone());
            }
            
            // Limit reached, remove and gracefully close the old actor
            println!("🔄 Hard Refresh: Session limit reached for {}. Restarting...", name);
            let _ = entry.tx.send(BrowserCommand::Close).await;
            self.sessions.remove(name);
        }

        // Launch new actor
        let (tx, rx) = mpsc::channel(32);
        let config = config_fn();
        let actor = BrowserActor::new(config, rx).await?;
        
        tokio::spawn(actor.run());

        let entry = SessionEntry {
            tx: tx.clone(),
            task_count: 0,
            created_at: std::time::Instant::now(),
        };
        self.sessions.insert(name.to_string(), entry);
        
        Ok(tx)
    }

    pub fn increment_task(&mut self, name: &str) {
        if let Some(entry) = self.sessions.get_mut(name) {
            entry.task_count += 1;
        }
    }

    pub async fn purge(&mut self, name: &str) {
        if let Some(entry) = self.sessions.remove(name) {
            let _ = entry.tx.send(BrowserCommand::Close).await;
        }
    }
}
