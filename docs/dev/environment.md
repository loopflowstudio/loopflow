# Environment

Environment configures a process; it never decides what the process is. A
variable that names a Machine or a process is checked against the store or
the filesystem before it grants anything, and a new session starts without it.

```bash
LF_HOME="$(mktemp -d)" target/debug/lf wave list --json   # the only Machine selector
env | grep -E '^(LF|LOOPFLOW)_'                            # what this process inherited
tmux show-environment -g | grep -E '^(LF|LOOPFLOW)_'       # what new tmux sessions inherit
```

## Machine and binary

| Variable | Set by | Read by | Policy |
|---|---|---|---|
| `LF_HOME` | A person, for an experiment; every lf launch path for its children; cron and CI-watch launchd plists; test fixtures | `store::lf_home_dir`, Desktop's observation client | Selects the data directory. Unset means `~/.lf`. The store is always `$LF_HOME/loopflow.db`, with its history and turns beside it in `$LF_HOME/history.db`. |
| `LF_BIN` | Desktop and lf launch paths, test fixtures | CLI dispatch and child launch | Explicit executable selection for private `LF_HOME` launches. Ordinary Sessions and agents receive the stable installed entry, or the Desktop helper that launched them; main-Machine dispatch ignores inherited selections. It grants no migration or process-control authority. |

Without a private `LF_HOME`, source commands forward to the installed CLI,
even with an inherited `LF_BIN`. The lf inside a Desktop app bundle
(`<App>.app/Contents/MacOS/lf`) is that app's own build: it runs itself and
its children on `~/.lf`, so a Dev app never pairs with an older installed lf. Store schema compatibility and migration
authority apply to every build; see [Machine selection](machines.md#one-main-machine).

## Process context

Loopflow sets these variables for its child processes. A session shell unsets all of it before
applying its launch's values, and the tmux client that may start the server
drops it too, so sessions a person opens by hand inherit none of it.

| Variable | Set by | Read by | Policy |
|---|---|---|---|
| `LF_CONTEXT_DELIVERY` | Native provider launch | Context and planning hooks | Private saved launch sources, separate from stable hook declarations. Scrubbed for independent launches and Codex app-server processes; headless Codex callbacks use their native thread binding. |
| `LF_CAPTURE_KEY` | Session capture and native resume | Capture lookup, provider callbacks, Task comments | Selects subordinate captured history in the resolved Machine. SQLite must record its owning Session; a present manifest must agree. It grants neither Task nor Graph authority. |
| `LF_TRACE_ID`, `LF_PROCESS_ID` | Journal, agent and session launch | Journal, git operations | Joins a child's events to its caller's trace. |
| `LF_AGENT_CALLER` | Session capture and native resume | Journal, once, then resolved process context | Carries Session identity, AgentProcess identity and origin Process for nested command ancestry and checkpoint composition. |
| `LF_AS` | `--as` | Run and Task commands | Declares the Work a command contributes to; resolved against the registry. |
| `LF_GRAPH_ID` | Graph process, for each agent step | The step's agent and skills | Names the Graph a step serves: its Graph process's id. Steps of one Graph share notes under it; `lf` reads nothing from it. |
| `LF_WAVE_ID` | Wave and Task launches | Wave resolution | Default Wave for a child command. |
| `LF_INSTALL_SWITCH` | Published install | `installation` | One-shot capability; must equal the id of the switch receipt in progress. |
| `LF_EXPECTED_MACHINE_ID` | `lf --machine` | `lf machine`, the remote preamble | Refuses a Machine-addressed command that reached a different Machine. |
| `LF_TERMINAL_ID`, `LF_TERMINAL_TTY` | Desktop's terminal | Session capture | Attach a Session to the terminal showing it. |
| `LF_USER_NAME` | lf launch paths | `config::participant_name` | The participant's display name. Presentation only. |
| `LOOPFLOW_DIRECTIVE_FILE` | Shell integration, agent launch with a relay | `lf` commands that ask the parent shell to act | Removed for agents unless a scoped relay is supplied. |
| `LOOPFLOW_GRAPH_NAME` | Graph process | Skill prose | Names the running Graph for the agent. |

## The caller's session

A conversation sets markers in its own shells: lf's process context above and
its provider's own. Every provider lf starts (Claude, Codex, OpenCode, Pi, Antigravity, in
chats, batches, work sessions and workers) drops the caller's, by name, through
`caller_session_env()` (shared lf markers plus each harness's
`Harness::caller_session_env()`), before applying the new conversation's.

| Set by | Removed |
|---|---|
| lf | the process-context variables above, `AI_AGENT` |
| Claude Code | `CLAUDECODE`, `CLAUDE_CODE_CHILD_SESSION`, `CLAUDE_CODE_SESSION_ID`, `CLAUDE_CODE_SESSION_ATTENDED`, `CLAUDE_CODE_ENTRYPOINT`, `CLAUDE_CODE_EXECPATH`, `CLAUDE_CODE_MESSAGING_SOCKET`, `CLAUDE_CODE_MESSAGING_TOKEN`, `CLAUDE_CODE_CHILD_NOTES_DIR`, `CLAUDE_PID`, `CLAUDE_EFFORT` |
| Codex | `CODEX_CI`, `CODEX_SANDBOX`, `CODEX_SANDBOX_NETWORK_DISABLED`, `CODEX_SESSION_ID`, `CODEX_THREAD_ID`, `CODEX_TOOL_CALL_ID`, `CODEX_VERSION`, `CODEX_APPLY_PATCH_PRESERVE_LINE_ENDINGS` |
| OpenCode | `OPENCODE`, `OPENCODE_SESSION_ID`, `OPENCODE_TERMINAL`, `OPENCODE_CONFIG_CONTENT` |
| Antigravity | `ANTIGRAVITY_AGENT`, `ANTIGRAVITY_AGENTAPI_EXE`, `ANTIGRAVITY_APP_DATA_DIR`, `ANTIGRAVITY_CONVERSATION_ID`, `ANTIGRAVITY_CSRF_TOKEN`, `ANTIGRAVITY_LS_ADDRESS`, `ANTIGRAVITY_LS_VERSION`, `ANTIGRAVITY_PROJECT_ID`, `ANTIGRAVITY_SOURCE_METADATA`, `ANTIGRAVITY_TRAJECTORY_ID` |

