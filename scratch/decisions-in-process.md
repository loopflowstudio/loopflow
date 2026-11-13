# LOO-889: Check decisions in-process, keep a small set of full-Machine tests

The Task brief holds the original measurements (where 25.1 s of
`promotion_selects_session_in_owned_and_shared_checkouts` goes, per-group
estimates, the seams). This plan keeps only what steers the work.
`test-speed.md` is the stacked parent's record (prepared store image); its
"What remains" is not this Task.

## Outcome

A developer changing promotion, launch policy or Task Graph admission runs
one filter and gets the decision table back in about a second, then the
surface's few full-Machine tests (listed in `rust/loopflow/tests/README.md`)
for wiring. A new decision case lands as a table row, not a new fixture.

Demo, per finished surface:

```bash
cargo nextest run -p loopflow --lib -E 'test(adoption::tests::)'   # every promotion/bind decision, <1 s each
cargo nextest run -p loopflow --test integration -E 'test(session_lifecycle_tests::promotion_)'  # the kept few
```

plus the before/after table below with real numbers.

## Docs

Written for this slice (intended behavior, before the code exists):

- `rust/loopflow/tests/README.md` "Decision rows and full-Machine tests":
  the row recipe and the promotion kept list. Later slices add their table
  under "Full-Machine tests".
- `scripts/testing.md` "Implement: one focused test": points at the rows.

## Approach

Each moved decision is checked by calling the function the CLI handler
calls, with the store and checkout the CLI would see, in the library test
build:

- Home: `SessionHome` (`src/ops/human_session/tests.rs`) sets `LF_HOME`/`LF_BIN`
  under `test_ambient::EnvGuard` and opens `open_ephemeral_store`, which copies
  the prepared image. Promote it to a shared `#[cfg(test)]` helper when a second
  module needs it; `task_create` reads its store from `LF_HOME`
  (`ops/planning_sync.rs` `planning_store`), so this is the only way in without
  a test-only parameter.
- Git: `loopflow_test_support::TestRepo` (bare origin included), since
  adoption reads branch, worktree root and default branch from real git.
- Sessions: record them the way a launch does, through
  `session_record::SessionRecorder::begin_at` (as
  `resume_refuses_a_run_and_leaves_its_provider_running` does), not raw
  `INSERT INTO agent_sessions`. This is the answer to "seeded rows drift from
  what a real launch writes": the row comes from the launch's own writer.
  Caller identity (`LF_AGENT_CALLER`) is set through the env guard from
  `session_attachment(..).caller(..)`, the same value the fixture passes today.
- Shape: one table-driven test per decision family. Each row names its case
  and asserts on results (Task id, primary, bound Task, cwd, error text), never
  on calls.

