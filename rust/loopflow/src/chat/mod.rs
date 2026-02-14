pub mod completion;
pub mod contract;

pub use completion::{final_message_count, validate_turn_completion, CompletionError};
pub use contract::{
    parse_send_message_args, AgentEvent, ChatTurnRequest, ChatTurnResult, ContextSnapshot,
    MemoryEditLog, SendMessageArgs, ToolCallLog, UserMessagePhase, WorkspaceSnapshot,
};

#[cfg(test)]
mod contract_test;
