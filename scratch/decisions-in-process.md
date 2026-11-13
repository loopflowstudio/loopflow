# LOO-889: Check decisions in-process, keep a small set of full-Machine tests

The Task brief holds the original measurements (where 25.1 s of
`promotion_selects_session_in_owned_and_shared_checkouts` goes, per-group
estimates, the seams). This plan keeps only what steers the work.
`test-speed.md` is the stacked parent's record (prepared store image); its
"What remains" is not this Task.

## Outcome

A developer changing promotion, launch policy or Task Graph admission runs
one filter and gets the decision table back in a few seconds, then the
surface's few full-Machine tests (listed in `rust/loopflow/tests/README.md`)
for wiring. A new decision case lands as a table row, not a new fixture.

Demo, per finished surface:

```bash
cargo nextest run -p loopflow --lib -E 'test(adoption::tests::)'   # every promotion/bind decision, 1–3 s per family
cargo nextest run -p loopflow --test session_lifecycle_tests -E 'test(promotion_)'  # the kept few (standalone binary)
cargo nextest run -p loopflow --lib -E 'test(run::launch_policy_tests::)'  # launch/resume settings, 0.6–1.9 s each
cargo nextest run -p loopflow --lib -E 'test(ops::task::explain::tests::)'  # task run/checkout previews, 0.02–2.5 s each (load-dependent)
cargo nextest run -p loopflow --lib -E 'test(ssh::dispatch_tests::) | test(task_ownership::tests::)'  # Machine dispatch and ownership, 15 in 2.5 s
```

plus the before/after table below with real numbers.

## Docs

- `rust/loopflow/tests/README.md` "Decision rows and full-Machine tests":
  the row recipe, and under "Full-Machine tests" the promotion, launch and
  Task Graph run kept lists. Later slices add their table there.
- `scripts/testing.md` "Implement: one focused test": points at the rows.

## Approach

The row recipe (home, `TestRepo`, `SessionRecorder::begin_at`, result
assertions) is in `rust/loopflow/tests/README.md`; `SessionHome`
(`ops/human_session/tests.rs`) is the shared home, since `task_create` reads
its store from `LF_HOME` (`ops/planning_sync.rs` `planning_store`).