Seams per surface (all in the CLI's call path):

| Surface | Functions the CLI calls |
|---|---|
| Promotion / binding | `engine::creation::create` (CLI and served `task create`; normalizes the directory, checks the receipt, then `ops::task::task_create` → `adoption::resolve_source`/`save`), `ops::human_session::bind`, `preview_binding`, `primary::ensure_task`; `lf::commands::session::prepared(cli, dir)` for `resume --prepare`, fed a Clap-parsed `Cli` so selector wiring stays covered |
| Launch policy | `prompt::process::prepare_process_prompt` → `AgentConfig` → `agent::session::build_session_command`; resume: `resolve_session_config` |
| Task Graph run | `ops::task::explain::explain_task_run`, `checkout_explain::explain_task_checkout`, `planning_explain::explain_task_planning`, `task_graph::CapturedGraph` |
| Machine dispatch | `lf::navigation::machine_invocation`, `ssh::parse_remote_command`, `machine::transport_failure`, `work_route::forward` |

The three explain modules and adoption have no in-process tests today; every
check of them goes through spawned `lf`.

## Kept full-Machine tests

Each kept test exists for one wiring reason: a value crossing Clap → ops, the
owner/PTY/provider argv, a native conversation identity surviving resume, or
receipt JSON. Write the surface's list into `tests/README.md` under a
"Full-Machine tests" heading before deleting any of its tests.

Promotion and binding (from 17 promotion/bind tests):

| Keep | Wiring it proves |
|---|---|
| `promotion_selects_session_in_owned_and_shared_checkouts`, one case (owned, `task create --session` from inside the conversation) | `LF_AGENT_CALLER` reaches creation; `resume --task` continues the same native conversation in the recorded cwd |
| `promotion_sets_directory_immediately_and_resumes_same_native_conversation` | bound directory reaches the next provider launch, not the running child |
| `promotion_background_run_does_not_claim_primary` | a real `-b` run's Session never becomes primary |
| `promotion_explicit_sources_with_run_keep_the_selected_work` | `task create --run` opens through ordinary admission into the adopted checkout |

Everything else in the promotion/bind family becomes rows: recency never
selects, shared-checkout transfer, exact selection, done Tasks, failed
reservation and retry, session-only vs worktree-only, recorded subdirectory,
cross-repository and cross-Task refusals, closed Session refusal, retry
idempotence. Rows whose current test also asserts a launch count keep that
assertion only in the kept tests.

Other surfaces get their list when their slice starts; candidates from the
brief: launch per harness (Claude terminal, Claude stream, Codex native,
OpenCode server, Pi), one Machine dispatch round trip plus one transport
failure, one Task Graph run to an interactive step and one to Auto-Ship,
release lock one killed and one exited case per mutation class, local
planning one case per connection mode.

## Slices

1. **This slice: promotion and binding.** Kept list into `tests/README.md`;
   adoption decision table beside `ops/task/adoption.rs`; delete the moved
   integration tests and the moved cases inside kept ones. Focused test:
   `cargo nextest run -p loopflow --lib -E 'test(adoption::tests::)'`.
2. Session launch contract: config precedence (personal vs repo vs
   `--setting`, chrome unset/on/off, yolo, model fallback, headless schema)
   as rows from repo/personal config → `prepare_process_prompt` →
   `build_session_command`. Keep one launch and one resume per harness.
3. Task Graph run and `global_commands` explain tests: rows against the three
   explain functions and `CapturedGraph`; keep the two Graph runs.
4. Machine commands: parsing and failure-text rows; keep one dispatch and the
   ownership round trip. The three ownership tests that make no `lf` call
   after setup move whole.
5. Release lock matrix: cut to one killed and one exited case per mutation
   class (no in-process move; the cost is real processes).
6. Local planning, `work_open_tests`, `context_launch_tests`,
   `settings_launch_tests`: seams not located yet; locate before cutting.

Transport is LOO-887. Deliberate sleeps in `ops::linear::task_planning_tests`
and the slow `config::migrate`/`store::tests` are outside this pattern.

## Risks and findings

- **In-process promotion is git-bound, not free.** Probe (temporary test,
  removed; this Mac, debug, other builds running): `TestRepo` + linked worktree,
  `SessionHome` store, two Sessions, `task_create --session`, then `bind`.

  | Stage | With bare origin (3 runs) | Origin removed (2 runs) |
  |---|---|---|
  | `TestRepo::new` + worktree | 0.26–0.38 s (0.86 s once under load) | 0.27–0.36 s |
  | ephemeral store | 0.02 s | 0.02 s |
  | planning acquire (`ls-remote`/fetch) | 0.11–0.16 s | 0.04 s |
  | resolve source | 0.04–0.06 s | 0.04 s |
  | `save` → `prepare` (`resolve_upstream_base` fetch, merge-base) | ~0.3 s | 0.25–0.37 s |
  | `promote_task` store write | 0.05 s | 0.05–0.08 s |
  | `sync_after_save` (planning push to origin) | 0.55–0.57 s | 0.02 s |
  | `task_create` total | 1.08–1.24 s | 0.43–0.57 s |
  | `human_session::bind` | 0.04–0.05 s | 0.04–0.05 s |

  The CLI's `task create` measured 0.52–1.07 s, so moving creation in-process
  alone saves almost nothing; the savings are the launches (1.08 s each),
  `MachineServer::start` and the 17 other spawned commands per test.
- Decision: promotion rows use a `TestRepo` with origin removed unless the
  row is about the remote (shared checkout default branch, planning publish).
  Planning publish is a side effect, and `sync_after_save` already treats its
  failure as "pending"; no production change. Each table shares one `TestRepo`
  and gives each row a fresh linked worktree, so a row costs ~0.4 s when it
  adopts a checkout and under 0.1 s when it refuses or binds.
- Group tables by decision family (2–4 rows each) so each nextest test stays
  around 1–2 s; a single 15-row table would be a new 6 s test.
- Not this Task, observed: every `lf task create --session/--worktree` a
  person runs pays a foreground origin fetch in `prepare` and a planning push,
  ~0.7 s of git before returning.
- Library tests that set `LF_HOME` serialize on the env lock; nextest runs
  each test in its own process, so the lock only matters within a test.
- A row only protects if the CLI handler calls the same function with the
  same arguments. For each surface, read the handler (`lf/commands/...`) and
  name the call in the test module's header comment; where the handler does
  its own resolution first (selectors, cwd normalization), that part stays
  covered by a kept test.

## Forbidden

- A test-only flag, parameter or env switch to skip planning sync or git
  fetches. Remove the origin in the fixture instead.
- The same decision checked both as a row and as a full-Machine test. A kept
  test asserts wiring; its decision assertions go to the rows.
- Raw `INSERT INTO agent_sessions` where a launch writer exists.

Six months out, the way this fails: rows pass while `lf task create
--session` breaks between Clap and `engine::creation::create`, because the
kept test for that path was cut "since the rows cover it". Each kept test's
row in the README names the wiring so that cut is visible in review.

## Measurements

Measure each group's "before" on this branch's base before changing it. Per group, nextest, six threads, summed per-test seconds and count under 1 s:

| Group | Before (tests, s, <1 s) | After |
|---|---|---|
| session_lifecycle_tests | 42, ?, ? | |
| session_launch_contract | 30, ?, ? | |

## Done when

- Promotion/binding, launch policy, Task Graph explain and Machine dispatch
  decisions each run as in-process rows under 1 s.
- `tests/README.md` lists each surface's kept full-Machine tests with the
  wiring each proves.
- The table above has before/after for every touched group.

## Checks

- `cargo nextest run -p loopflow --lib -E 'test(adoption::tests::)'` — implement, each slice
- kept integration filter per surface — implement, each slice
- `cargo fmt`, `cargo clippy --all-targets -- -D warnings` — before commit
- full suites — CI
