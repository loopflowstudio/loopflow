# Review: managed Tasks survive missing lifecycle flows

Reviewed 2026-08-16 against the LOO-207 directive and
`scratch/make-managed-tasks-survive-missing.md`. The live stranded Tasks were
kept read-only as required; the closest production-like demonstration is the
isolated LOO-193 fixture through the real Task status/resume surfaces.

## Evidence

| Claim | Planned behavior | Implemented behavior | Proof | Result |
|---|---|---|---|---|
| Recorded LOO-193 recovery | Status advertises the exact `task` to `slice` migration; explicit resume preserves Task identity, dirty work, provider continuation, lifecycle cursor, and PR history | One shared lifecycle preflight is read-only for status and persisted only by explicit resume before launch | `cargo test -p loopflow --test managed_task_lifecycle_tests` (`parked_dirty_legacy_task_resumes_through_an_explicit_migration`) | pass |
| Merged predecessor and active successor | Migration changes only the lifecycle pin | The Task-level update leaves the ordered PR rows unchanged | same suite (`legacy_migration_preserves_merged_prs_and_the_open_successor`) | pass |
| Missing worktree | Status remains readable, recommends `no_action`, and names exact path/branch recovery without mutation | Resume preflight catches adoption failure before reconciliation and projects it through the action model | same suite (`missing_worktree_status_is_actionable_and_read_only`) | pass |
| Unknown missing flow | Status and command both fail closed; repeated launch attempts reserve no Runs | Only exact historical `task` is migratable; all other resolution failures become refusals before Run reservation | same suite (`unknown_missing_flow_never_becomes_a_run_or_legal_resume`) plus `unavailable_persisted_flow_is_rejected_before_every_run_reservation` | pass |
| New Task validation | An unavailable plan persists no Task or PR | Both `task run` and `task start` resolve and phase-validate lifecycle flows before local Task/PR persistence; `task start` also validates before its PM mutation | `new_task_lifecycle_rejects_an_unavailable_flow_before_persistence`; source ordering in `ops/task.rs` | pass |
| Automatic recovery with settled PR | Invalid lifecycle creates neither a successor PR nor a Run | Automatic relaunch validates before `ensure_working_pr` and recovery planning | `automatic_relaunch_rejects_an_unavailable_flow_before_pr_rotation` | pass |
| Resident convergence | Two ticks over LOO-167, LOO-193, and LOO-195 shapes write one actionable event each, with zero Runs and no PR/Task mutation | The resident compares the exact latest failure receipt and parks before PR reconciliation | `resident_records_missing_lifecycle_flow_once_without_retrying` | pass |
| Initialization publication | Task, Working PR, Steer, and initialization evidence publish atomically; status, zero-timeout wait, and roadmap remain readable with no Run | SQLite publishes `WorktreeInitializing` in the creation transaction and appends `PrStarted` after worktree creation | `initial_task_publication_includes_worktree_initialization`; managed lifecycle suite (`initializing_worktree_keeps_status_wait_and_roadmap_readable`) | pass |
| Required gates | Focused suite, lifecycle tests, formatting, and clippy pass | All exact commands complete cleanly | commands recorded below | pass |

## Review findings

- Fixed the resident proof fixture: it borrowed the repository under test and
  failed whenever review ran during a detached-HEAD rebase. It now owns an
  isolated repository and branch, so the non-retry proof is deterministic.
- The compressed resume model has one legal state (`Ready`, `Migration`,
  `Initializing`, or `Refused`) instead of independent optional strings and a
  Boolean. Status, roadmap, and action derivation cannot disagree through an
  impossible combination.
- No `task` compatibility flow alias exists. The only special case is the
  explicit persisted-plan migration at resume.

## Commands

```text
cargo test -p loopflow --test managed_task_lifecycle_tests       # 5 passed
cargo test -p loopflow task_lifecycle                            # 4 passed
cargo test -p loopflow unavailable_persisted_flow_is_rejected_before_every_run_reservation
cargo test -p loopflow automatic_relaunch_rejects_an_unavailable_flow_before_pr_rotation
cargo test -p loopflow resident_records_missing_lifecycle_flow_once_without_retrying
cargo test -p loopflow initial_task_publication_includes_worktree_initialization
cargo fmt --all --check
cargo clippy -p loopflow -- -D warnings
```

## Publication boundary

The source and behavioral review pass. Publication is still blocked before the
PR boundary because this worktree is mid-rebase and the current sandbox cannot
write Loopflow's shared Git metadata. The installed `lf rebase --plan` also
panics after its read-only ledger write fails, so the review did not bypass
Loopflow with raw Git. A subsequent `lf rebase --continue` failed with
`Operation not permitted`; `lf top` could not inspect processes under the same
sandbox. The required `lf ask --user` escalation could not be persisted because
the local ledger is read-only and this unrecorded invocation has no active Turn.
