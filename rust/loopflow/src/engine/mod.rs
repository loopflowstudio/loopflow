pub mod agent;
pub mod builtins;
pub mod clipboard;
pub mod command;
pub mod config;
pub mod context_budget;
pub mod definition_name;
pub mod error;
pub mod execution;
pub mod flow;
pub mod flow_graph;
pub mod flow_instructions;
pub mod flow_output;
#[cfg(target_os = "macos")]
pub(crate) mod fs_events;
pub mod git;
pub mod identity;
pub mod machine_route;
pub mod naming;
pub mod planning_exchange;
pub mod planning_git;
pub mod platform;
pub(crate) mod process;
pub mod process_prompt;
pub mod prompt;
pub mod skill_catalog;
pub mod skill_invocation;
pub mod skills;
pub mod stream;
pub mod structured_reply;
pub mod target;
pub(crate) mod terminal_title;
pub mod transitions;
pub mod workflow;
pub mod worktree;
pub mod worktrees;

pub use crate::repo::find_repo_root;
pub use agent::{
    build_agent_command, build_claude_command, build_codex_command, build_model_command,
    build_opencode_command, check_cli_available, codex_permission_args, missing_agent_message,
    run_agent, workspace_add_dirs, AgentCapabilities, AgentCapture, AgentConfig,
    AgentExecutionBoundary, AgentFailure, AgentProcessResult, AgentWriteScope, ClaudeArgs,
    ProcessConfig,
};
pub use command::{run_command, CommandError};
pub use config::{
    default_agent, load_config, load_config_or_default, parse_agent, Config, SessionConfig,
};
pub use error::{CoreError, GitError, LoadError};
pub use execution::{
    current_skill, ExecutionContext, ExecutionCursor, FlowEngine, FlowOutcome, NestedCursor,
    SkillExecutor, SkillOutcome, StepProgress,
};
pub use flow::{
    available_flow_names, compile_flow, human_occurrence_ids, load_flow, load_skill, Command,
    ConcreteCommand, ConcretePath, ConcreteSkill, ConcreteStep, ConcreteXor, FlowDefinition, Skill,
    Step, XorDef, XorPath,
};
pub use process_prompt::{
    prepare_process_prompt, ContextSourceOverrides, PreparedProcessPrompt, ProcessPromptInput,
};
pub use prompt::{
    count_tokens, drop_duplicate_docs, format_prompt, gather_context, gather_documents,
    write_prompt_log, DiffTier, Document, DocumentSource, GatherContextOpts, GatherSpec,
    PromptComponents, Surface,
};
pub use skills::{sync_skills, SkillSyncOptions, SkillSyncReport};
pub use stream::{
    format_event, render_event, ParseResult, ResultSubtype, StreamEvent, StreamFormat, StreamParser,
};
pub use structured_reply::{
    render_structured_reply_guidance, structured_replies_for_context, ClientContext,
    StructuredReply,
};
