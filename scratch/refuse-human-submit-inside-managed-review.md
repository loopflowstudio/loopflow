# Review: Refuse human submit inside managed Task worktrees

## Evidence matrix

| Claim | Planned behavior | Implemented behavior | Proof | Result |
|---|---|---|---|---|
| Managed refusal is mutation-free | `lf pr submit` refuses a managed Task before local, durable, or remote mutation and names the legal `land` actions. | `prepare_pr` resolves durable Task ownership and rejects `UserMerge` before range healing, scratch cleanup, commit/rebase, settlement writes, push, or GitHub mutation. | Current-source `target/debug/lf pr submit` against an isolated consistent backup of the live registry resolved LOO-247, exited 1 with the second-gate message, and left HEAD and status unchanged. `task_pr_authority_tests` also starts from a stale recorded base plus dirty worktree and proves `base_commit`, HEAD, status, merge request, remote branch, and GitHub log are unchanged. | pass |
| Ordinary submit remains available | A non-Task PR is prepared, marked ready, and assigned for a human merge without auto-merge. | The managed rejection is an explicit no-op for an unrelated worktree; the existing submit path continues. | `cargo test -p loopflow --test task_pr_authority_tests` passed all 10 tests, including `ordinary_submit_still_assigns_for_review`. | pass |
| Task settlement stays exact-head and automatic | Managed Task delivery can record only an Auto request pinned to the published GitHub head; no controller or obsolete InteractionReview model is required. | The active Task write/replay API is `request_task_pr_auto_merge` / `matching_task_pr_auto_merge_request`; `enable_auto_merge` always sends `--match-head-commit`. Historical User rows remain readable by reconciliation only. | `controller_free_task_land_records_and_replays_only_auto_settlement` passed. Source search found no `InteractionReview`, generic `request_task_pr_merge`, or generic replay API in active code. | pass |
| Prompts and user docs teach one Task sequence | Publish evidence during work, review in `finally`, then `lf pr land`; reserve `submit` for non-Task PRs. | LOOPFLOW.md, the submit and Task gate skills, architecture/user docs, rendered HTML, and goldens use that sequence. | `cargo test -p loopflow --test golden_prompt` and `uv run --project website python scripts/render_architecture_html.py --check` passed. | pass |
| Recovery preserved the LOO-162 lineage | The replacement keeps the committed parent work while settling on the recovered lifecycle. | LOO-247 is rooted on current main after stack recovery, and the merged LOO-162 commit remains in its ancestry. | `git merge-base --is-ancestor 2a242ddd9 HEAD` succeeded. | pass |
| Rust quality gates hold | Formatting and warnings remain clean. | The reviewed source and tests follow the repository Rust conventions. | `cargo fmt --all -- --check` and `cargo clippy -p loopflow --all-targets -- -D warnings` passed. | pass |

## Review finding fixed

The initial branch called `verify_task_pr_range` and
`require_task_pr_range_nonempty` before the managed rejection. Both may heal a
stale `base_commit`, so the clean-path test did not prove the advertised
mutation boundary. The rejection now precedes those helpers, and the behavior
test deliberately supplies a stale recorded base to make any regression
observable.

## Disposition

The Task contract holds after the bounded review fix. No remaining blocker or
unauthorized scope was found. The systemic 5 Whys follow-ups about architecture
review guidance remain separate prevention slices; they are not required to
restore this delivery surface.