An inherited `CLAUDE_CODE_CHILD_SESSION` turns off an interactive Claude's
transcript (`-p` runs keep theirs), so the Session couldn't be resumed; an
inherited `CODEX_SANDBOX` gives a nested Codex its parent's sandbox. Settings a person chooses pass through:
`CLAUDE_CONFIG_DIR`, `CLAUDE_CODE_OAUTH_TOKEN`, `CLAUDE_CODE_USE_*`,
`ANTHROPIC_*`, `CODEX_HOME`, `CODEX_API_KEY`, `CODEX_ACCESS_TOKEN`, `OPENAI_*`,
and the `NO_COLOR`, `PAGER` and locale values Codex also sets. lf's hooks
still read `CLAUDE_CODE_MESSAGING_SOCKET`: they run inside the new provider,
which sets its own.

## Native authentication and forwarded credentials

Native clients own login selection. Explicit `forward.env` and `forward.files`
settings send named values and home files through SSH stdin, never in command
arguments. Forwarded variables enter the destination launch environment; detached
Session shells scrub transient credentials.

| Variable | Policy |
|---|---|
| `LF_SETTINGS` | User-supplied YAML preference mapping; below explicit CLI and above database/files. Inherited by child commands and forwarded remotely. Never include credentials. |
| `LF_AGENT_OVERRIDE` | User-supplied agent preference inherited by children and remote commands. Explicit CLI wins; Loopflow never exports a CLI or resolved default through this variable. Cleared for independent Session shells. Contains no credential. |
| `LF_TASK_SOURCE` | One SSH invocation's saved Task ID, identifier, optional tracker issue ID, branch and required commit. Contains no planning payload, local paths or execution authority. |
| `LF_DISCORD_TOKEN` | Chat bridge token; removed before any provider child. |
| `LF_CREDENTIAL_SOCKET` | Local service credential broker. |
| `GH_TOKEN`, `OPENCODE_API_KEY`, `CLAUDE_CODE_OAUTH_TOKEN`, `ANTHROPIC_API_KEY`, `CODEX_ACCESS_TOKEN`, `OPENAI_API_KEY` | Provider credentials, applied per program. Exported API-key variables are removed from Claude and Codex launches. |
| `CODEX_HOME`, `CLAUDE_CONFIG_DIR` | Native provider homes inherited from the caller. |

## Installation and tooling

| Variable | Read by | Policy |
|---|---|---|
| `LF_INSTALL_DIR`, `LF_INSTALL_CLI_ONLY`, `LF_APPLICATIONS_DIR` | `release/install.sh`, `lf install` | Installer destinations. |
| `LF_PROVIDER_TOKEN_KEY_PATH` | `store::token_crypto` | Location of the token encryption key. |
| `LF_PERF_OUTPUT` | `performance` | Existing directory for opt-in CLI process/SQLite volume receipts; no SQL or data values. |
| `LF_TRACE` | `ops::trace` | Emit operation traces instead of executing. |
| `LOOPFLOW_DEV_WAVE_REPO` | `scripts/loopflow-dev.py` | Repository a development app opens. |
| `LOOPFLOW_BUILD_*`, `LOOPFLOW_RELEASE_TAG` | `build.rs`, release scripts | Build provenance, compiled in. |
| `LF_RELEASE_*` | Release scripts | Inputs from `lf repo release` to its scripts. |

`LF_TEST_*`, `LF_PROBE_*`, `LOOPFLOW_UI_TEST_*` and
`LOOPFLOW_TEST_*` exist only in tests and their fixtures.

## Removed

`LF_DB_PATH`, `LF_CONTROL_BIN`, `LF_CONTROL_HOME`, `LF_CONTROL_DB_PATH`,
`LF_RUN_CONTEXT`, `LF_TASK_ORIGIN`, `LF_PARENT_RUN_ID`, `LF_RUN_LEASE`,
`LF_WORKTREE_WRITER_ID`, `LF_SSH_TARGET`, `LF_AGENT_INVOCATION_ID`,
`LF_INSTALL_PROMOTE_HOP`, `LF_HUMAN_SESSION`, `LF_HUMAN_SESSION_RUN`,
`LF_HUMAN_SESSION_RUN_BIND`,
`LF_EXTERNAL_TERMINAL`, the `LF_TASK_*` and `LF_PROJECT_*` lease trio, and the
`LF_LINEAR_*`, `LF_GITHUB_WEBHOOK_*` and `LF_LFD_ALLOW_NON_LOOPBACK` daemon
settings. Nothing reads them. An older running lf may still export some of
them; they are inert.
