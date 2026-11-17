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
merged into this branch. Add no account selection back.

#1955 (528ccd8eda) is merged into this checkout. `native_exit::record_exit`
uses main's `Result<()>` body with `&Admitted`; native interruption no longer
reads a stored AgentProcess outcome. Main also supplies `PromptBuild::skill_version`.
Earlier merge analysis is in b9542ecef5:scratch/harness-launch-type.md.

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

**Usage parsing** now shares `claude::usage(&Value)` between stream and
transcript decoding. Missing input remains distinct from zero; cache-only
observations still contribute to peak pressure.

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
    /// Stream driver for a headless launch (today's trait, renamed), built
    /// admitted from the launch.
    fn driver(&self, launch: &Launch<'_>,
              events: mpsc::UnboundedSender<ConversationEvent>,
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

- `transport::delegate*` returns `Delegation::{Delegated, Here { mode }}`
  instead of `bool`. The early return (inside a held terminal or a detached
  launcher) resolves the mode too. `LaunchInput::mode` owns the former
  `is_interactive_run` predicate; `Place::Held` still keys on `batch`.
- Place and Holder read two facts, each derived once in `transport`.
  **Place** reads whether the command names its Work (`named`: `task run`,
  `--task`, `--wave`, or a reopened Session that is not bare), as `delegate`
  does today. **Holder** reads whether the launch's Work source is managed
  (`WorkSource::is_managed()`, or `store.is_work` for a native reopened Session).
  A headless resume by ID uses only the command's named Work, as today; native
  resume consults stored Session Work. Inherited `LF_AS` outside a Task
  checkout binds as `Declared`, so it runs here (`named = false`) with
  `Holder::Server`.
  A Graph step is already `Server` and here, so that pair is not new. This
  Task keeps every entry as it is ("behave as before"); the entry gets its own
  row in the launch matrix so the next change to it is deliberate.
- `delegate` runs before the binding is resolved, so it cannot know the
  source. `Delegation::Here { mode }` carries the mode; one
  `transport::holder(work, source)` returns the `Holder` once the
  binding exists, and it is the only caller of `Holder::of`. A caller that
  holds a Session record reads `store.is_work` itself and passes the answer. It replaces the
  four derivations (`run.rs:52`, `:205`, `:285`, `native_session.rs:96`). A
  held or detached child re-executes the same argv and reaches the same
  function. Do not resolve the implicit binding before `delegate` to merge the
  two: that adds a store read ahead of delegation for every bare launch.
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

Settled by the design:

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
- The holder has four derivations today (see "Launch value"); one function
  replaces them.
- `Outcome` was undefined; see "One entry point".

Third pass, against the admission diff:

- Place and Holder are two facts (see "Launch value"). The earlier "one
  managed-work input" would have moved inherited `LF_AS` outside a Task
  checkout from here to held or detached. Slice 3 is unblocked.
- `Harness::driver` takes the `Launch`. Today `claude::` and `pi::` drivers
  are built empty and hold `owner: Option<Admitted>` with a "has not started"
  error, because `start(config, admitted)` delivers it late. Built from a
  `Launch`, a driver owns its `Admitted` from construction; `Driver::start`
  takes no arguments and both `owner() -> Result` helpers go.
- `Admitted::from_runtime` still fails with "invocation has not claimed its
  Session attachment" when `LfProcessRuntime::admitted()` is `None`. Slice 4
  checks whether an agent launch can reach that; if only non-agent captures
  have no attachment, the agent entry points take a runtime type that always
  has one. `native_exit::settle`'s `Option<Admitted>` stays: there `None` is a
  store that did not open, a recording gap.

Slice 3 counterexample (2026-11-16): headless `lf -b resume ID --message`
reaches `run::resume` with `named_work` derived only from CLI selectors in
`lf/commands/session.rs::resume`. It neither delegates by stored Session Work
nor uses it for Holder. The same ID can therefore be Caller headless and Server
native. The resolver now retains that distinction; the matrix needs separate
headless/native resume-by-ID rows. The earlier blanket statement about reopened
Sessions was incorrect. This changes the written design, not launch behavior.

Fourth pass, against 1c3af13938 (mode and holder in transport). Two changes
for the next implement, both before slice 4:

- **`transport::holder` takes the Work fact, not a Session id.** Native resume
  (`native_session.rs:95`) used to call `store.is_work(named_work,
  Some(&session))?` on the store and record it already holds. It now passes
  the id, and `transport::works` opens `journal::process_ledger()` again,
  re-reads the Session, and turns any failure into Work. A store error that
  used to stop the resume now launches it as `Holder::Server`: a holder that
  differs from main, which "Forbidden outcomes" rules out. Make it
  `holder(work: bool, source: Option<WorkSource>)`; native resume passes
  `store.is_work(named_work, Some(&session))?`, headless resume passes
  `named_work`, bound launches pass `false` and the source. `works(…, session)`
  stays private to `delegate`, where no record is in hand and an unreadable
  store must not block the launch.
- **Mode is an argument, not a `Cli` field.** `Cli::launch_mode:
  Option<Mode>` plus the `unwrap_or_else(… .preview_mode())` in
  `build_prompt_at` is "Option means resolve later" again, with a second
  `has_prompt` expression (`skill.is_some() || message.is_some()` against
  dispatch's `command.is_some() || skill_input.is_some()`; equal on every
  entry checked today, by coincidence of `lf.rs:2974`). `process_options()`
  copies the field, so a caller that then sets `batch` (`ship.rs:55`) would
  lose to a stale mode if that command ever delegated. Pass `Mode` from
  dispatch through `execute_target`/`execute_command` into
  `run`/`run_bound`/`resume` and `build_prompt*`; callers that never delegate
  state theirs (preview: `LaunchInput::mode()`; ship and Discord: `Headless`).
  Delete the field and the fallback. `LaunchInput` keeps one public `mode()`
  that reads the terminal (today's `preview_mode`, which `delegate` also
  calls) and a private tty-parameter form for the table test.

Checked and unchanged: Graph steps spawn `lf` children from `step_args`, so no
resolved mode crosses into a step; `place` matches the former inline branch
row for row.

Open, outside this Task (not blocking): what inherited `LF_AS` outside a Task
checkout should be. `server/README.md` "Turn and tool boundaries" and
`transport/README.md` "Which launches are managed" list the managed entries
and say every other launch is bare, a child run a Session's agent starts
included; the code makes this one `Holder::Server`. But lf also sets `LF_AS`
on purpose for workers it starts (`pr_landing.rs:354`, the ci-fix worker), and
`task/delivery.rs` calls it "an explicit declaration". Either the READMEs gain
this entry or the inherited case binds as `WorkSource::Inherited`; the second
changes hooks and the server requirement for those child runs.

## Delete, do not maintain

- `agent/mod.rs`: `AgentConfig::holder`; any remaining `pin_provider_account*`.
- `native_session.rs`: the program-name dispatch and provider `SessionCommand`.
- `agent/session.rs`: `ProviderInvocation`, `SessionCommand`,
  `build_session_command`, `native_connection_command`; its name-keyed tests
  become `HarnessKind::ALL` loops.
- `harness/mod.rs`: `default_create_harness`, `canonical_harness` (callers use
  `HarnessKind::parse`), `is_terminal_harness_error`'s string list.
- `Holder`'s `Default` derive, plus `Holder::of` callers outside `transport`.
- `agent::missing_agent_message` match.

## Forbidden outcomes

- `SessionCommand` or `ProviderInvocation` surviving beside `NativeClient`/`Start`.
- `Launch` or `Admitted` holding an `Option` attachment, or a constructor that
  admits lazily.
- A `match kind { … }` outside `HarnessKind::harness()` and `parse`/`as_str`.
  Branch on `Capabilities` instead.
- `Holder::of` called anywhere but `transport::holder`, or
  `is_interactive_run` anywhere but `delegate`.
- A resolved `Mode` stored on `Cli`, or an `Option<Mode>` with a fallback.
- An entry whose place or holder differs from main.
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

Required admission is implemented: native entry points admit before command
construction; every stream `start` takes `&Admitted`; the runtime retains that
same store and frozen attachment. Native `admit` returns its `Admitted` alongside
any capture it created, so the entry point owns that capture's settlement; a
supplied runtime stays caller-owned. Both optional launch fields, `open_owner` and
the unattached-launch test are deleted. `admit_for_test(home)` returns `Admitted`.

Remaining work:
3a. **Repair slice 3** per "Fourth pass": `holder(work, source)` with native
   resume reading its own store, and `Mode` passed as an argument with
   `Cli::launch_mode` deleted. Focused: the two suites under 3, plus a native
   resume test where the store read fails and the resume errors.
3. **Finish moving holder into `Launch`.** Mode now comes from
   `Delegation::Here { mode }` and reaches `PromptBuild` through `Cli::launch_mode`;
   prompt previews use `LaunchInput::preview_mode` without delegation.
   `transport::holder` is the only `Holder::of` caller. Place still reads named
   Work, independently of reporting ownership; the inherited `LF_AS` row pins
   here + Server. `ProcessConfig::auto` and its unreachable branches are deleted.
   Still remove `AgentConfig::holder` and `Holder: Default` with slice 4 so
   reporting ownership travels on `Launch`, through native commands and drivers.
   Focused: `--lib transport::launch_mode_tests::` and
   `--lib lf::commands::run::launch_policy_tests::`.
4. **`Harness` + `Driver` + per-harness `open`.** Move the table rows above;
   one `harness::launch`; shared `native::run`. `claude.rs`'s private
   `Launch { config, owner }` is the driver-side half of this and goes when
   `Harness::driver` takes `harness::Launch`. (`transcript::Launch`, a parsed
   launch turn, is unrelated and keeps its name.) Focused:
   `--lib harness::conformance_tests` plus a new `harness::launch_matrix` test.
5. **Leftovers.** Replay, raw process recognition, settings rule, docs.
   Claude usage parsing now has one helper for live and saved observations.

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
  (83 non-test comparisons at 470da475a1; 79 at d49e0e16f9).
- `rg -n 'fn run_interactive|ProviderInvocation|SessionCommand\b|session_attachment: Option|holder: crate::session::Holder' rust/loopflow-execution/` prints nothing.
  The unrelated Clap `loopflow::lf::SessionCommand` remains.
- `docs/dev/codebase.md` "Add a provider" says: add `<name>.rs`
  implementing `Harness`, add its `HarnessKind` variant and registry line, and
  add conformance fixtures.

## Checks

Earlier admission and runner checks are recorded at
b9542ecef5:scratch/harness-launch-type.md.

- implement (2026-11-16): `cargo test -p loopflow-execution --lib harness::native_exit::` — 3 passed after the #1955 merge.
- implement (2026-11-16): `cargo nextest run -p loopflow -p loopflow-execution --lib -E 'test(transport::launch_mode_tests::) | test(lf::commands::run::launch_policy_tests::) | test(agent::tests::native_retry_policy_matrix) | test(harness::claude_decode::tests::result_extracts_turn_usage_tokens) | test(harness::claude_decode::tests::assistant_usage_is_live_and_deduplicated_by_message) | test(harness::claude_decode::tests::result_pairs_peak_assistant_input_with_its_model_window) | test(transcript::live::tests::a_finished_claude_turn_reads_its_end_model_and_usage_once_per_message)'` — 15 passed; macOS ld emitted its compact-unwind size warning.
- implement (2026-11-16): `cargo +stable fmt --all -- --check` (Rustup tools first on PATH) and `git diff --check` — passed.
- implement (2026-11-16): `cargo +stable clippy --all-targets -- -D warnings` (Rustup tools first on PATH, inherited `LF_*` unset) — passed with Rust 1.100.0.
- gate (after the remaining design): `uv run python scripts/test.py --reuse-passing --rust-filter 'test(harness::launch_matrix) | test(session_record::capture_tests::) | test(lf::commands::run::launch_policy_tests::) | test(documented_commands::) | binary(lf)'`; CI runs the suites and clippy.
- gate: the two `rg` commands above.
- demo/qa: real-terminal launches above (needs installed providers).

## Docs written (write-docs)

- `rust/loopflow-execution/src/harness/README.md` "One launch type per harness"
- `docs/dev/codebase.md` "Add a provider"

#1953 replaced the README's account paragraphs with "Native clients own
authentication."

Both describe the finished design. `Admitted` and `Mode` now exist; shared
`Launch`, per-harness `open` and `launch_matrix` remain absent. The README
names `transport::holder` after Work binding and distinguishes Caller/Server
from physical terminal placement. The finished interface docs stay with this
unpublished branch until slice 4 is built.
