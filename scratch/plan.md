# LOO-963: provider launches drop the caller's session markers

## Outcome

A `claude`, `codex`, `opencode` or `pi` that lf starts never inherits the
per-session markers of the Claude Code or Codex conversation whose shell ran
`lf`. Today an agent that runs `lf task run`, `lf -b`, `lf resume` or
`lf server run` hands its markers on, and Claude answers "Transcript saving is
off — inherited CLAUDE_CODE_CHILD_SESSION marker": no transcript, so the
Session can't be resumed. After this change those launches save transcripts
like any other.

Settings a person sets on purpose pass through: `CLAUDE_CONFIG_DIR`,
`CLAUDE_CODE_OAUTH_TOKEN`, `CLAUDE_CODE_USE_*`, `ANTHROPIC_*`, `CODEX_HOME`,
`CODEX_API_KEY`, `CODEX_ACCESS_TOKEN`, `OPENAI_*`. That rules out a prefix
scrub (`CLAUDE_CODE_*`, `CODEX_*`); the list is by name.

## The marker list (read 2026-11-17)

Read from real provider shells, names only:

- Claude Code 2.1.327, this agent's own shell: `CLAUDECODE`,
  `CLAUDE_CODE_CHILD_SESSION`, `CLAUDE_CODE_SESSION_ID`,
  `CLAUDE_CODE_SESSION_ATTENDED` (`0`), `CLAUDE_CODE_ENTRYPOINT` (`sdk-cli`),
  `CLAUDE_CODE_EXECPATH`, `CLAUDE_CODE_MESSAGING_SOCKET`,
  `CLAUDE_CODE_MESSAGING_TOKEN`, `CLAUDE_CODE_CHILD_NOTES_DIR`, `CLAUDE_PID`,
  `CLAUDE_EFFORT` (`medium`), `AI_AGENT` (`claude-code_2-1-327_agent`).
  The binary also knows `CLAUDE_CODE_FORCE_SESSION_PERSISTENCE` (the escape
  hatch the warning names); lf removes the marker rather than forcing it on.
- Codex 0.169.0, `codex exec` shell started with the Claude markers unset:
  `CODEX_CI`, `CODEX_SANDBOX`, `CODEX_SANDBOX_NETWORK_DISABLED`,
  `CODEX_SESSION_ID`, `CODEX_THREAD_ID`, `CODEX_TOOL_CALL_ID`, `CODEX_VERSION`,
  `CODEX_APPLY_PATCH_PRESERVE_LINE_ENDINGS`. A nested Codex seeing
  `CODEX_SANDBOX` / `CODEX_SANDBOX_NETWORK_DISABLED` takes the parent's
  sandbox as its own.
- Codex also sets `NO_COLOR`, `PAGER`, `GIT_PAGER`, `GH_PAGER`, `LC_ALL`,
  `LC_CTYPE`. A person may set these on purpose, so they pass through
  (made for review; see `questions.md`).

## Design

One list and one function, in `rust/loopflow-execution/src/harness/mod.rs`
beside `configure_agent_env`:

```rust
/// What a Claude Code or Codex conversation sets in its own shells. A
/// provider lf starts is a new conversation, not that one's child.
pub(crate) const PROVIDER_SESSION_ENV: [&str; 20] = [...];

pub(crate) fn remove_provider_session_env(command: &mut std::process::Command)
```

Applied last, after `config.env` is merged, so a captured environment can't
put a marker back:

| Path | Where | Covers |
|---|---|---|
| `configure_agent_env` | `harness/mod.rs` | Claude, Codex app-server, OpenCode, Pi headless and their version probes |
| `NativeClient::new` | `harness/native_session.rs` | every native session (chat, work session) for all four harnesses; this is the path LOO-959's thread came through |
| `start_held` | `transport.rs` | the environment a held terminal starts with, so the held `lf` and anything run in that terminal start clean |

The server needs no change of its own. Supervised, it runs under launchd
(`server.plist` sets only `LF_HOME`, `LF_BIN`, `LF_SUPERVISED`) or systemd,
neither of which inherits the installing shell. Run by hand from an agent
shell, its own environment keeps the markers, but every conversation its
recovery pass starts goes through `NativeClient::new` or
`configure_agent_env`, which remove them. Detached `lf` children
(`os_process/detached.rs`) are the same: they are `lf`, and their providers go
through the two functions.

`lf`'s own hooks keep reading `CLAUDE_CODE_MESSAGING_SOCKET`
(`lf/commands/context.rs` `claude_inbox`): they run inside the new provider,
which sets its own. That is why the scrub is at the spawn sites, not at `lf`
startup.

## Acceptance check

`harness/launch_matrix.rs` `every_harness_runs_all_four_launch_kinds` sets
every name in `PROVIDER_SESSION_ENV` in the test process, each stand-in
provider records which of them it sees, and every row (4 harnesses x chat,
batch, work session, worker) asserts none. Removing the call from either
function fails rows. A passthrough name (`CLAUDE_CONFIG_DIR`) is set too and
asserted present, so a later prefix scrub fails.

```bash
env $(env | grep '^LF_' | cut -d= -f1 | sed 's/^/-u /') \
  cargo test -p loopflow-execution --lib harness::launch_matrix:: < /dev/null
```

## Demo

From an agent shell (which carries `CLAUDE_CODE_CHILD_SESSION`), run a branch
`lf` batch with Claude; the thread it records has a transcript under
`~/.claude/projects/` and `lf resume` reopens it. On main the same command
records a thread with no transcript.

## What goes

- Nothing to delete: no earlier attempt at this scrub exists in Rust.
- `swift/LoopflowMac/Services/ProcessEnvironment.swift` strips
  `CLAUDE_CODE_*` / `CODEX_*` by prefix for Desktop panes. Left as is: it
  serves terminal panes a person types in, not lf's provider launches.

## Exclusions

- Repairing Sessions whose recorded thread has no transcript (LOO-959).
- Provider auth probes (`provider_auth`, `claude_trust`, `codex.rs` probe):
  they start no conversation.
- A Rex server autostarted from an agent shell gives its panes the markers;
  lf's launches in those panes are covered, a person's bare `claude` in one is
  not.

## Slices

1. List, function, the three call sites, and the launch-matrix assertion.
   This is the whole change; the matrix run shows whether any of the 16 cells
   reaches a provider some other way.
2. If a cell still sees a marker, follow that path to its spawn and route it
   through the function rather than adding a fourth call.
