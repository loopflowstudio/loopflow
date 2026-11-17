# LOO-924: one launch type per harness

## Outcome and constraints

One per-harness `Harness` answers native and headless launch questions. Core
uses capabilities and `HarnessKind::harness()`; adding a harness adds its file,
kind/registry entry and conformance fixtures. Launches retain their existing
mode, terminal placement, holder, hooks, provider thread and exit behavior.

lf stays outside provider conversation bytes. Codex clients reach their private
app-server directly. Recording failures remain gaps. No account routing returns.
LOO-913 (#1953, 4a0691a9a0) and main through 4852348e5c are merged here.

The full earlier design and counterexamples are at
58a4135e40:scratch/harness-launch-type.md. The Task directive snapshot is in
`.lf/prompts/20261116-210624-4d5d755e-928f-47e1-a709-941f4eb980c5-d6amE0-summary.md`.

## Interface

`HarnessKind` owns parse/as_str/ALL and one registry dispatch, `harness()`.
Each file has a unit `Harness` implementation and its persistent stream
`Driver`. The driver protocol is start, input, same-turn input, interrupt,
stop, provider identity and runtime updates. `start()` takes no arguments;
construction clones the required configuration and frozen admission.

`Launch<'a>` borrows `AgentConfig` and required `Admitted`; it carries its
`Harness`, `Holder`, `Start::{New, Resume { thread }}` and optional
context-file path (built today with `Mode` and without the harness; slice 6).
`Admitted` retains the store, Session and exact `SessionAttachment`. Assigned
native thread IDs come from the admission's capture UUID; callers do not name
them. Capabilities include a Caller/Server thread-naming pair, native skill
dialect, same-turn input, turn limits, schemas, respawn behavior and install copy.

### Two runners (implementation choice, 2026-11-16)

Use the original design's two-runner option over the same `Launch` and `Harness`.
`native_session::run` returns `NativeOutcome`: an OS exit status, interruption
and move notice. `drive::run` returns `AgentProcessResult`: a provider turn's
answer and typed failure for retry policy. No synthetic OS status or mode-tagged
result joins them. `run_agent` owns retries/admission and builds a new `Launch`
for each attempt; native entry points admit before selecting their harness.

`NativeClient` holds a prepared `std::process::Command`, optional remote endpoint,
thread, title, reported-thread path and owned companion. The shared runner may
exec a client with no companion into a held terminal. OpenCode's companion owns
its observer thread and shutdown sender; it signals and joins on every exit path.

| harness | native client | Caller names thread | Server names thread | companion |
|---|---|---|---|---|
| Claude | native TUI; prompt on stdin | admission | admission | none |
| Codex | Caller: plain codex; Server: remote app-server client | start hook | app-server | none |
| OpenCode | private server client | server | server | observer; always wait |
| Pi | native TUI + extension | admission | extension | none |

Hook declarations, title flags, model validation, skill dialect and process
recognition are answered by the owning harness. Shared context helpers keep
hook persistence; they do not dispatch by provider name. Claude stream syntax
and usage decoding each have one implementation.

## Next implement direction: slice 6, one representation per launch fact

Concept review of the implemented slices 4–5 (2026-11-16). Launch behavior is
right; four launch facts still have two or three forms. Each item removes a
form and changes no behavior. Do them in this order; 1–3 unlock each other.

1. **Mode is the runner, not a field.** No harness reads `Launch::mode`. Its
   only readers are `drive::run` and `native_session::run`, each refusing the
   other value, and `codex.rs::open_native` / `opencode.rs::open_native`
   hand-write `Mode::Interactive` to build a `Driver`. Mode is still resolved
   once at `transport::delegate` (`Delegation::Here { mode }`) and selects
   `native_session::run` or `drive::run`; the harness learns it by being asked
   for `open` or `driver`. Delete `Launch::mode` and both refusals.
   `harness::Mode` stays for transport and prompt assembly.
2. **Resume is `Start`.** It is also `AgentConfig::resume_token` and
   `Driver::set_agent_session`. Every `Driver::new` copies
   `launch.start.thread()` back into a cloned config; Codex `open_native`
   builds `Start::New` and then calls `set_agent_session(saved)`, its only
   production caller. Drivers read `launch.start`; a native Codex open whose
   store has a thread passes `Start::Resume`; `set_agent_session` leaves the
   `Driver` trait. `resume_token` remains only as `run_agent`'s retry state,
   read where the attempt's `Launch` is built; harness files do not read it.
   Check that a held Codex with a stored thread and a first prompt still sends
   that prompt (`initial_prompt_on_resume`).
3. **Harnesses do not build Launches.** After 1 and 2, `open` passes the
   Launch it received, with a replaced config where needed
   (`Launch { config: &config, ..*launch }`, as `drive::run` does). The two
   literal constructions in `codex.rs` and `opencode.rs` go, and with them
   their dropped `context_file`.
4. **Launch carries its harness.** `Launch::harness()` re-parses the agent
   string on every call and returns `Result`; `drive::run` calls it three
   times, once behind `expect("validated launch")`, and
   `resume_session_with_env` re-parses behind `expect("resolved harness")`.
   Add `harness: &'static dyn Harness`, parsed once where each Launch is built
   (`agent/mod.rs`, `native_session::{launch_session, resume_session_with_env}`);
   delete the method and the expects.
5. **Capabilities are one struct.** `initial_prompt_on_resume`,
   `supports_execution_boundary` and `classifies_failures` are yes/no
   capabilities answered as trait methods beside `Capabilities`. Move them in.
6. **Sweep the last kind tables.** `transcript/watch.rs::watch` lists four
   homes and their subdirectories beside `Harness::transcript_root` (Pi is
   `sessions` there and the trait default is `projects`: find which is read
   before merging). `provider_conversation.rs::find` maps `Provider` back to a
   kind with `_ => Claude`, the inverse of `Harness::provider()`. A fifth
   harness must not need a line in either.

The launch matrix and conformance tests are the check: same 16 cells, same
commands and threads, with `mode` gone from the fixtures.

## Decisions that must remain true

- Mode is not `!batch`: `LaunchInput::mode()` is
  `!batch && (interactive || tty || !has_prompt)`. Place still uses `batch`.
  `Delegation::Here { mode }` carries that decision to prompt assembly.
  Previews call the same resolver without delegating. Nonlaunch commands have
  no invented mode and `Cli` stores none.
- Place and Holder read different facts. Place reads named Work. Holder reads
  the selected source's managed fact. Inherited `LF_AS` outside a Task checkout
  is `Declared`: here + Server. Do not resolve implicit binding ahead of
  delegation; that adds a store read to every bare invocation.
- Native resume by ID reads `store.is_work(named_work, Some(session))` for
  Holder and propagates read failures. Headless `lf -b resume ID --message`
  uses only CLI Work selectors: without selectors it remains here + Caller,
  even for a stored managed Session. Its command passes no stored Session to
  the holder resolver. These are distinct matrix rows.
- Graph steps re-exec lf, resolving their own mode and holder. Replay, commit
  messages, PR text and sync execution select Headless + Caller in process.
- `Provider` in storage means provider homes/credentials, not a harness; its
  parsing remains. The application Clap `SessionCommand` is unrelated and stays.
- A missing native store is a recording gap in exit settlement; that owner's
  `Option<Admitted>` is unrelated to a required launch admission.
- Pi's native Server thread comes from its extension; Caller gets an assigned
  UUID. A bare Codex reports its provider thread through the start hook.

The unresolved meaning of inherited LF_AS is outside this Task; see
`scratch/questions.md`. It must not change as part of this refactor.

## Delete — do not maintain

The surviving paths replace these; their coverage belongs on Harness/Launch:

- duplicate interactive runner and its runtime launch/exit methods;
- provider `SessionCommand`, `ProviderInvocation`, `build_session_command`,
  `native_connection_command` and program dispatch in native_session;
- `AgentConfig::holder`, Holder's Default and optional launch attachments;
- private Claude Launch, optional driver owners and late `start(config, owner)`;
- `default_create_harness`, `canonical_harness`, terminal-error string table;
- context native_args table, title-argument match, skill native-name tables,
  raw process-name table, settings OpenCode model rule and install-hint match;
- saved-transcript reader, layout and retention dispatch, and KNOWN_AGENTS.
  The final deletion sweep found these remaining HarnessKind authorities;
  they now ask the same harness for transcript policy and use ALL for discovery.

Do not remove store `Provider`, Clap `SessionCommand`, transcript's parsed
`Launch`, or exit-record `Option<Admitted>`.

## Remaining gate and demo checks

Slices 4–5 are implemented; slice 6 above is not. Local checks cover the 16-cell native/headless ×
Caller/Server matrix, command policy, recorded threads, OpenCode diagnostics,
mode/holder decisions, bare native resume and saved-transcript policy.

Gate/CI still owns held-terminal exec, `documented_commands`, the session CLI
resolution contract, application launch/resume and attachment completion suites.
Real-terminal launches with installed providers remain the demo. The headless
matrix prints waited rows; it does not claim held-terminal exec coverage.

## Checks

- `cargo test -p loopflow-execution --lib harness::conformance_tests` — 10 passed.
- Combined nextest decision filter (`test(harness::launch_matrix::) | test(lf::commands::run::launch_policy_tests::) | test(transport::launch_mode_tests::) | test(harness::native_session::tests::bare_codex_resume_does_not_resend_its_first_turn)`) — all 12 settings/mode/holder rows and bare resume passed; the stand-in's missing bare Codex hook report was repaired and the final matrix passed below.
- Combined nextest transcript filter (`test(harness::launch_matrix::) | test(transcript::tests::) | test(transcript::raw::tests::a_pi_process_takes_the_newest_session_file_of_its_directory) | test(transcript::summary::tests::)`) — 10 transcript tests passed; history rendering's `a_turn_prints_one_line_then_its_requests_and_what_the_agent_did` reported LEAK. Gate owns that observation. Pi's provider-only sandbox rejection was retained; the matrix now uses Pi's existing configured policy.
- `cargo nextest run -p loopflow -p loopflow-execution --lib -E 'test(harness::launch_matrix::) | test(harness::native_session::tests::bare_codex_resume_does_not_resend_its_first_turn) | test(transcript::raw::tests::a_pi_process_takes_the_newest_session_file_of_its_directory)'` — 4 passed, including every one of the 16 launch cells, native policy refusals and the OpenCode diagnostic-log assertion (2026-11-16).
- `cargo clippy --all-targets -- -D warnings` — passed on the finished Rust changes; `cargo fmt` and `git diff --check` passed.
- `cargo build -p loopflow --bin lf` — passed on the finished source; `target/debug/lf --version` ran successfully.
- Obsolete-symbol check — no provider SessionCommand, ProviderInvocation, default_create_harness, canonical_harness or run_interactive; Holder::of has one caller, transport::holder. Runtime and exit-record optional owners remain outside launch admission.

Earlier admission/runner checks: b9542ecef5:scratch/harness-launch-type.md.
Slice 3b checks: 58a4135e40:scratch/harness-launch-type.md.
