# LOO-924: one launch type per harness

## Problem

lf launches Claude Code, Codex, OpenCode and Pi, interactive or headless, as
the caller (bare) or held for the Machine server. The `Harness` trait
(`harness/mod.rs`) covers only the headless, stream-driven launch. Interactive
launches are assembled from `match harness.as_str()` arms spread across
`agent/session.rs`, `harness/context.rs`, `harness/native_session.rs`,
`terminal_title.rs`, `skills/invocation.rs`, `transcript/raw.rs` and
`lf/commands/{run,replay}.rs`. Mode and holder are re-derived at each
entry point, and the attachment is an `Option` that gets refused at runtime.

Outcome (Task text): one per-harness type answers every launch question; mode
and holder resolved once at `transport::delegate`; `HarnessKind` is the only
place a harness name is compared; one interactive Claude path; attachment
required in the type; a fifth harness is one file.

## Risk findings (measured at 470da475a1)

**The second interactive Claude path is dead in production.** Every production
`run_agent` caller passes `auto: true`: `ops/commit.rs:209`, `ops/pr.rs:549`,
`ops/sync_execution.rs:374`, `replay.rs:118`, and `run.rs:872`, which
`run_prompt` reaches only after `if !built.process.auto` has returned through
`launch_session` (`run.rs:735`). Interactive `lf` always goes
`run_prompt → native_session::launch_session → session_command_status_with_env
→ run_native_session`. The non-auto branch of `_run_agent_once`
(`agent/mod.rs:1868-2018`), `run_interactive`, `spawn_agent_child`,
`ChildPidGuard`, `native_session::run_native_agent`, and
`LfProcessRuntime::{spawn_native_agent, record_native_agent_exit}`
(`process/runtime.rs:207-249`) are reached only from
`agent::tests::unknown_provider_is_rejected_in_both_launch_modes` and
`session_record/capture_tests.rs`. Deleting them changes no shipped
behavior. The "managed provider" disagreement (`agent/mod.rs:1947` Claude-only
vs `native_session.rs:555` Claude/Codex) is inside that dead branch and inside
account routing, which LOO-913 also deletes.

**LOO-913 must land first.** PR #1953 (`jack-heart/cli-prox`) deletes
about 24k lines including `provider_account/`, most of `provider_auth/`,
`subscription.rs`, and the account route in both launch paths. It was cut
before #1951 moved `harness/` and `agent/` into `rust/loopflow-execution`, and
on 2026-11-16 GitHub reported it `CONFLICTING`. Every slice here rewrites the
same functions. Implement starts from origin/main after #1953 merges; if it
hasn't, stack on `jack-heart/cli-prox` once that branch is synced onto main,
and never re-add account selection.

**The resume path admits lazily.** `session_command_status_with_env`
(`native_session.rs:379`) creates a capture and claims an attachment when
`command.attachment` is `None`. That is the "Option means admit later"
contract. Making the attachment required means moving that block into one
`admit` function that runs before the harness is chosen. Nothing new is
refused.

**Interactive shapes differ by mechanism, not by name.**

| harness | interactive client | who names the thread | companion while client runs |
|---|---|---|---|
| Claude | native TUI, prompt on stdin | lf (`--session-id`) | none; may `exec` into a held terminal |
| Pi bare | native TUI | lf (`--session-id`) | none |
| Pi held | native TUI + extension | provider (extension reports file) | none |
| Codex bare | plain `codex`, prompt in argv | provider (start hook, `reported_thread`) | none |
| Codex held | `codex --remote unix://…` after lf opens the app-server thread | provider | none (client may `exec`) |
| OpenCode | `opencode --server … --session` after lf starts the private server | provider | observer thread draining events; must wait here, never `exec` |

So the shared runner needs only one rule: a client with a companion waits in
this process, and every other client may `exec` into a held terminal.

**Mode is not `!batch`.** `is_interactive_run` (`run.rs:700`) is
`!batch && (--interactive || tty || no prompt)`, and `delegate` separately
decides Held from `!batch`. The resolver keeps both inputs so neither decision
changes.

**Usage parsing is duplicated.** `transcript/claude.rs:117` and
`harness/claude_decode.rs:296,325,500,691` each read
`input_tokens / cache_read_input_tokens / cache_creation_input_tokens`.

## Role models: what to take

