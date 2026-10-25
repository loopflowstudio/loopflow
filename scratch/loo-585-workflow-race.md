# LOO-585: local_planning tests raced their detached Graph

Symptom: CI/merge-group panics at local_planning.rs:3823 (`waveless_task_lifecycle`) and :3919
(`workflow_defaults_follow_task_wave_and_repository`) comparing Workflow JSON whose `history` differed.

Cause: #1675 made bare `lf task run` return once the Graph registers; the arrival is written later
by the detached owner. These tests (and `two_people`'s takeover run) still used bare `task run`
and then read settled state. `-b` still waits for the Graph and its arrival.

Repair: tests that read state after a run now use `lf -b task run`. Tests that exercise detaching
(task_graph_launch_tests) or refusals and no-Graph edges are unchanged.

Verification on the rebased branch (12-step Graph probe to widen the window, CPU saturated with `yes`
per core, 5 rounds × 2 tests): bare `task run` 10/10 failed at the CI lines (:3823, :3927); `-b` 0/10.
Unmodified tests never failed locally without the probe (0/20). Keep the probe at 12 steps: at 40 both
reads land mid-edge and agree, hiding the race. These tests compile into the lib target (`src/lib.rs`
`#[path]`): run `cargo test -p loopflow --lib local_planning::<name>`; a `--test integration` filter
matches nothing. `cargo test --test two_people` ok; clippy ok. One unrelated lib failure
(`store::sqlite::children::…task_labels_and_generated_names…`, "capture does not belong to a recorded
Session in this Machine") is deterministic on this machine, green on main CI: left to CI.

Gate (2026-10-25, rebased on origin/main): `cargo fmt --check` ok; `cargo clippy -p loopflow --all-targets -D warnings` ok;
`--lib local_planning::waveless_task_lifecycle` ok; `--lib local_planning::workflow_defaults_follow` ok; `--test two_people` ok.

## Why it happened (5 whys)

**Affected.** PR #1679 (LOO-583) went red in CI and spent a ci-fix round; PR #1678 (LOO-579) was
evicted from the merge queue (`failed_checks`, run 40173940518) and re-armed by hand. Both have
since merged.

1. **Why did the assertions see a different `history`?** The tests read `task workflow show` while the
   edge's Graph had not yet written its arrival. Evidence: the CI diffs differ only in `history`;
   #1679's ci-fix diagnosed the same; widening the window (12-step probe) reproduces 10/10 at the CI
   lines, and `-b` drops it to 0/10.
2. **Why was the arrival not written when `task run` returned?** #1675 (LOO-575, merged 06:12 UTC, about
   an hour before the failures) moved bare `lf task run`'s return point from "Graph finished and
   arrival written" to "Graph registered" whenever an execution owner exists (`graph.rs`,
   `letting_go = !cli.batch && !cli.interactive && transport::current().is_some()`). The test binary
   runs under an owner, so every bare call in a test now detaches.
3. **Why did #1675 leave those callers on the old meaning?** Its caller inventory covered prose and
   UI callers: docs, builtin skills and the Swift launcher. Test callers weren't in it, and the change
   gave them nothing to notice. The argv stayed the same, the exit code stayed 0, and the tests still
   passed, because the Graph in these fixtures finishes in milliseconds. #1675's own PR CI passed
   every rust-test shard, local_planning included (local runs left it to CI).
4. **Why did CI only fail later, and on unrelated PRs?** The window is a single fast Graph step against
   the next `lf` process spawn. It loses only under contention, like merge-group runners. Unloaded and
   loaded local runs of the unmodified tests went 0/20. So any PR's run could lose it, not just
   #1675's. Merge-group checks run without retries, so one loss evicts the entry (the
   eviction handling itself belongs to LOO-583/LOO-584).
5. **Why did the first repair not stop the second failure?** ci-fix repairs the failing assertion. On
   #1679 it added a 30s poll loop around one `workflow show` in `waveless_task_lifecycle` and left
   the other bare runs in that file racing. #1678's merge group then failed on the next one (:3919).
   That makes three ways for a test to wait on a Graph: `-b`, a hand poll, and nothing.

Root conditions: (a) a command's completion point changed while its spelling stayed the same, and
the only callers that could notice were timing-sensitive tests; (b) incident repair is scoped to
the failing line, not to the class it belongs to.

### Branches and contradicting evidence

- No CI log shows the race directly. Causation comes from the line numbers, the `history`-only diff,
  #1679's diagnosis and the probe reproduction. Unmodified tests never failed locally (0/20).
- `two_people`'s takeover race is inferred from code; it has not failed in CI.
- Every remaining bare `task run` under `rust/loopflow/tests/` (audited 2026-10-25) is `--explain`, a
  refusal, an unknown Task, the `end` edge (runs no Graph), or a task_graph_launch test of detaching.
- Unanswered: whether other commands that went "returns once running" (if any) have the same test
  exposure. Only `task run` was checked.

### Prevention

Done here: `-b` is the one waiting form (the #1679 poll loop is deleted; the Task brief's shared
fixture helper is unnecessary), and AGENTS.md's Rust-tests list names the rule and the
`grep -rn '"task", "run"' rust/loopflow/tests/` inventory for return-point changes.

## Deferred, not filed

- ci-fix grepping sibling call sites when it diagnoses a race: unmeasured (proof would be a replay of
  #1679's failure) and pulls against ci-fix's narrow mandate. File only if another race repair stops
  at one line.
- Auditing other "returns once running" commands (if any): no failure evidence; the AGENTS.md rule
  covers new `task run` callers.
- Not proposed: a test-only delay knob in `task run`. It reshapes production code for tests, and the
  probe already shows the race on demand.
