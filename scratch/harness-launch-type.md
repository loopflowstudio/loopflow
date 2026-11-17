# LOO-924: one launch type per harness

## Problem

lf launches Claude Code, Codex, OpenCode and Pi, interactive or headless, as
the caller (bare) or for the Machine server. The `Harness` trait
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

LOO-913 (#1953, which removes account routing) is on main at 4a0691a9a0 and
merged into this branch. Add no account selection back. The original caller
inventory and dependency investigation are in
`3b735e67d6:scratch/harness-launch-type.md`.

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
| Pi, `Holder::Caller` | native TUI | lf (`--session-id`) | none |
| Pi, `Holder::Server` | native TUI + extension | provider (extension reports file) | none |
| Codex, `Holder::Caller` | plain `codex`, prompt in argv | provider (start hook, `reported_thread`) | none |
| Codex, `Holder::Server` | `codex --remote unix://…` after lf opens the app-server thread | provider | none (client may `exec`) |
| OpenCode | `opencode --server … --session` after lf starts the private server | provider | observer thread draining events; must wait here, never `exec` |

So the shared runner needs only one rule: a client with a companion waits in
this process, and every other client may `exec` into a held terminal.

Three axes, three words. `Holder` (Caller | Server) is who the provider
reports to. `Mode` is whether a native client or a stream driver runs.
`transport::Place` (Held | Detached | here) is which process runs the launch;
it stays private to `transport` and no harness sees it. "Held" means only
`Place::Held`.

**Mode is not `!batch`.** `is_interactive_run` (`run.rs:700`) is
`!batch && (--interactive || tty || no prompt)`, and `delegate` separately
decides `Place::Held` from `!batch`. The resolver keeps both inputs so neither decision
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
pub enum Mode { Interactive, Headless }

pub struct Launch<'a> {
    pub config: &'a AgentConfig,     // what to run; no holder, no attachment
    pub mode: Mode,
    pub holder: Holder,
    pub admitted: &'a Admitted,      // required
    pub start: Start<'a>,            // New | Resume { thread: &AgentSessionId }
    pub context_file: Option<&'a Path>,
}

