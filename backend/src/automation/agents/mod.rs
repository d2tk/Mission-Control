pub mod protocol;
pub mod chatgpt;
pub mod claude;

pub use protocol::{AgentContext, AgentResult};
pub use chatgpt::execute_chatgpt_task;
pub use claude::execute_claude_task;
