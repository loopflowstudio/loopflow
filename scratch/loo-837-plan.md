# LOO-837 plan

Status: implemented. One rule in `rust/loopflow/src/ops/task.rs`: `select_stack_parent` (read; also used by `checkout --explain`) and `record_stack_parent` (selects, then pushes if origin lacks the branch or is behind, never force; record an unpublished `task_prs` row via `delivery::next_task_pr`, shared with publication, when the parent has none). Store `stack_task_placement` no longer requires a GitHub PR, only an unsettled one.

Why kickoff-placed Tasks had no row: `place_task_in` (`store/sqlite/children.rs`) inserts the placement PR only when `publication.is_some()`, i.e. when a GitHub PR already exists for the branch. Every fresh placement behaves this way, not only `feature`-graph ones; 821–828 had rows because they had published. Placement is unchanged; recording a `Working` row at every placement would change board and snapshot phases far outside this Task. Say so in the PR.

Diverged parent: when origin's parent branch has diverged from the local one, nothing is pushed or refused; the child bases on `origin/<branch>`. `stack_parent_branch` classifies the branch with one `rev-list --left-right --count` (current, behind → push, diverged with the count of local-only commits; neither local nor on origin is an error), and the stack line names what the child leaves out, both for `stack_existing_task` and new placement. `fetch_task_refs` runs first, so the remote-tracking ref is current.

`lf checkout --explain --stack-on P` lists what stacking would change outside the Task (push of P's branch, unpublished PR record for P, the diverged note) under `unavailable`, from refs already fetched.

Concept review (2026-11-11): the model holds. The stack edge stays `parent_pr_id` → `task_prs` (retries keep the parent's PR identity); an unpublished row is phase `Working`, which `task_actions` treats like no row, so the parent's board and actions don't change. Second pass after the diverged work: basing on `origin/<branch>` is the only choice that neither forces nor ships the parent's unpublished commits inside the child's PR; the note says what was left out. Nothing to change.

Remaining: gate; PR body states the placement finding above. Docs (`docs/ops.md`, `docs/dev/delivery.md`) describe the diverged case.

Checks:
- `cargo test -p loopflow --lib stack`: pass (new `task_preparation_stacks_on_origin_when_the_parent_branch_diverged`)
- `cargo test -p loopflow --test integration sync_tests::stack`: pass
- `cargo test -p loopflow --test global_commands task_checkout_explain_reuses`: pass (stack effects under `unavailable`)
- clippy `--all-targets`: clean
- compress pass (2026-11-11): same three test commands and clippy pass after folding `Missing` into an error
- QA (2026-11-11, `scratch/qa.html`): real CLI against a bare origin; the push was silent, now printed (`e6c5971f1`). Lib stack tests and clippy pass after it.
- Gate owns the affected-suite run.
