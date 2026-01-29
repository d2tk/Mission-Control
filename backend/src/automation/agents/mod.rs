pub mod protocol;
pub mod chatgpt;
pub mod claude;
pub mod ollama;

pub use protocol::{AgentContext, AgentResult};
pub use chatgpt::execute_chatgpt_task;
pub use claude::execute_claude_task;
pub use ollama::execute_ollama_task;