Seams per surface (all in the CLI's call path):

| Surface | Functions the CLI calls |
|---|---|
| Promotion / binding | `engine::creation::create` (CLI and served `task create`; normalizes the directory, checks the receipt, then `ops::task::task_create` → `adoption::resolve_source`/`save`), `ops::human_session::bind`, `preview_binding`, `primary::ensure`; `lf::commands::session::prepared(cli, dir)` for `resume --prepare`, fed a Clap-parsed `Cli` so selector wiring stays covered |
| Launch policy | `run::build_prompt` (Clap-parsed `Cli` → config at the checkout → `prepare_process_prompt` → `AgentConfig`) → `build_session_command`; resume: `select_invocation` + `resolve_session_config(session, &Cli::default())` |
| Task Graph run | `ops::task::explain::explain_task_run`, `checkout_explain::explain_task_checkout`, `planning_explain::explain_task_planning`, `task_graph::CapturedGraph` |
| Machine dispatch | `lf::navigation::machine_invocation`, `ssh::parse_remote_command`, `machine::transport_failure` (`work_route::forward` carries Task moves, covered by the kept `machine_commands` tests) |

`planning_explain` still has no in-process tests: its six `global_commands`
tests (`task_planning_explain_*`, 49 s warm) are the next row candidate. It is
not a Graph decision, so not a Done-when obligation.

## Kept full-Machine tests

Each kept test exists for one wiring reason: a value crossing Clap → ops, the
owner/PTY/provider argv, a native conversation identity surviving resume, or
receipt JSON. Write the surface's list into `tests/README.md` under a
"Full-Machine tests" heading before deleting any of its tests.

Promotion, launch/resume, Task Graph run and Machine dispatch lists are in
`tests/README.md`; the plan doesn't repeat them.

Other surfaces get their list when their slice starts; candidates from the
brief: release lock one killed and one exited case per mutation class, local
planning one case per connection mode.

## Slices

1. **Done: promotion and binding.** Kept list into `tests/README.md`;
   adoption decision table beside `ops/task/adoption.rs`; delete the moved
   integration tests and the moved cases inside kept ones. Focused test:
   `cargo nextest run -p loopflow --lib -E 'test(adoption::tests::)'`.
2. **Done: session launch contract.** Rows in
   `lf/commands/run/launch_policy_tests.rs` (`run::build_prompt` with a
   Clap-parsed `Cli` → `build_session_command`; resume: `select_invocation`
   + `resolve_session_config(session, &Cli::default())`, as `bin/lf.rs` and
   `resume_session_with_env` do). Deleted `resume_fallback_…`,
   `personal_true_overrides_…`, `unset_chrome_leaves_…`; trimmed the
   override resume from `terminal_launch_and_resume_…` and the schema/chrome
   checks from `headless_launch_and_graph_…`. Kept list in `tests/README.md`.
   The brief estimated 12 of 30 decision-only; 3 moved whole. Not yet read
   for row candidates: `contextual_resume_uses_scope_and_recovers_…`,
   `wave_targets_prepare_reuse_and_open_…` (resume target selection);
   `machine_dispatch_resolves_destination_policy_…` belongs to slice 4.
3. **Done: Task Graph run and its previews.** Rows in
   `ops/task/explain/tests.rs` against `explain_task_run` (→ `select_task_run`
   → `CapturedGraph`) and `explain_task_checkout`, with the
   `lf::commands::context` wrappers for an absent or unreadable registry. The
   Clap → `TaskProcessOptions` mapping is in `bin/lf.rs`, outside the library,
   so it stays with the kept explain tests. Deleted
   `task_run_from_explain_…`, `task_run_explain_preserves_absent_…`,
   `task_checkout_explain_reads_restoration_…`,
   `task_checkout_explain_proposes_unallocated_…`,
   `a_run_explanation_includes_…_xor`; trimmed three kept explain tests to
   wiring and renamed the Graph run test
   `a_task_takes_up_its_waves_graph_and_keeps_the_one_it_named`. Kept list
   in `tests/README.md`.
4. **Done: Machine dispatch.** Rows in `lf/commands/ssh/dispatch_tests.rs`
   (`machine_invocation` → `parse_remote_command`; `transport_failure`) and
   three ownership tests moved whole into `store/sqlite/task_ownership.rs`
   (two `open_ephemeral` stores, two `TestRepo`s, peer planning exchange).
   Deleted `machine_selector_dispatches_…`, `transport_options_require_…`
   and the three `task_ownership_*` tests; trimmed
   `failures_name_the_recovery_…` to one diagnostic and the literal-prompt
   dispatch from `machine_dispatch_resolves_destination_policy_…`. `lf ssh
   mini status` parses as a skill named `ssh` (refused only at skill
   lookup), so it left the rows rather than becoming a parse row. Kept list
   in `tests/README.md`. `work_route::forward` got no rows: its cases
   (moved Sessions and Tasks) are the Task-move tests, not parsing.
Slices 5 and 6 go beyond Done-when; take them only if this Task continues.

5. Release lock matrix: cut to one killed and one exited case per mutation
   class (no in-process move; the cost is real processes).
6. Local planning, `work_open_tests`, `context_launch_tests`,
   `settings_launch_tests`: seams not located yet; locate before cutting.

Transport is LOO-887. Deliberate sleeps in `ops::linear::task_planning_tests`
and the slow `config::migrate`/`store::tests` are outside this pattern.

## Row speed (concept review, 2026-11-13)

Row speed reads as met: the brief's "Each decision is checked once,
in-process, in well under 1 s" is per decision, and a row costs ~0.4 s
adopting a checkout, under 0.1 s refusing or binding (probe above); the
1.3–3.3 s is a family test holding three to six rows. One nextest test per
row (option b) adds fixture setup per row for no decision coverage. Cutting
`prepare`'s foreground fetch (option c) changes what people wait for on
`lf task create --session`, so it is its own Task, not this one.

## Promotion slice notes

- Rows cover what the ten deleted tests checked except: plain `--run`
  creation opening a fresh checkout (Task Graph admission, slice 3), `lf open
  --link`/`task status` output after binding, and `task checkout` after a
  shared-primary transfer. None is a promotion decision.
- A closed Session row records through `SessionRecorder::begin_at`, then sets
  `completed_at` with one `UPDATE`: closing follows the agent process's exit,
  and a row has no process.
- `promotion_background_run_does_not_claim_primary` asserted `returned["id"]`
  on a receipt that nests the Task under `task`; both asserts passed on null.
  Fixed. It still inserts its Session row directly.

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

The README's rules hold: no test-only switch to skip sync or fetches, no
decision checked both as a row and a full-Machine test, no raw
`INSERT INTO agent_sessions` where a launch writer exists.

Six months out, the way this fails: rows pass while `lf task create
--session` breaks between Clap and `engine::creation::create`, because the
kept test for that path was cut "since the rows cover it". Each kept test's
row in the README names the wiring so that cut is visible in review.

## Measurements

Measure each group's "before" on this branch's base before changing it. Per group, nextest, six threads, summed per-test seconds and count under 1 s:

| Group | Before (tests, s, <1 s) | After |
|---|---|---|
| Promotion/bind, `session_lifecycle_tests` | 14, 194.5, 0 | 4, 43.2, 0 |
| Promotion/bind rows, `adoption::tests` | — | 8, 17.9, 0 (1.3–3.3 s each) |
| session_launch_contract | 30, 254.3, 0 | 27, 244.7, 0 |
| Launch/resume rows, `run::launch_policy_tests` | — | 5, 6.0, 1 (0.6–1.5 s each) |
| Explain and Graph run, `global_commands` + `task_graph_launch_tests` | 15, 190.3, 0 | 10, 94.6, 0 |
| Explain rows, `ops::task::explain::tests` | — | 7, 7.0, 2 (0.02–1.4 s each) |
| machine_commands + destination-policy test | 49, 1769.1, 0 | 44, 1717.5, 0 |
| Dispatch and ownership rows | — | 3 + 3 new, 7.2, 3 (dispatch 0.02–0.1 s, ownership 1.3–2.4 s) |

Promotion, 2026-11-13, Mac debug, load average 545–600 from other
checkouts: before and after ran back to back. An earlier baseline at lower
load summed 163.8 s for the same 14. Promotion total 194.5 → 61.1 s. Rows
miss the <1 s target: each adopting row pays `TestRepo` worktree + `prepare`
git (~0.4 s unloaded, per the probe), and families hold three to six.

Launch, 2026-11-13, load average 415–470, back to back: 254.3 → 250.7 s
with rows. Small, because the moved tests were the cheap ones (2.4–6.6 s);
`task_conversation_preparation_is_passive_…` alone is 70–71 s, over a minute
of it unexplained by its five launches (a wait, not a decision; not
investigated). The `--output-schema` refusal on `session resume` is a check
in `lf/commands/session.rs` `resume`, before `resolve_session_config`, so it
stays in the previews test.

Explain, 2026-11-13: before at load 484–638, after at 712–724, both warm
(the first run after a build spent 70–86 s per test on a cold `lf`). Of the
10 after, the six untouched `task_planning_explain_*` tests sum 49.0 s
(59.6 s before); the Graph/run/checkout part went 9 tests, 130.7 s → 4,
45.7 s.

Machine dispatch, 2026-11-13, load average 585–675: totals are noise.
Each run's first wave of six paid a cold `lf` (~150 s each); without them
the warm tests summed 889.0 s (43) before and 875.2 s (37) after. The moved
tests themselves cost 55.4 s before (`machine_selector_…` 25.1, three
ownership 7.2 each, `transport_options_…` 8.7); `failures_name_…` went
32.6 → 25.5 s and the destination-policy test 16.1 → 7.3 s. The before run
had two failures from a build-version skew (`.dirty` from editing during the
build), not product failures.

## Done when

- Promotion/binding, launch policy, Task Graph explain and Machine dispatch
  decisions each run as in-process rows. Done: all four, each row under
  1 s (~0.4 s adopting, under 0.1 s refusing); family tests 1.3–3.3 s.
- `tests/README.md` lists each surface's kept full-Machine tests with the
  wiring each proves.
- The table above has before/after for every touched group.

## Checks

- `cargo nextest run -p loopflow --lib -E 'test(adoption::tests::)'` — implement, each slice
- kept integration filter per surface — implement, each slice
- `cargo fmt`, `cargo clippy --all-targets -- -D warnings` — before commit
- full suites — CI
- compress, 2026-11-13: `adoption::tests::` 8/8 passed (1.3–3.3 s each); clippy `--all-targets` clean
- implement slice 2, 2026-11-13: `run::launch_policy_tests::` 5/5; `session_launch_contract` 27/27; `documented_commands::` ok; clippy clean
- compress slice 2, 2026-11-13: `run::launch_policy_tests::` 5/5 (0.8–1.9 s each)
- implement slice 3, 2026-11-13: `ops::task::explain::tests::` 7/7; kept explain + Graph run filter 10/10; clippy `--all-targets` clean
- compress slice 3, 2026-11-13: restore rows read as sequential stanzas; `ops::task::explain::tests::` 7/7 (0.05–2.5 s each)
- implement slice 4, 2026-11-13: `ssh::dispatch_tests:: | task_ownership::tests::` 15/15; `machine_commands` + destination-policy 44/44; clippy `--all-targets` clean
- compress slice 4, 2026-11-13: ownership `Pair` holds one `Side` (store, checkout, path) per Machine; `ssh::dispatch_tests:: | task_ownership::tests::` 15/15 in 2.3 s; clippy `--all-targets` clean
- implement rename, 2026-11-13: `Machine` → `Table` row fixtures; three row filters 20/20 in 4.3 s; fmt, clippy `--all-targets` clean
- compress rename, 2026-11-13: nothing to reduce; rename is name-only (`Machine` stays in dispatch and ownership rows, where it is the domain noun)
