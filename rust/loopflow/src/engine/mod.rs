pub mod agent;
pub mod builtins;
pub mod clipboard;
pub mod command;
pub mod config;
pub(crate) mod context_budget;
pub mod error;
pub mod event;
pub mod exec;
pub mod execution;
pub mod flow;
pub mod flow_graph;
pub mod flow_output;
pub mod git;
pub mod identity;
pub mod naming;
pub mod platform;
pub(crate) mod process;
pub mod prompt;
pub mod skills;
pub mod stream;
pub mod structured_reply;
pub mod target;
pub mod transitions;
pub mod wave_home;
pub mod worktree;
pub mod worktrees;

pub use crate::repo::find_repo_root;
pub use agent::{
    build_agent_command, build_claude_command, build_codex_command, build_model_command,
    build_opencode_command, check_cli_available, codex_permission_args, exec_agent,
    missing_agent_message, workspace_add_dirs, AgentCapabilities, AgentCapture, AgentConfig,
    AgentExecResult, AgentExecutionBoundary, AgentFailure, AgentWriteScope, ClaudeArgs,
    DefaultRunner, ProcessConfig, Runner,
};
pub use command::{run_command, CommandError};
pub use config::{
    default_agent, load_config, load_config_or_default, parse_agent, Config, ExecTarget,
    SessionConfig,
};
pub use error::{CoreError, GitError, LoadError, StoreError};
pub use exec::{prepare_exec_prompt, ContextSourceOverrides, ExecPromptInput, PreparedExecPrompt};
pub use execution::{
    current_skill, ExecutionContext, ExecutionCursor, FlowEngine, FlowOutcome, NestedCursor,
    SkillExecutor, SkillOutcome, StepProgress,
};
pub use flow::{
    available_flow_names, compile_flow, find_skill_source_path, human_occurrence_ids, load_flow,
    load_goal, load_skill, render_goal, Command, ConcreteCommand, ConcretePath, ConcreteSkill,
    ConcreteStep, ConcreteXor, Flow, Goal, GoalRenderContext, Skill, Step, XorDef, XorPath,
};
pub use prompt::{
    count_tokens, drop_native_instruction_docs, format_claude_system_prompt,
    format_claude_task_prompt, format_context_prompt, format_prompt, format_task_prompt,
    gather_context, gather_documents, write_prompt_log, DiffTier, Document, DocumentSource,
    GatherContextOpts, GatherSpec, PromptComponents, PromptFormatMode, Surface,
};
pub use skills::{sync_skills, SkillSyncOptions, SkillSyncReport};
pub use stream::{
    format_event, render_event, ParseResult, ResultSubtype, StreamEvent, StreamFormat, StreamParser,
};
pub use structured_reply::{
    render_structured_reply_guidance, structured_replies_for_context, ClientContext,
    StructuredReply,
};

pub mod invocation;