- **ACP (Zed)**: the agent advertises capabilities at `initialize`
  (`loadSession`, prompt content types); the client never infers them from
  identity. Session new/load/prompt/cancel are the whole surface. Take the
  capability struct and new-vs-load as one `Start` value. Don't take the
  transport: ACP puts the client in the byte path of every turn, and lf must
  keep native TUIs and stay out of the byte path. That is why driving all four
  harnesses through ACP adapters (`claude-code-acp`, `codex-acp`) is ruled out.
- **LSP**: capabilities are data the core branches on. Take that `if caps.x`
  replaces `if name == "codex"` everywhere outside the harness's own file.
- **Terraform providers**: core holds a registry and never names a provider.
  Take one registry function, `HarnessKind::harness()`, as the only dispatch.
- **`llm`**: a model plugin is one class with flag attributes (`can_stream`,
  `supports_schema`) and one `execute`. Take this as the size to aim for: a
  harness file is a unit struct, a capability literal, and three methods.

## Design

### Source of truth

`HarnessKind` (enum, `harness/mod.rs`) keeps `parse`/`as_str`/`ALL` and gains
one method, `fn harness(self) -> &'static dyn Harness`. These are the only
places a harness name appears outside its own file and tests.

```rust
pub trait Harness: Sync {
    fn kind(&self) -> HarnessKind;
    fn capabilities(&self) -> Capabilities;
    /// Native client for an interactive launch: the command, and any
    /// companion lf keeps alive while it runs (Codex/OpenCode servers are
    /// started here).
    fn open(&self, launch: &Launch<'_>) -> Result<NativeClient>;
    /// Stream driver for a headless launch (today's trait, renamed).
    fn driver(&self, events: mpsc::UnboundedSender<ConversationEvent>,
              approval: ApprovalPolicy) -> Box<dyn Driver>;
}

pub struct Capabilities {
    pub thread: ThreadNaming,          // { caller, server }: Assigned | Reported
    pub same_turn_input: bool,         // send_current can succeed
    pub native_skills: Option<SkillDialect>,
    pub turn_limit: bool,              // Claude only today
    pub output_schema: bool,           // Claude only today
    pub respawns_after_crash: bool,    // replaces is_terminal_harness_error's list
    pub install_hint: &'static str,    // replaces missing_agent_message's match
}
```

`Driver` is the current `Harness` trait renamed: start, send_input,
send_current, interrupt, stop, agent_session, pid, process_group_id,
set_agent_session, set_runtime. `ClaudeHarness` and the others become
`claude::Driver` and so on. LOO-913 removes the account methods.

### Launch value

```rust
pub struct Placement { pub mode: Mode, pub holder: Holder }   // Copy
pub enum Mode { Interactive, Headless }

pub struct Launch<'a> {
    pub config: &'a AgentConfig,     // what to run; no holder, no attachment
    pub placement: Placement,
    pub admitted: &'a Admitted,      // required
    pub start: Start<'a>,            // New | Resume { thread: &AgentSessionId }
    pub context_file: Option<&'a Path>,
}

pub struct Admitted { store: SqliteStore, session: String, attachment: SessionAttachment }
```

- `transport::delegate*` returns `Delegation::{Delegated, Here(Placement)}`
  instead of `bool`. It already computes `works(named_work, session)`; the
  early return (inside a held terminal or a detached launcher) computes it too,
  so a re-executed child gets the same holder. `Mode` comes from
  `is_interactive_run`, moved into `transport`; Held still keys on `batch`.
- In-process headless callers that never delegate (commit message, PR text,
  sync execution, replay, Graph steps through `ops/run.rs`) use
  `Placement::HEADLESS_CALLER`.
- `Admitted` replaces the `AttachmentOwner` tuple and both
  `Option<(String, SessionAttachment)>` fields. One `admit(…) -> Result<Admitted>`
  holds today's lazy block from `session_command_status_with_env`, and the
  runtime path wraps `LfProcessRuntime::session_attachment()`. Claude headless
  respawn keeps getting fresh attachments through `Driver::set_runtime`.

### One entry point

`harness::launch(launch: Launch) -> Result<Outcome>`:

- `Mode::Interactive` → `native::run(harness.open(&launch)?, &launch)`:
  today's `run_native_session` tail made harness-agnostic (terminal record,
  folder-trust report, hooks, title, transport `exec` handoff unless a
  companion exists, spawn/wait, exit record, move detection, reported thread).
