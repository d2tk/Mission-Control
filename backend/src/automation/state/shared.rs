use std::collections::HashSet;
use tokio::sync::{mpsc, oneshot};
use std::fs;

const PROCESSED_FILE: &str = "processed_ids.txt";

#[derive(Debug)]
pub enum StateCommand {
    CheckProcessed { id: usize, reply: oneshot::Sender<bool> },
    SaveProcessed { id: usize },
    IsBusy { agent: String, reply: oneshot::Sender<bool> },
    SetBusy { agent: String, busy: bool },
    GetLastMessageId { reply: oneshot::Sender<usize> },
    SetLastMessageId { id: usize },
}

pub struct StateActor {
    processed_ids: HashSet<usize>,
    busy_agents: HashSet<String>,
    last_message_id: usize,
    rx: mpsc::Receiver<StateCommand>,
}

impl StateActor {
    pub fn new(rx: mpsc::Receiver<StateCommand>) -> Self {
        let mut processed_ids = HashSet::new();
        if let Ok(content) = fs::read_to_string(PROCESSED_FILE) {
            processed_ids = content.lines()
                .filter_map(|l| l.parse::<usize>().ok())
                .collect();
        }

        Self {
            processed_ids,
            busy_agents: HashSet::new(),
            last_message_id: 0,
            rx,
        }
    }

    pub async fn run(mut self) {
        while let Some(cmd) = self.rx.recv().await {
            match cmd {
                StateCommand::CheckProcessed { id, reply } => {
                    let _ = reply.send(self.processed_ids.contains(&id));
                }
                StateCommand::SaveProcessed { id } => {
                    self.processed_ids.insert(id);
                    let content: String = self.processed_ids.iter()
                        .map(|id| id.to_string())
                        .collect::<Vec<_>>()
                        .join("\n");
                    let _ = fs::write(PROCESSED_FILE, content);
                }
                StateCommand::IsBusy { agent, reply } => {
                    let _ = reply.send(self.busy_agents.contains(&agent));
                }
                StateCommand::SetBusy { agent, busy } => {
                    if busy {
                        self.busy_agents.insert(agent);
                    } else {
                        self.busy_agents.remove(&agent);
                    }
                }
                StateCommand::GetLastMessageId { reply } => {
                    let _ = reply.send(self.last_message_id);
                }
                StateCommand::SetLastMessageId { id } => {
                    if id > self.last_message_id {
                        self.last_message_id = id;
                    }
                }
            }
        }
    }
}

#[derive(Clone)]
pub struct StateClient {
    tx: mpsc::Sender<StateCommand>,
}

impl StateClient {
    pub fn new(tx: mpsc::Sender<StateCommand>) -> Self {
        Self { tx }
    }

    pub async fn check_processed(&self, id: usize) -> bool {
        let (tx, rx) = oneshot::channel();
        let _ = self.tx.send(StateCommand::CheckProcessed { id, reply: tx }).await;
        rx.await.unwrap_or(false)
    }

    pub async fn save_processed(&self, id: usize) {
        let _ = self.tx.send(StateCommand::SaveProcessed { id }).await;
    }

    pub async fn is_busy(&self, agent: &str) -> bool {
        let (tx, rx) = oneshot::channel();
        let _ = self.tx.send(StateCommand::IsBusy { agent: agent.to_string(), reply: tx }).await;
        rx.await.unwrap_or(false)
    }

    pub async fn set_busy(&self, agent: &str, busy: bool) {
        let _ = self.tx.send(StateCommand::SetBusy { agent: agent.to_string(), busy }).await;
    }

    pub async fn get_last_message_id(&self) -> usize {
        let (tx, rx) = oneshot::channel();
        let _ = self.tx.send(StateCommand::GetLastMessageId { reply: tx }).await;
        rx.await.unwrap_or(0)
    }

    pub async fn set_last_message_id(&self, id: usize) {
        let _ = self.tx.send(StateCommand::SetLastMessageId { id }).await;
    }
}
