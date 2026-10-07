# Run a Task on another machine — LOO-412

Jack Heart authorized step 3 on 2026-10-07: implement, publish a PR stacked on
LOO-394 / #1484, then stop for review. No landing or other remote-work slice.
The complete multi-step design and dated decisions remain at
`e50dbd749e3207599f9937ce45653f4c6f33a5cd:scratch/work-on-another-machine-name.md`.
LOO-411 owns machine registration; credentials, detached work, relay, Desktop
restoration and cross-machine observation belong to later Tasks.

## Outcome

`lf ssh <machine> --task <ISSUE> …` adopts an absent Task and checks out its
pushed implementation. Repetition reuses the Task and checkout. New Task IDs
are deterministic from the Linear issue ID; existing IDs survive unchanged.
Unpushed branches or commits are named in an error without committing, pushing
or resetting them. Each machine owns its Workflow, Sessions and history.

## Implementation

- Global `--task` uses Task preparation on a registry miss, sharing the existing
  adoption operation with `task run` and `task checkout`.
- Fetch origin branch refs before placement and missing-checkout recovery,
  including branches with no GitHub PR; prune obsolete remote-tracking refs.
- SSH sends the issue name, branch, required commit and existing `PmTaskRecord`
  in its stdin preamble (`LF_TASK_SOURCE`). This is per-invocation input, not a
  new store. No path, Task ID or execution authority
  crosses machines. The original planning observation time survives.
- The recipient imports planning only when absent, through the existing guarded
  writer, validating repository Team and Wave ownership. Existing planning,
  removal and invalidation evidence wins. A cold target needs no Linear read
  when origin planning is available. GitHub remains the PR lookup owner.
- The explicit issue seeds a cold, unselected Wave with its exact active Project.
  Existing selections and unfinished rotation remain authoritative. Registration
  and launch retain their current-Project checks; no provider Project is created.
- UUID v5 derives new Task IDs from the issue's stable ID. No schema migration.

## Delete — do not maintain

- The local-only `--task` lookup in `ops/run.rs`.
- Random Task ID minting in `create_prepared_task`.
- The PR-dependent fetch repair after placement has already selected a strategy.
- Forwarding the origin's machine-local Work declaration over SSH.

Preserve stored Task/PR IDs, local edits, Workflow state and history. Transport
requirements cannot move an existing Task's checkout or branch. A retained
checkout lacking the requested commit reports the needed branch/commit and
`lf sync`; no automatic reset or replay.

## Remaining

Sync the changed parent and publish. Gate owns affected suites and the real SSH
transport acceptance; no installed machine or provider credential was exercised.
Review kept SQLite admission intact, moved missing-code checks before planning
reads, preserved checkout-recovery ordering, and stopped forwarding machine-local
Work declarations and SSH source data to descendant invocations.

Checks: `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, `git diff --check`, and network-isolated Rust tests pass (4 remote adoption/public-dispatch, 8 binding, 5 preparation); checkout-recovery ordering repair pending focused rerun; full affected suites and real SSH transport remain with gate.