- `Mode::Headless` → today's `_run_harness_once` with `harness.driver(...)`.

`run_prompt`, `resume_session_with_env` and `run_agent` all build a `Launch`
and call this. `SessionCommandOutcome::result` and `bare_exit_code` take
`Placement` and the `HarnessKind`, not a program string.

### Per-harness files absorb the tables

| today | moves to |
|---|---|
| `agent/session.rs` `build_session_command` match, `ProviderInvocation::ClaudeStream`, `native_connection_command` | each harness's `open` (and Claude's driver args) |
| `harness/context.rs` `native_args` match, `configure_hooks` filter `"claude"\|"codex"\|"pi"` | each harness's `open` |
| `native_session.rs` `program == "codex"/"opencode"` dispatch, `run_codex_native`, `run_opencode_native`, `open_codex_native` | `codex::open`, `opencode::open` |
| `claude_trust` calls gated on `program == "claude"` | `claude::open` |
| `terminal_title.rs:257` match | a `title_args` step in each `open` |
| `skills/invocation.rs` `native_for(&str)`, `native_declarations(&str)` | take `Capabilities::native_skills` |
| `run.rs:740` `names_thread` | `capabilities().thread` |
| `agent/mod.rs` `run_admitted_agent` Claude-only warnings | `turn_limit` / `output_schema` |
| `transcript/raw.rs:194-221` process recognition | `HarnessKind::parse` plus a per-harness argv check |
| `config/settings.rs:311-344` OpenCode model rule | `opencode.rs` |
| `replay.rs:185-266` per-harness defaults | capabilities + `HarnessKind::ALL` |
| Claude usage keys in two decoders | one `claude::usage(&Value)` |

### Concept review (2026-11-16, headless)

Settled by the design, for implement:

- `Start::New` carries no thread. An Assigned thread id is the UUID in the
  capture's artifact key (`run.rs:743-753` strips `run_` today), so `admit`
  owns it: `Admitted::assigned_thread()`, and `harness::launch` passes it to
  `open` when `capabilities().thread` is `Assigned` for the placement's
  holder. Callers never choose or pass one; `run.rs`'s `names_thread` block
  is deleted, not moved.
- `ThreadNaming` is a pair `{ caller, server }` of `Assigned | Reported`,
  because Pi differs by holder. It stays data, not a method.
- "Whether it can be managed" (Task text) is not a capability. After LOO-913
  every harness can be held; managed means `Holder::Server`, which
  `Placement` already carries.
- `install_hint` is copy, not behavior; keep it on `Capabilities` only
  because it removes a name `match`.

No open product choice. Implement still waits on #1953 (see
`scratch/questions.md`).

## Delete, do not maintain

- `agent/mod.rs`: non-auto branch of `_run_agent_once`, `run_interactive`,
  `spawn_agent_child`, `ChildPidGuard`; `ProcessConfig::auto`;
  `AgentConfig::{holder, session_attachment}`; `pin_provider_account*` if
  LOO-913 left any.
- `native_session.rs`: `run_native_agent`, `native_provider_attachment`'s
  Option handling, the program-name dispatch, `SessionCommand`.
- `agent/session.rs`: `ProviderInvocation`, `SessionCommand`,
  `build_session_command`, `native_connection_command`; its name-keyed tests
  become `HarnessKind::ALL` loops.
- `process/runtime.rs`: `spawn_native_agent`, `record_native_agent_exit` and
  the `capture_tests.rs` cases that call only them. The behavior those cases
  pin (a settled runtime launches nothing; a replaced attachment cannot record
  an exit) is re-asserted against `agent_process::spawn_native`, which is the
  path that survives.
- `harness/agent_process.rs`: `open_owner(Option)` and its "requires an
  admitted invocation" refusal, plus `mod.rs`'s
  `unattached_launch_is_refused_before_any_provider_starts`, which the type
  now makes unrepresentable.
- `harness/mod.rs`: `default_create_harness`, `canonical_harness` (callers use
  `HarnessKind::parse`), `is_terminal_harness_error`'s string list.
- `Holder`'s `Default` derive, plus `Holder::of` callers outside `transport`.
- `agent::missing_agent_message` match.
- Docs: `docs/dev/codebase.md` "Add a provider" steps 1-2 (account routing is
  gone after LOO-913) and `harness/README.md`'s account paragraphs.

