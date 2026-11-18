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

The concept is the caller's session: the markers a conversation sets in its
own shells, whether lf set them (`LF_CAPTURE_KEY`, `LF_STEER_TASK`, ...) or
its provider did (`CLAUDE_CODE_CHILD_SESSION`, `CODEX_THREAD_ID`, ...). A
conversation lf starts is a new session, never that one's child. lf already
has this list for its own markers, `EXECUTION_IDENTITY_ENV` in
`agent/mod.rs`; Claude's and Codex's markers are more of the same, so they
join it rather than getting a second list (chosen in concept review; see
`concept-review.html`).

```rust
/// What a conversation sets in its own shells, lf's markers and its
/// provider's. A conversation lf starts sets its own; it never inherits
/// the caller's.
pub(crate) const CALLER_SESSION_ENV: [&str; 32] = [...];

pub(crate) fn remove_caller_session_env(command: &mut std::process::Command)
```

Rename `EXECUTION_IDENTITY_ENV` to `CALLER_SESSION_ENV` in place, add the 20
provider names, and replace each hand-written loop with the function:

| Site | Today | After |
|---|---|---|
| `configure_agent_env` (`harness/mod.rs`) | loops over the lf list | calls the function; covers Claude, Codex app-server, OpenCode, Pi headless and their probes |
| Codex app-server (`harness/codex.rs:780`) | loops over the lf list | calls the function |
| `NativeClient::new` (`harness/native_session.rs`) | removes nothing of either list | calls the function; every native chat and work session, all four harnesses (LOO-959's path) |
| `transport.rs:384` `start_held` caller | `&\|_\| {}` | unchanged (see Slices: the held `lf` is the caller's own invocation) |
| `agent/mod.rs:960` | strips the lf list from `launch.env` | same loop over the renamed list (a map, not a `Command`) |

Order: the function runs on the inherited environment, before
`.envs(&config.env)`, as `configure_agent_env` already does. lf's new
markers for the new conversation arrive in `config.env`; removing after it
would delete them. Provider markers can't come back through `config.env`:
the map is stripped at `agent/mod.rs:960`, and `conversation_environment`
records only `config.env` keys plus `PATH`/`LF_BIN`/`LF_HOME`.

Risk for slice 1: `NativeClient::new` starts removing lf's own markers it
used to inherit. A native work session that read one from the held `lf`'s
environment rather than `config.env` loses it; the matrix's turn-hook
assertions show it.

The server needs no change of its own. Supervised, it runs under launchd
(`server.plist` sets only `LF_HOME`, `LF_BIN`, `LF_SUPERVISED`) or systemd,
neither of which inherits the installing shell. Run by hand from an agent
shell, every conversation it starts goes through the function. Detached `lf`
children (`os_process/detached.rs`) are `lf`; their providers go through the
same sites.

`lf`'s own hooks keep reading `CLAUDE_CODE_MESSAGING_SOCKET`
(`lf/commands/context.rs` `claude_inbox`): they run inside the new provider,
which sets its own. That is why the scrub is at spawn sites, not at `lf`
startup.

## Acceptance check

`harness/launch_matrix.rs` `every_harness_runs_all_four_launch_kinds` sets
every name in `CALLER_SESSION_ENV` to the value `caller` in the test process;
each stand-in provider records those names' values, and every row (4
harnesses x chat, batch, work session, worker) asserts none equals `caller`.
lf's markers that the new conversation sets fresh pass; inherited ones of
either kind fail. Removing the call from any site fails rows. A passthrough
name (`CLAUDE_CONFIG_DIR`) is set too and asserted present, so a later prefix
scrub fails.

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

- `EXECUTION_IDENTITY_ENV` and its three hand-written removal loops become
  `CALLER_SESSION_ENV` and one function; no second list.
- `swift/LoopflowMac/Services/ProcessEnvironment.swift` strips
  `CLAUDE_CODE_*` / `CODEX_*` by prefix for Desktop panes. Left as is: it
  serves terminal panes a person types in, not lf's provider launches.

## Exclusions

- Repairing Sessions whose recorded thread has no transcript (LOO-959).
- The `LF_`-prefix filters in `server/service.rs` (session resume) and
  `installation/server.rs` (server refresh) choose which `lf` home a child
  uses, then set `LF_HOME`/`LF_BIN`; a different job, left as is.
- Provider auth probes (`provider_auth`, `claude_trust`, `codex.rs` probe):
  they start no conversation.
- A Rex server autostarted from an agent shell gives its panes the markers;
  lf's launches in those panes are covered, a person's bare `claude` in one is
  not.

## Slices

Slice 1 built (2026-11-17): the list is `CALLER_SESSION_ENV` (32 names) with
`remove_caller_session_env`; `configure_agent_env`, the Codex app-server and
`NativeClient::new` call it; `agent/mod.rs` strips the map. The matrix plants
every provider marker as `caller` plus `CLAUDE_CONFIG_DIR` and passes all 16
cells; removing the `NativeClient::new` call fails `Claude Chat` at once. No
cell reached a provider another way, so slice 2 has nothing to do.

What it taught:
- `start_held` stays `&|_| {}`. The held `lf` is the delegating invocation
  continued in a terminal; it joins its parent's trace through
  `LF_TRACE_ID`/`LF_PROCESS_ID` (`journal/mod.rs` parent resolution), so
  removing lf's markers there would orphan it. The provider it starts goes
  through `NativeClient::new`.
- The matrix can't plant lf's own markers: the test process is lf and reads
  them (`LF_CAPTURE_KEY=caller` fails with "invalid capture key"). It plants
  the provider names; lf's names keep their coverage from the `managed` hook
  assertions.

Remaining: the Demo, run by hand (demo or gate step; a headless run
here can't show a person the transcript). Nothing remains to build.

## Docs

Written ahead of the build: `docs/sessions.md` ("Run one headless", one
paragraph) and `docs/dev/environment.md` ("The caller's session", the list and
the passthrough names). The table in `environment.md` matches
`CALLER_SESSION_ENV` (checked 2026-11-17).
