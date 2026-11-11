# LOO-837 plan

Status: implemented. One rule in `rust/loopflow/src/ops/task.rs`: `select_stack_parent` (read; also used by `checkout --explain`) and `record_stack_parent` (selects, then pushes if origin lacks the branch or is behind, never force; record an unpublished `task_prs` row via `delivery::next_task_pr`, shared with publication, when the parent has none). Store `stack_task_placement` no longer requires a GitHub PR, only an unsettled one.

Why kickoff-placed Tasks had no row: `place_task_in` (`store/sqlite/children.rs`) inserts the placement PR only when `publication.is_some()`, i.e. when a GitHub PR already exists for the branch. Every fresh placement behaves this way, not only `feature`-graph ones; 821–828 had rows because they had published. Placement is unchanged; recording a `Working` row at every placement would change board and snapshot phases far outside this Task. Say so in the PR.

Diverged parent: when origin's parent branch has diverged from the local one, nothing is pushed or refused; the child bases on `origin/<branch>`. `stack_parent_branch` classifies the branch with one `rev-list --left-right --count` (current, behind → push, diverged with the count of local-only commits; neither local nor on origin is an error), and the stack line names what the child leaves out, both for `stack_existing_task` and new placement. `fetch_task_refs` runs first, so the remote-tracking ref is current.

`lf checkout --explain --stack-on P` lists what stacking would change outside the Task (push of P's branch, unpublished PR record for P, the diverged note) under `unavailable`, from refs already fetched.

Concept review (2026-11-11): the model holds. The stack edge stays `parent_pr_id` → `task_prs` (retries keep the parent's PR identity); an unpublished row is phase `Working`, which `task_actions` treats like no row, so the parent's board and actions don't change. Second pass after the diverged work: basing on `origin/<branch>` is the only choice that neither forces nor ships the parent's unpublished commits inside the child's PR; the note says what was left out. Nothing to change.

Remaining: CI or a Machine with at least 32 GiB free must run the affected suites, clippy and package verification below before shipping. PR body states the placement finding above. Docs (`docs/ops.md`, `docs/dev/delivery.md`) describe the diverged case.

Gate plan (2026-11-11): compare with checkout base `976b53db0` (also the merge-base with `origin/main`). Task preparation, parent selection and PR creation reach `ops::task::tests::`, `pr_tests::`, and the graph's `task_checkout_selects_parent_without_rewriting_work_or_publication`; store placement reaches `store::sqlite::children::local_planning_tests::`; CLI help and explain reach `lf::tests::` and `global_commands::task_checkout_`; stack integration reaches `sync_tests::`. Docs and the embedded ship-decomposed skill require `documented_commands::`, the runner's architecture/website checks, and `cargo package --locked --allow-dirty -p loopflow`. Shared Git helpers, schema, Desktop and provider transport are unchanged; their slow suites stay with CI. Use the changed-aware runner with passing-result reuse and this Rust filter, followed by package verification, serially.

Gate repair: an idempotent retry after its parent merged must retain the historical parent without checking or republishing its retired branch. Added a regression with and without the local branch. Explain now labels branch forecasts as based on already-fetched refs. The unpushed-parent acceptance also performs `lf sync` and checks the inherited file.

Checks:
- `cargo test -p loopflow --lib stack`: pass (new `task_preparation_stacks_on_origin_when_the_parent_branch_diverged`)
- `cargo test -p loopflow --test integration sync_tests::stack`: pass
- `cargo test -p loopflow --test global_commands task_checkout_explain_reuses`: pass (stack effects under `unavailable`)
- clippy `--all-targets`: clean
- compress pass (2026-11-11): same three test commands and clippy pass after folding `Missing` into an error
- QA (2026-11-11, `scratch/qa.html`): real CLI against a bare origin; the push was silent, now printed (`e6c5971f1`). Lib stack tests and clippy pass after it.
- Gate (2026-11-11): `uv run python scripts/test.py --reuse-passing --rust-filter 'test(ops::task::tests::) | test(pr_tests::) | test(graph_tests::task_checkout_selects_parent_without_rewriting_work_or_publication) | test(store::sqlite::children::local_planning_tests::) | test(lf::tests::) | test(task_checkout_) | test(sync_tests::) | test(documented_commands::)'`: stopped at resource preflight, **no suites ran**; 17.4 GiB free / 32 GiB reserve. Recovery reclaimed 0; uv cache busy, build cleanup exited 2. `lf wt prune --builds --dry-run` then retained every candidate, reclaimable 0. Receipt: `.lf/tmp/gate/run-44733`. CI owns Rust, clippy and website results for the gate repair; earlier passes above do not cover it.
- `cargo fmt --all -- --check`: pass with rustup stable 1.99.1 (CI tracks stable; prior Homebrew compiler was 1.98.1).
- `uv run --no-sync python scripts/check_architecture.py`: pass, zero unexplained owners/mirrors/shims/stale vocabulary.
- `uv run --no-sync python scripts/check_rust_test_inventory.py` and `git diff --check`: pass.
- `cargo package --locked --allow-dirty -p loopflow`: deferred to CI/capable Machine because of the same disk reserve; embedded skill changes require this check.
