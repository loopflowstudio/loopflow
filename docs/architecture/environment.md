# Environment

Environment configures a process; it never decides what the process is. A
variable that names a Home, an Exec or a claim is checked against the store or
the filesystem before it grants anything, and a new session starts without it.

```bash
LF_HOME="$(mktemp -d)" target/debug/lf wave list --json   # the only Home selector
env | grep -E '^(LF|LOOPFLOW)_'                            # what this process inherited
tmux show-environment -g | grep -E '^(LF|LOOPFLOW)_'       # what new tmux sessions inherit
```

## Home and binary

| Variable | Set by | Read by | Policy |
|---|---|---|---|
| `LF_HOME` | A person, for an experiment; every lf launch path for its children; cron and CI-watch launchd plists; test fixtures | `store::lf_home_dir`, Desktop's observation client | Selects the data directory. Unset means `~/.lf`. The database is always `$LF_HOME/loopflow.db`. |
| `LF_BIN` | lf launch paths, test fixtures | `engine::process::resolve_lf_binary`, only when `LF_HOME` selects an experiment | Names the experiment's binary. Under the main Home lf ignores it and uses the installed CLI, so a stale value cannot choose the wrong binary for ordinary work. |

A source build with no explicit `LF_HOME` forwards to the installed CLI and
main Home before opening a store. Development builds refuse the main database.

## Exec context

Set by lf for its own children. A session shell unsets all of it before
applying its launch's values, and the tmux client that may start the server
drops it too, so sessions a person opens by hand inherit none of it.

| Variable | Set by | Read by | Policy |
|---|---|---|---|
| `LF_RUN_ID`, `LF_RUN_DIR` | Session capture, agent launch | `session_record::inherited_caller`, `lf runs`, Task lifecycle, direct-skill checkpointing, Task comments | Parent attribution. `inherited_caller` honours it only when the manifest in `LF_RUN_DIR` names the same id. Three readers still treat its presence alone as "inside a Run": the direct-skill checkpoint, branch-Task lookup and the progress marker on Task comments. |
| `LF_TRACE_ID`, `LF_PROCESS_ID` | Journal, agent and session launch | Journal, git operations | Joins a child's events to its caller's trace. |
| `LF_AGENT_CALLER` | Session capture | Journal, once, then removed | Provenance of the agent that issued a nested command. |
| `LF_AS` | `--as`, Task worker launch | Run and Task commands | Declares the Work a command contributes to; resolved against the registry. |
| `LF_WAVE_ID` | Wave and Task launches | Wave resolution | Default Wave for a child command. |
| `LF_FLOW_STEP` | Flow driver | `ops::flow_run::token` | Fences a step to one Flow position; validated against the saved Flow. |
| `LF_WORK_ADVANCE_CLAIM` | Task worker launch | Flow and run commands, once, then removed | One-shot claim; must match the Task's stored claim. |
| `LF_TASK_SKILL_OPTIONS` | `lf task` commands | Task worker, once | Carries step flags across the worker launch. |
| `LF_HUMAN_SESSION`, `LF_HUMAN_SESSION_RUN`, `LF_REVIEW_RUN_RESERVATION` | Conversation and review launch | `ops::human_session`, removed on use | Identify the prepared conversation a new terminal opens. |
| `LF_GIT_OPERATION_ID` | `ops::git_operation` | Nested lf commands inside an owned git operation | Lets recovery continue its own operation; checked against the worktree's record. |
| `LF_PROVIDER_ACCOUNT_ID` | Provider launch | `lf runs` | Records which account a provider child used. |
| `LF_INSTALL_SWITCH` | Published install | `machine_install` | One-shot capability; must equal the id of the switch receipt in progress. |
| `LF_EXPECTED_HOME_ID` | `lf ssh` | `lf home`, the remote preamble | Refuses a Home-addressed command that reached a different Home. |
| `LF_TERMINAL_ID`, `LF_TERMINAL_TTY` | Desktop's terminal | Session capture | Attach a Session to the terminal showing it. |
| `LF_USER_NAME` | lf launch paths | `engine::config::participant_name` | The participant's display name. Presentation only. |
| `LOOPFLOW_DIRECTIVE_FILE` | Shell integration, agent launch with a relay | `lf` commands that ask the parent shell to act | Removed for agents unless a scoped relay is supplied. |
| `LOOPFLOW_FLOW_NAME` | Flow driver | Skill prose | Names the running Flow for the agent. |

