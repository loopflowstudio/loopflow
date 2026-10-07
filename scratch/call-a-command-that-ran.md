# Call a command that ran a process (LOO-400)

Jack Heart requested the rename on 2026-10-07 and authorized autonomous design,
implementation and publication, stopping at pr-review. This is the selected
implementation design, not a recorded review approval.

## Shape

Use Process/ProcessId in Rust and the Desktop wire model; process_id,
parent_process_id and processes on the wire and in SQLite. FlowProcess and
FlowProcessStep describe the existing append-only Flow metadata on a process;
they introduce no new lifecycle or identity. The Flow is one lf process and its
step processes. Prefer “the command exited” when identity is unnecessary.
Engineering process remains ordinary prose; use “lf process” where ambiguous.
No command-map changes or new ps command (LOO-397). Rename identity flags such
as --accept-unknown-process with their concept. Keep execution-state fields.

## Delete — do not maintain

Replace Exec/ExecId, FlowExec/FlowExecStep, exec DTO fields, old model/module
filenames and current documentation terminology end to end. No parallel API or
wire aliases. Preserve Unix exec operations, external provider protocols,
released migration bytes, release notes, dated evidence and opaque history.
A single draft renames live SQL tables/columns, indexes/triggers and revision
domain while preserving every row, identity, parent, exit and ownership fence.
Historical JSON/capture formats must remain readable where persisted outside SQL;
inspect those boundaries explicitly before changing their encoding.
Delete the unused `Runner`/`DefaultRunner` abstraction; `run_agent` owns launches.
Let SQLite rewrite references in unchanged indexes/triggers instead of copying
their definitions into the rename draft.

## Preservation and acceptance

A populated released-frontier migration must retain parent/child records, unknown
and failed outcomes, Session provenance, Flow steps, Task workflow and revision
invalidation. Existing DTO fixtures must decode in Rust and Swift. Help, Task
view and current docs use process. Inspecting a command retains parent and exit.
Build and focused tests here; broader gate and mounted judgment remain later.

## Inventory and review findings

The existing owners are `process.rs` (record), `journal` (admission and receipts),
`store/sqlite/processes.rs` (queries and Session fences), `ops/flow_run.rs` (Flow
metadata), and `engine/process.rs` (OS launches). The last is a different job;
prompt preparation moves from `engine/exec.rs` to `engine/process_prompt.rs`.
The old record modules and DTO fixture are renamed, with no parallel reader or
hidden command. `ExecProcessReceipt` becomes `ProcessReceipt`; it retains the
existing on-disk directory and encoding. Foundation.Process is explicit wherever
Swift launches an OS process, distinct from Loopflow's durable Process DTO.

Review found persisted manifest `exec`, receipt `exec_id`, AgentCaller
`origin_exec_id`, boot witness `provider_exec_id`, trace node `exec` and Task
decision `exec_ids` outside renamed SQL columns. These historical formats remain
readable; the first four retain one write encoding and the last two accept old history while emitting process
names. Public process DTOs have only the new names. Task acceptance's SQL reader
recognizes the two dated event spellings without modifying append-only bytes.
Unix shell exec, pre_exec and external provider commands are unchanged.
Jack Heart's October 7 cleanup steer is satisfied by replacing the existing owners;
no alternate execution model, alias command or new ps surface is introduced.

Compression reproduced a lost boot witness: the renamed JSON reader could not
recover after a restart from released evidence. Retaining its encoding fixes
that boundary without rewriting history or adding another format. The regression
checks the saved witness, provider generation and native thread. Launch functions
now say `run` or `launch`; `SessionLaunch` describes a command and optional app URL.
The unused runner abstraction and redundant migration definitions are removed.

## Remaining work

PR #1483 is published at `51c409ab86e49c0a3f172abb9a4d8b4673335895`;
`scratch/pr-review.html` remains pinned to that head. This compression diff is
local and needs publication and a walkthrough refresh before review. `lf commit`
returned an empty error; the preexisting review note is preserved at
`/tmp/loo400-pr-review-note.md`. Reviewer assessment, full gate/CI, installation
container checks and installed migration remain later boundaries. Rendering is
unavailable; no landing is authorized.

Checks: `cargo test -p loopflow --lib` with filters `process_names_preserves_released_history_and_constraints`, `session_record::recovery::tests`, `skill_invocation_seed`, `session_launch_` passed 25 tests with LF_* cleared; `cargo clippy --all-targets -- -D warnings`, `cargo fmt --all -- --check` passed; published-head checks remain at `51c409ab8:scratch/call-a-command-that-ran.md`; broader acceptance belongs to gate/CI.
