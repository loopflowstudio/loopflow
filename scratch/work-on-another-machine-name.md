# Run a Task on another machine — LOO-412

Jack Heart authorized step 3 on 2026-10-07: implement and publish for review.
His latest steer selects main after LOO-411 / #1489 merged, global `--machine`,
and republication of #1491. No landing or other remote-work slice.
The complete multi-step design and dated decisions remain at
`e50dbd749e3207599f9937ce45653f4c6f33a5cd:scratch/work-on-another-machine-name.md`.
LOO-411 owns machine registration; credentials, detached work, relay, Desktop
restoration and cross-machine observation belong to later Tasks.
Jack Heart's constraints remain: no code, tests, help or config from herdr/cmux;
no cross-version compatibility, shared resident process, hidden arguments or
automatic turn/Flow retry. Each redesign deletes what it replaces.

## Outcome

`lf --machine <label-or-id> --task <ISSUE> …` adopts an absent Task and checks out its
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

- The retired remote subcommand, raw-host dispatch and per-call repository override;
  LOO-411 owns the registered-machine selector and transport.
- The local-only `--task` lookup in `ops/run.rs`.
- Random Task ID minting in `create_prepared_task`.
- The PR-dependent fetch repair after placement has already selected a strategy.
- Forwarding the origin's machine-local Work declaration over SSH.
- The duplicate issue field and one-use constructor wrapper in `TaskSource`.
- The second parent-branch fetch after `prepare_new_task` fetches all origin refs.

Preserve stored Task/PR IDs, local edits, Workflow state and history. Transport
requirements cannot move an existing Task's checkout or branch. A retained
checkout lacking the requested commit reports the needed branch/commit and
`lf sync`; no automatic reset or replay.

## Remaining

Reconciled 2026-10-07 against main `35e759aaf4e79e7dc8eef2e8d30048f10172b45a`.
The Task source is attached to the registered-machine transport. Target parsing,
account selection, identity probes and saved repository remain owned by LOO-411.
The public command fixture registers the target and invokes the real recipient
CLI by both label and ID. It reads pushed implementation twice from one checkout,
then rejects a later required commit while retaining local notes and HEAD.
Source legacy Task IDs travel as issue names. SSH networking and providers are
simulated; no installed machine or real credentials are exercised.

Publication of #1491 for review remains. Gate owns full affected suites, real
two-machine SSH acceptance and focused preservation of existing Project
selection/pending rotation and Workflow/Session history. The empty Workflow
assertion proves no eager creation, not an existing Workflow's preservation.

Review retains Release's entry-point lesson: the public selector must exercise
the new transport and the actual recipient, not just call adoption directly.
The other fixtures cover two-store identity, observation age, missing pushed
code, source dirtiness and target removal evidence.

Checks: `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, `git diff --check` and network-isolated `task_remote_tests` (5) pass; full affected suites and real SSH acceptance remain with gate.
