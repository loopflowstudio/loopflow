pub mod agent;
pub mod builtins;
pub mod clipboard;
pub mod command;
pub mod config;
pub mod error;
pub mod event;
pub mod execution;
pub mod fast_path;
pub mod flow;
pub mod git;
pub mod identity;
pub mod launch;
pub mod naming;
pub mod platform;
pub(crate) mod process;
pub mod prompt;
pub mod repo;
pub mod skills;
pub mod stream;
pub mod structured_reply;
pub mod wave_config;
pub mod wave_context;
pub mod worktree;
pub mod worktrees;

pub use agent::{
    build_agent_command, build_claude_command, build_codex_command, build_gemini_command,
    build_model_command, build_opencode_command, check_cli_available, codex_permission_args,
    launch_agent, workspace_add_dirs, AgentCapabilities, AgentConfig, ClaudeArgs, DefaultRunner,
    LaunchResult, ProcessConfig, Runner,
};
pub use command::{run_command, CommandError};
pub use config::{
    load_config, load_config_or_default, parse_agent, Config, LaunchTarget, SessionConfig,
};
pub use error::{CoreError, GitError, LoadError, StoreError};
pub use execution::{
    advance_cursor_after_wait, current_flow_parents, current_skill, xor_verdict_path,
    ExecutionContext, ExecutionCursor, ExecutionSkill, FlowEngine, FlowOutcome, FlowProgress,
    NestedCursor, SkillExecutor, SkillOutcome, TEMP_XOR_ROUTE_STEP_NAME,
};
pub use flow::{
    available_flow_names, expand_flow, load_direction, load_flow, load_goal, load_skill,
    next_action, render_goal, ConcreteOp, ConcreteSkill, ConcreteStep, ConcreteXor, Direction,
    Flow, FlowAction, Goal, GoalRenderContext, Op, Skill, Step, XorDef, XorPath,
};
pub use launch::{
    prepare_launch_prompt, ContextSourceOverrides, LaunchPromptInput, PreparedLaunchPrompt,
};
pub use prompt::{
    count_tokens, drop_native_instruction_docs, format_claude_system_prompt,
    format_claude_task_prompt, format_context_prompt, format_prompt, format_task_prompt,
    gather_context, gather_documents, write_prompt_log, DiffTier, Document, DocumentSource,
    GatherContextOpts, GatherSpec, GatheredContext, PromptComponents, PromptFormatMode,
    RenderedPrompt, Surface,
};
pub use repo::find_repo_root;
pub use skills::{sync_skills, SkillSyncOptions, SkillSyncReport};
pub use stream::{
    format_event, render_event, ParseResult, ResultSubtype, StreamEvent, StreamFormat, StreamParser,
};
pub use structured_reply::{
    render_structured_reply_guidance, structured_replies_for_context, ClientContext,
    StructuredReply,
};
