# LOO-1098: wake Antigravity on reopen and enforce harness parity through the interface and tests

History and 5 Whys: [5whys.md](5whys.md). Concept review: [concept-review.html](concept-review.html).

## Direction & findings

### Live Antigravity probes (2026-11-24)

- **Held-terminal paste-mode wake stopped (moved to LOO-1099):** A disposable
  PTY running `agy 1.5.16` emitted `ESC[?2004h` at the workspace trust dialog.
  With a trusted workspace, the slash menu, the model picker opened with Enter,
  and returning to the prompt produced no paste-mode transition. A restarted
  server can also adopt a terminal holding an unsubmitted draft with no
  keystroke history. Live idle wake for an already-open `agy` belongs to
  LOO-1099; any replacement there must pass those live checks and preserve
  drafts across server replacement.
- **Reopen with waiting steers works live (built & QA'd at `c0b28b7dc6`):**
  `agy --conversation <id> -i <prompt>` records a second `USER_INPUT` on the
  original transcript, runs `PreInvocation` before anyone types, and reports
  `Stop` at turn end. Verified on real `agy 1.5.16` (private Machine): quit the
  Task conversation, save a repository steer, run `lf resume --task <id>` →
  original transcript recorded the turn and exact answer `VIOLET`, `lf session
  steers` showed one receipt (`delivered`) and zero pending, and reopening with
  nothing waiting passed no `-i` and started no turn.

### Post-build concept review & parity direction (2026-11-24)

Jack Heart's direction on the parity contract built in pass 1:
- `"i would rather make it harder to add a harnes if it makes it harder to miss imortant behavior. when i add a harness, i want it to have all the good stuff"`
- `"1. Just remove the table. Instead, just make sure that all behaviors are asserted to work and testeed  across all harnesses."`
- `"and that the Harness interface demands full functionality"`

Pass 1 introduced `harness/parity.rs` (`Behavior`, `Support::{Yes, Gap { why, owner }}`,
`Behavior::assert_observed`) and a generated table in `harness/README.md`. That
model is wrong: `Support::Gap` turns missing behavior into a passing test and
gives new harnesses an opt-out table. Meanwhile, idle-wake reach dispatch still
lives in SQL (`s.provider='codex'`), `CALLER_SESSION_ENV` still lives outside
`Harness`, and `ResumeInput` conflates Codex's empty-thread bootstrap replay
with reopen steer delivery.

## Model to build

### 1. Delete the parity table and `Support::Gap`

- Delete `rust/loopflow-execution/src/harness/parity.rs` and `pub mod parity;`
  from `rust/loopflow-execution/src/harness/mod.rs`.
- Delete the `## Harness parity` section, `<!-- parity:start/end -->` table,
  and `regenerate_readme_table` instructions from
  `rust/loopflow-execution/src/harness/README.md`. Keep the prose explaining
  how hooks, usage, unanswered questions, interrupts, idle wake, and failure
  classification work across providers.
- Update `docs/dev/codebase.md` ("Adding a harness") to point at the required
  `Harness` / `Launcher` interface and the `HarnessKind::ALL` test suites
  instead of `harness::parity`.

### 2. Demand full functionality on `Harness` and `Launcher`

Make core per-harness mechanisms required trait methods or non-optional
`Capabilities` fields (no silent defaults, no reusable `Unsupported` / `Gap`
opt-out):

1. **`loopflow_harness::Harness`** (`rust/loopflow-harness/src/lib.rs`):
   - Add required `fn caller_session_env(&self) -> &'static [&'static str];` (no
     default). Move Claude, Codex, OpenCode, and Antigravity's parent-session
     markers out of `agent/mod.rs`'s monolithic `CALLER_SESSION_ENV` onto each
     provider's `Harness` impl (`Pi` declares `&[]` explicitly). `agent/mod.rs`
     keeps only Loopflow's `LF_*` (and shared `"AI_AGENT"` if not on Claude)
     and `remove_caller_session_env` strips `LF_*` plus every
     `HarnessKind::ALL`'s `caller_session_env()`. Update `launch_matrix.rs` to
     collect provider markers from the same helper.
   - Remove the default body on `fn recognizes_driver(&self, command: &str) -> bool`
     (4 of 5 harnesses already override it; add Claude's 1-line impl).
   - Remove the default body on `fn sign_in_advice(&self) -> &'static str` so
     every harness names its own sign-in command (Claude/Codex still prefer
     `machine::login::sign_in_advice` in `agent/mod.rs` when applicable).

2. **`loopflow_execution::harness::Launcher` and `Capabilities`**
   (`rust/loopflow-execution/src/harness/mod.rs`):
   - Add required `fn idle_reach(&self, endpoints: &IdleEndpoints) -> Option<IdleReach>;`
     on `Launcher` (no default).
     - `Claude` maps `endpoints.inbox` → `IdleReach::Inbox(Inbox::Claude(..))`.
     - `Codex` maps `endpoints.endpoint` → `IdleReach::Codex(..)`.
     - `OpenCode` and `Pi` map `endpoints.socket` → `IdleReach::Inbox(Inbox::Socket(..))`.
     - `Antigravity` returns `None` with `// TODO(LOO-1099)` until live wake
       ships.
   - Remove provider-name guessing from SQL (`rust/loopflow-storage/src/sqlite/steers.rs`):
     - `idle_conversation_candidates` drops `CASE WHEN s.provider='codex' ...`
       and the column-presence `OR` filter, keeping
       `INDEXED BY agent_sessions_unended_agent` and returning raw candidate
       fields (`provider`, `endpoints: IdleEndpoints`, `adapter`, `thread`,
       `cwd`, `identity`).
     - `loopflow_execution::steer::idle_conversations` resolves `IdleReach`
       through `HarnessKind::parse(&provider).and_then(|k| harness::launcher(k).idle_reach(&endpoints))`
       after checking live process identity.
     - Add `loopflow_execution::steer::is_idle_reachable(store, adapter)` (or
       reuse the same reach resolver without requiring process-table checks
       where `context.rs::read_news` runs inside a hook) so `context.rs` and
       `wake.rs` share one owner of idle reach, and update `boundary.rs` tests
       to assert through the shared resolver.
   - Split `Capabilities.resume_input` (`ResumeInput`) back into two clear,
     required fields:
     - `pub initial_prompt_on_resume: bool` — Codex-only empty-thread replay
       (`agent/session.rs`, `ops/human_session.rs`, `native_session.rs`).
     - `pub reopen_steers: ReopenSteers` — required enum with only working
       mechanisms (`ReopenSteers::ServerBoundary` for Claude, Codex, OpenCode,
       Pi; `ReopenSteers::PromptHook` for Antigravity). No `None`/`Unsupported`
       variant.
     - In `native_session.rs::resume_session_with_env`: keep `config.task_prompt`
       when `initial_prompt_on_resume` on `Worker | WorkSession`; call
       `prompt_for_waiting_steers` when `WorkSession` and
       `reopen_steers == ReopenSteers::PromptHook` with `conversation_context`;
       otherwise `take_steers_to_start` for `Chat | WorkSession`.

### 3. Assert every behavior unconditionally across `HarnessKind::ALL`

Replace every `Behavior::*::assert_observed(...)` call with direct,
unconditional assertions over `HarnessKind::ALL`:

- **Unconditional across all 5 harnesses (and any future harness):**
  - `task_graph_launch_tests::a_task_step_runs_as_a_worker_on_every_harness`
    (worker settles/answer/thread, step context, mid-turn steer, usage).
  - `harness::launch_matrix::every_harness_runs_all_four_launch_kinds`
    (headless permission bypass, yolo, caller-env stripping, hook ownership,
    thread recording).
  - `task_graph_launch_tests::a_step_that_cannot_launch_is_not_started_again`
    (missing CLI error + install hint + fallback). Also fix the pre-check in
    this test to inspect only system tool dirs (`/opt/homebrew/*`, `/usr/local/*`)
    matching `scripts/test_network.py`, so a host with `~/.local/bin/claude`
    does not false-panic inside `LOOPFLOW_NETWORK_ISOLATED=1`.
  - `task_graph_launch_tests::a_worker_stops_on_interrupt_and_the_next_run_starts_its_step_again`
    (interrupt + re-run).
  - `harness::conformance_tests::each_harness_classifies_its_own_refusals`
    (quota, revoked login, unserved model).
  - `harness::conformance_tests::every_harness_reads_a_synced_skill`
    (global + repo skills and Graphs).
  - `server::wake::tests::reopened_work_sessions_take_waiting_steers` +
    `session_launch_contract::a_reopened_antigravity_task_hears_repository_direction_once`
    (dispatch exhaustively on `HarnessKind` / `ReopenSteers` with no wildcard,
    asserting delivery unconditionally).
- **Narrow, shrinking inline exceptions (explicit legacy arms, NO wildcards):**
  - `server::wake::tests::every_harness_wakes_its_idle_work_session`: assert
    `delivered` and message content unconditionally for every harness; only
    `HarnessKind::Antigravity` sits in an explicit `// TODO(LOO-1099)` arm. A
    6th harness fails to compile/pass until it wakes idle work sessions.
  - `task_graph_launch_tests::a_question_with_nobody_there_ends_the_step`:
    assert `unanswered_question` failure + stop after 1 attempt unconditionally
    for `OpenCode | Pi` and any future harness (exhaustive match, no wildcard);
    only `Claude | Codex | Antigravity` sit in an explicit
    `// TODO(Task 48edb9)` arm.
- `ViewerHandoff` (LOO-1097) stays tracked in Linear and `.lf/qa/antigravity.md`,
  not in a Rust table.

## Execution order

1. **Interface & reach ownership first (proves the model holds soonest):**
   - Tighten `Harness` (`caller_session_env`, required `recognizes_driver`,
     required `sign_in_advice`) and move provider env vars out of `agent/mod.rs`.
   - Add `Launcher::idle_reach`, split `ResumeInput` into
     `initial_prompt_on_resume` + `ReopenSteers`, move reach resolution out of
     `sqlite/steers.rs` into `loopflow_execution::steer`, and update `wake.rs`,
     `context.rs`, `boundary.rs`, and `query_plans.rs`.
2. **Remove `parity.rs` and the README parity table, convert all tests:**
   - Delete `harness/parity.rs`, remove `## Harness parity` table from
     `harness/README.md`, update `docs/dev/codebase.md` and `scratch/tour.yaml`.
   - Convert all `Behavior::assert_observed` sites to unconditional assertions
     across `HarnessKind::ALL` with only the two narrow inline `TODO` arms, and
     fix the system-tool-dir check in `a_step_that_cannot_launch_is_not_started_again`.
3. **Verify:** Run the acceptance suite, `cargo fmt`, and
   `cargo clippy --all-targets -- -D warnings`.

## Acceptance at gate

```sh
cargo test -p loopflow-storage --lib query_plans::
cargo test -p loopflow-execution --lib harness::conformance_tests::
cargo test -p loopflow-execution --lib harness::launch_matrix::every_harness_runs_all_four_launch_kinds
cargo test -p loopflow-execution --lib harness::environment_tests::
cargo test -p loopflow --lib server::wake::tests::
cargo test -p loopflow --lib server::boundary::tests::
cargo test -p loopflow --lib task_graph_launch_tests::
cargo test -p loopflow --test session_launch_contract a_reopened_antigravity_task_hears_repository_direction_once
```

Already-open Antigravity idle wake belongs to LOO-1099. Viewer handoff belongs
to LOO-1097. Headless unanswered questions on Claude/Codex/Antigravity belong
to Task 48edb9.