pub struct Admitted { store: SqliteStore, session: String, attachment: SessionAttachment }
```

- `transport::delegate*` returns `Delegation::{Delegated, Here { mode, holder }}`
  instead of `bool`. The early return (inside a held terminal or a detached
  launcher) resolves them too, so a re-executed child gets the same holder.
  `Mode` comes from `is_interactive_run`, moved into `transport`;
  `Place::Held` still keys on `batch`.
- The holder has one input. Today `delegate` reads `named_work`
  (`bin/lf.rs:2534`: `task_run || --task/--wave`), while the launch sets
  `AgentConfig::holder` from `WorkSource::is_managed()` (`run.rs:52`, `:284`),
  from `named_work` (`run.rs:205`) and from `store.is_work`
  (`native_session.rs:68`). `run` reaches `run_bound_prompt` through
  `implicit_binding` only when no selector was given, so `delegate` saw
  `named_work = false` and the holder still comes from the binding's source.
  Slice 3 passes the launch's managed-work fact (`WorkSource::is_managed()`,
  or `store.is_work` for a reopened Session) into the resolver, and both
  `Place` and `Holder` read it. If any entry gives a different holder than
  today, record the entry and stop; that would be a behavior change.
- In-process headless callers that never delegate (commit message, PR text,
  sync execution, replay) build `Launch` with `Mode::Headless` and
  `Holder::Caller`. A Graph step for a Task is `Holder::Server`
  (`lf -b --task`); it goes through the resolver.
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
and call this. `SessionCommandOutcome::result` and `bare_exit_code` take the
`Holder` and the `HarnessKind`, not a program string.

`Outcome` is one struct both modes fill (exit status, interrupted, moved,
thread, answer or failure), replacing `SessionCommandOutcome` and
`AgentProcessResult`. The test: no caller matches on which mode produced it.
If the two cannot merge, drop the single entry point and keep
`native::run` and the driver loop as two functions over the same `Harness`
and `Launch`; do not add a mode-tagged result.

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
| `transcript/summary.rs:269` name match | `HarnessKind::provider()` |

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
  every harness can run for the Machine server; managed means
  `Holder::Server`, which `Launch` already carries.
- `install_hint` is copy, not behavior; keep it on `Capabilities` only
  because it removes a name `match`.

Second pass, against the working diff (runner deletion):

- No `Placement` type. The name already means the `machine:` setting
  (`config::placement::Placement`) and a pane's container
  (`open/workspace.rs`), and reads as `transport::Place`. `Launch` carries
  `mode` and `holder` as fields.
- `run_agent` is now headless only, so `ProcessConfig::auto` no longer
  selects anything inside execution. Its remaining reads are dead and go
  with the field in slice 3 (list under "Delete"). Mode lives in the CLI's
  `PromptBuild` until the resolver returns it.
- `agent::tests::unknown_provider_is_rejected` now asserts only
  `HarnessKind::parse(..).is_none()`, which
  `harness::tests::default_create_harness_rejects_unknown` covers. Delete it.
- The holder has two derivations today (see "Launch value"); the resolver
  takes one fact.
- `Outcome` was undefined; see "One entry point".

No open product choice.

## Delete, do not maintain

- `agent/mod.rs`: `ProcessConfig::auto` and its dead reads: the `process.auto &&`
  guard on the Claude-only warnings, `!process.auto ||` in the retry loop, the
  `"tui"` surface and the `begin_with_context` arm of `_begin_process_runtime`,
  and the `for auto in [false, true]` test loop. Confirm each is unreachable
  from `run.rs`'s interactive path before deleting.
  `AgentConfig::{holder, session_attachment}`; any remaining
  `pin_provider_account*`; `agent::tests::unknown_provider_is_rejected`.
- `run.rs`: the `!process.auto && holder == Caller` exit arm in
  `run_headless_prompt` (`run_prompt` returns before it when not `auto`).
- `native_session.rs`: `native_provider_attachment`'s Option handling,
  the program-name dispatch and provider `SessionCommand`.
- `agent/session.rs`: `ProviderInvocation`, `SessionCommand`,
  `build_session_command`, `native_connection_command`; its name-keyed tests
  become `HarnessKind::ALL` loops.
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

The duplicate interactive runner and its runtime-only launch/exit methods are
removed in the working diff. Their settled-attachment and late-exit checks now
exercise `agent_process::spawn_native` and the store fence on the surviving path.
The full design remains unfinished; the remaining internal slices are:

2. **Required admission.** `Admitted` and `admit`; remove both Option fields
   and `open_owner`; `admit_for_test` returns `Admitted`. Focused:
   `--lib harness::native_session::` and `--lib harness::agent_process::`.
3. **Mode and holder at delegate.** `Delegation`, `Mode`, one managed-work
   input; delete `AgentConfig::holder`, `ProcessConfig::auto` with its dead
   reads, `Holder: Default`. Focused:
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
- `rg -n 'fn run_interactive|ProviderInvocation|SessionCommand\b|session_attachment: Option|holder: crate::session::Holder' rust/loopflow-execution/` prints nothing.
  The unrelated Clap `loopflow::lf::SessionCommand` remains.
- `docs/dev/codebase.md` "Add a provider" says: add `<name>.rs`
  implementing `Harness`, add its `HarnessKind` variant and registry line, and
  add conformance fixtures.

## Checks

- implement (2026-11-16): `cargo test -p loopflow-execution --lib agent::tests::unknown_provider_is_rejected` — execution test binary built; 1 passed.
- implement (2026-11-16): built `loopflow_execution-* --exact` with `native_cleanup_records_wait_without_touching_a_replacement_or_remote_provider`, `a_failed_terminal_receipt_still_closes_only_the_owned_provider`, `a_settled_runtime_cannot_launch_another_native_provider` — 3 passed.

- implement (2026-11-16): `cargo fmt --check` and `git diff --check` — passed.

- gate: `uv run python scripts/test.py --reuse-passing --rust-filter 'test(harness::) | test(agent::) | test(transport::)'`; CI runs the suites.
- gate: the two `rg` commands above.
- demo/qa: real-terminal launches above (needs installed providers).

## Docs written (write-docs)

- `rust/loopflow-execution/src/harness/README.md` "One launch type per harness"
- `docs/dev/codebase.md` "Add a provider"

#1953 replaced the README's account paragraphs with "Native clients own
authentication."

Both describe the finished design, none of which exists yet (no `Launch`,
`Admitted`, `Mode` or `launch_matrix` in the tree). Realign (2026-11-16)
replaced the README's `Placement` with `mode` and `holder`. They must not merge
ahead of slice 4; implement corrects them to whatever it actually builds.
