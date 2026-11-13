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
```

plus the before/after table below with real numbers.

## Docs

- `rust/loopflow/tests/README.md` "Decision rows and full-Machine tests":
  the row recipe, and under "Full-Machine tests" the promotion and launch
  kept lists. Later slices add their table there.
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
| Machine dispatch | `lf::navigation::machine_invocation`, `ssh::parse_remote_command`, `machine::transport_failure`, `work_route::forward` |

The three explain modules have no in-process tests today; every check of
them goes through spawned `lf`.

## Kept full-Machine tests

Each kept test exists for one wiring reason: a value crossing Clap → ops, the
owner/PTY/provider argv, a native conversation identity surviving resume, or
receipt JSON. Write the surface's list into `tests/README.md` under a
"Full-Machine tests" heading before deleting any of its tests.

Promotion (4 of 17 kept) and launch/resume (one launch and one resume per
harness) lists are in `tests/README.md`; the plan doesn't repeat them.

Other surfaces get their list when their slice starts; candidates from the
brief: one Machine dispatch round trip plus one transport
failure, one Task Graph run to an interactive step and one to Auto-Ship,
release lock one killed and one exited case per mutation class, local
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

## Done when

- Promotion/binding, launch policy, Task Graph explain and Machine dispatch
  decisions each run as in-process rows. Done: promotion/binding, launch
  policy. Remaining: Task Graph explain (slice 3), Machine dispatch (slice 4). The brief asks "well under 1 s"
  per decision; promotion rows cost ~0.4 s each but their family tests take
  1.3–3.3 s (open, `scratch/questions.md`).
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
