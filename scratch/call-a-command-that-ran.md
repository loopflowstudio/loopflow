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
`origin_exec_id`, trace node `exec` and Task decision `exec_ids` outside renamed
SQL columns. These historical formats remain readable; the first three retain
one write encoding and the last two accept old history while emitting process
names. Public process DTOs have only the new names. Task acceptance's SQL reader
recognizes the two dated event spellings without modifying append-only bytes.
Unix shell exec, pre_exec and external provider commands are unchanged.
Jack Heart's October 7 cleanup steer is satisfied by replacing the existing owners;
no alternate execution model, alias command or new ps surface is introduced.

## Remaining work

Publish for review and prepare the pr-review HTML walkthrough. Full gate/CI,
installation-container checks, installed migration and interactive judgment remain
later boundaries. No rendering environment is available for the walkthrough.

Checks: Rust populated migration, discovery, Flow inventory, journal, retained acceptance/capture, DTO, process ownership and observation checks passed (installation-only proofs deferred to isolated CI); cargo check, fmt, Clippy, architecture and docs sync passed; 42 headless Desktop tests passed; disposable CLI help/show proof passed.
