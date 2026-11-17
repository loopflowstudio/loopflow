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
context-file path. Mode selects the native or headless runner; it is not a field.
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
for each attempt; native entry points admit before opening their provider.

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

## Slice 6 (implemented, bad53639ec)

The launch carries its resolved harness. `Start` alone selects a driver's
resume thread; `AgentConfig::resume_token` remains retry state read at the
launch boundary. Native helpers retain the launch with field updates rather
than reconstructing it. Capabilities include initial prompt on resume,
execution boundary and failure classification. Transcript watches and provider
conversation discovery iterate `HarnessKind::ALL` and ask the owning harness;
Pi's transcript root is `sessions`, OpenCode watches its home for store hints.

The detailed slice direction and earlier check results are at
6b51cfc07c:scratch/harness-launch-type.md.

## Slice 7 (implemented, 2026-11-16)

The five remaining production launch/transcript decisions now belong to the
harness:

- Codex validates a shared AgentProcess server before the common close path.
- Human input recency groups wanted threads by kind and home; Codex reads one
  history file per home, Claude reads each distinct transcript once.
- Skill portability keys tool-name notes on source and target dialect. Terminal
  input receives the resolved harness rather than parsing its name again.
- Saved-turn completion detail uses the harness's transcript policy.
- Raw discovery asks each harness for live process ownership once per sample,
  then resolves named transcripts through that harness's layout.

Existing recency and ownership tests now exercise the owning harness methods;
the history fixture includes two wanted threads. Portability checks cover both
translation directions and a target without native skills.

These stay, with the reason:

- `AgentConfig::resolved_agent` and `agent/session.rs:98` compare `"claude"`
  to apply `claude_model`. `claude_model` and `codex_service_tier` are
  settings named for one harness; folding them into the harness is a settings
  change, not a launch change.
- `loopflow-storage` (`Provider` parsing, `session_record::native_usage_key`)
  sits below execution and reads recorded payloads; it cannot ask a harness.
- `machine/credentials.rs` filters `opencode auth export` entries by
  `integrationID == HarnessKind::OpenCode.as_str()`: that compares OpenCode's
  own JSON field inside OpenCode credential reading, not a harness decision.
- Skill invocation names in warning text remain; they do not select policy.
- A harness file comparing its own name (`codex.rs::resolve_model`, process
  recognition) is the owner answering.

Check: the awk listing below prints only the "stay" lines, and the slice 6
nextest filter plus `test(skills::invocation::)`, `test(transcript::facts::)`
and `test(harness::agent_process::)` pass.

```
for f in $(grep -rlE '"(claude|codex|opencode|pi)"|HarnessKind::(Claude|Codex|OpenCode|Pi)\b' --include='*.rs' rust/loopflow-execution/src | grep -vE 'tests?\.rs|test_support|launch_matrix'); do
  awk -v f="$f" '/#\[cfg\(test\)\]/{exit} /"(claude|codex|opencode|pi)"|HarnessKind::(Claude|Codex|OpenCode|Pi)/ && /==|!=|=>|Some\("|home\(/ {print f":"NR": "$0}' "$f"
done | grep -vE 'harness/(claude|codex|opencode|pi|mod)\.rs'
```

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
- `Launch::mode`, `Launch::harness()`, `Driver::set_agent_session`, driver
  config resume copies, separate boolean capability methods, watch-root table
  and provider-to-kind fallback;
- close-by-provider branch, human-input provider table and shared history parsers,
  skill translation target-name match, Claude completion-detail branch and raw
  Claude ownership reader;
- saved-transcript reader, layout and retention dispatch, and KNOWN_AGENTS.
  The final deletion sweep found these remaining HarnessKind authorities;
  they now ask the same harness for transcript policy and use ALL for discovery.

Do not remove store `Provider`, Clap `SessionCommand`, transcript's parsed
`Launch`, or exit-record `Option<Admitted>`.

## Remaining gate and demo checks

Slices 1–7 are implemented. Local checks cover the 16-cell native/headless ×
Caller/Server matrix, command policy, recorded threads, OpenCode diagnostics,
mode/holder decisions, bare native resume and saved-transcript policy.

Concept review of slice 7 (2026-11-16): the sweep prints only the "stay"
lines; no launch or transcript decision is made by name outside a harness
file. `claude.rs::conversation_owners` now uses `transcript::raw::START_SLACK`
for its pid-reuse tolerance instead of a second literal `60`.

Gate/CI still owns held-terminal exec, `documented_commands`, the session CLI
resolution contract, application launch/resume and attachment completion suites.
Real-terminal launches with installed providers remain the demo. The headless
matrix prints waited rows; it does not claim held-terminal exec coverage.

## Checks

- `cargo nextest run -p loopflow-execution --lib -E 'test(harness::launch_matrix::) | test(harness::conformance_tests::) | test(harness::native_session::tests::bare_codex_resume_does_not_resend_its_first_turn) | test(harness::claude::tests::agent_session_resumes_on_a_fresh_process) | test(harness::pi_tests::pi_turns_resume_the_exact_conversation_and_capture_tools_usage) | test(transcript::watch::tests::) | test(transcript::raw::tests::) | test(ops::human_session::provider_conversation::tests::) | test(skills::invocation::) | test(transcript::facts::) | test(harness::agent_process::)'` — final tree: 62 passed, including all 16 launch cells (2026-11-16). The two slow launch fixtures acquired the updated CLI before running.
- Fixture `cargo build --offline -p loopflow --bin lf --message-format=json` and `target/debug/lf --version` — passed.
- `cargo clippy --all-targets -- -D warnings` — passed.
- `cargo fmt --all -- --check` and `git diff --check` — passed.
- Realign (2026-11-16): `cargo nextest run -p loopflow-execution --lib -E 'test(transcript::raw::tests::) | test(ops::human_session::provider_conversation::tests::) | test(harness::claude::)'` after sharing START_SLACK — 20 passed.
- Slice 7 deletion sweep — only retained named settings, credential JSON and warning-label lines; removed symbols absent.

Unchanged mode/holder decision checks reuse the six passing launch-policy rows
recorded at 012c4d2f4f:scratch/harness-launch-type.md. Gate/CI owns broader
suites. Earlier checks: 6b51cfc07c:scratch/harness-launch-type.md.