## Forbidden outcomes

- `SessionCommand` or `ProviderInvocation` surviving beside `NativeClient`/`Start`.
- `Launch` or `Admitted` holding an `Option` attachment, or a constructor that
  admits lazily.
- A `match kind { … }` outside `HarnessKind::harness()` and `parse`/`as_str`.
  Branch on `Capabilities` instead.
- `Holder::of` or `is_interactive_run` called anywhere but the resolver.
- A test-only harness variant or factory trait. Fixtures use the existing
  stand-in provider scripts.

## Exclusions

- Swift display names (`SessionProgramStatus.swift:115`,
  `WorkspacePanesView.swift:608`): UI labels, not launch decisions.
- `loopflow_storage::provider::Provider` (provider homes, not harnesses).
- Session move, resume-explanation and Desktop relay behavior beyond the
  signature changes.

## Slices (one PR)

1. **This slice: delete the dead interactive path.** Everything in the first
   risk finding, plus `unknown_provider_is_rejected_in_both_launch_modes`,
   whose headless half becomes a `HarnessKind::parse` rejection test. Focused:
   `cargo test -p loopflow-execution --lib agent::` and
   `--lib session_record::capture_tests`.
2. **Required admission.** `Admitted` and `admit`; remove both Option fields
   and `open_owner`; `admit_for_test` returns `Admitted`. Focused:
   `--lib harness::native_session::` and `--lib harness::agent_process::`.
3. **Placement at delegate.** `Delegation`, `Placement`, `Mode`; delete
   `AgentConfig::holder`, `ProcessConfig::auto`, `Holder: Default`. Focused:
   `cargo test -p loopflow --lib lf::commands::run::` and
   `--lib transport::`.
4. **`Harness` + `Driver` + per-harness `open`.** Move the table rows above;
   one `harness::launch`; shared `native::run`. Focused:
   `--lib harness::conformance_tests` plus a new `harness::launch_matrix` test.
5. **Leftovers.** Claude usage once, replay, raw process recognition, settings
   rule, docs.

## Demo

```bash
cargo test -p loopflow-execution --lib harness::launch_matrix -- --nocapture
```

For every `HarnessKind::ALL × {Interactive, Headless} × {Caller, Server}`, the
matrix runs one `harness::launch` against the stand-in provider scripts and
prints a row: argv, hook flags, whether it `exec`ed or waited, and the thread
lf recorded. That table is the launch contract, and today it is
spread over six files. Then, in a real terminal, run `lf -a claude 'say hi'`,
`lf -a pi 'say hi'`, `lf -b -a codex 'say hi'` and `lf resume <id>`. They
behave exactly as before.

## Done when

- `lf`, `lf -b`, `lf resume`, `lf --task …` behave as before for all four
  harnesses. The launch matrix passes and names each cell.
- This prints only `HarnessKind::parse`/`as_str` and `#[cfg(test)]` code:
  `rg -n --type rust '(==|!=|=>)\s*"(claude|codex|opencode|pi)"|"(claude|codex|opencode|pi)"\s*(==|!=|=>|\|)' rust/*/src -g '!*test*'`
  (baseline at 470da475a1: 83 non-test comparisons in 21 files).
- `rg -n 'fn run_interactive|ProviderInvocation|SessionCommand\b|session_attachment: Option|holder: crate::session::Holder' rust/` prints nothing.
- `docs/dev/codebase.md` "Add a provider" says: add `<name>.rs`
  implementing `Harness`, add its `HarnessKind` variant and registry line, and
  add conformance fixtures.

## Checks

- gate: `uv run python scripts/test.py --reuse-passing --rust-filter 'test(harness::) | test(agent::) | test(transport::)'`; CI runs the suites.
- gate: the two `rg` commands above.
- demo/qa: real-terminal launches above (needs installed providers).

## Docs written (write-docs)

- `rust/loopflow-execution/src/harness/README.md` "One launch type per harness"
- `docs/dev/codebase.md` "Add a provider"

The README's account paragraphs are left for LOO-913 (#1953) to delete, to avoid a conflict.

Both describe the finished design, none of which exists yet (no `Launch`,
`Admitted`, `Placement` or `launch_matrix` in the tree). They must not merge
ahead of slice 4; implement corrects them to whatever it actually builds.