## Forwarded authority

Credentials and account authority handed to exactly one process. Session
shells unset them and tmux never receives them.

| Variable | Policy |
|---|---|
| `LF_ACCOUNT_LEASE`, `LF_ACCOUNT_SELECTION`, `LF_ACCOUNT_LEASE_SOCKET` | Account route for one provider launch or one `lf ssh` command. |
| `LF_FORWARDED_PM_TOKEN`, `LF_FORWARDED_PM_PROVIDER`, `LF_FORWARDED_SECRET_NAMES` | Planning credential and named secrets carried over `lf ssh`. |
| `LF_DISCORD_TOKEN` | Chat bridge token; removed before any provider child. |
| `LF_CREDENTIAL_SOCKET`, `LF_AUTH_BROWSER_FIFO` | Local credential broker and browser handoff for login. |
| `GH_TOKEN`, `OPENCODE_API_KEY`, `CLAUDE_CODE_OAUTH_TOKEN`, `ANTHROPIC_API_KEY`, `CODEX_ACCESS_TOKEN`, `OPENAI_API_KEY` | Provider credentials, applied per program. |
| `CODEX_HOME`, `CLAUDE_CONFIG_DIR` | Set per provider child to the managed account's directory. |

## Installation and tooling

| Variable | Read by | Policy |
|---|---|---|
| `LF_INSTALL_DIR`, `LF_INSTALL_CLI_ONLY`, `LF_APPLICATIONS_DIR` | `release/install.sh`, `lf install` | Installer destinations. |
| `LF_PROVIDER_TOKEN_KEY_PATH` | `store::token_crypto` | Location of the token encryption key. |
| `LF_NPX_BIN` | Skill discovery | Substitute `npx`. |
| `LF_PERF_OUTPUT` | `performance` | Existing directory for opt-in CLI process/SQLite volume receipts; no SQL or data values. |
| `LF_TRACE` | `ops::trace` | Emit operation traces instead of executing. |
| `LOOPFLOW_DEV_WAVE_REPO` | Metrics, Desktop portfolio discovery | Repository a development app opens. |
| `LOOPFLOW_BUILD_*`, `LOOPFLOW_MIGRATION_AUTHORITY`, `LOOPFLOW_RELEASE_TAG` | `build.rs`, release scripts | Build provenance, compiled in. |
| `LF_RELEASE_*` | Release scripts | Inputs from `lf release` to its scripts. |

`LF_TEST_*`, `LF_PROBE_*`, `LF_SCREENSHOT_*`, `LOOPFLOW_UI_TEST_*` and
`LOOPFLOW_TEST_*` exist only in tests and their fixtures. The three
`LF_TEST_CLAUDE_*_URL` readers in `subscription.rs` are compiled only under
`cfg(test)`.

## Removed

`LF_DB_PATH`, `LF_CONTROL_BIN`, `LF_CONTROL_HOME`, `LF_CONTROL_DB_PATH`,
`LF_RUN_CONTEXT`, `LF_TASK_ORIGIN`, `LF_PARENT_RUN_ID`, `LF_RUN_LEASE`,
`LF_WORKTREE_WRITER_ID`, `LF_SSH_TARGET`, `LF_AGENT_INVOCATION_ID`,
`LF_INSTALL_PROMOTE_HOP`, `LF_HUMAN_SESSION_RUN_BIND`,
`LF_EXTERNAL_TERMINAL`, the `LF_TASK_*` and `LF_PROJECT_*` lease trio, and the
`LF_LINEAR_*`, `LF_GITHUB_WEBHOOK_*` and `LF_LFD_ALLOW_NON_LOOPBACK` daemon
settings. Nothing reads them. An older running lf may still export some of
them; they are inert.
