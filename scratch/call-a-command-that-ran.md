# Call a command that ran a process (LOO-400)

Jack Heart requested the rename on 2026-10-07 and authorized autonomous design,
implementation and publication. On October 7, Jack approved the refinement
`process.lfid` for durable identity and `process.pid` for the Unix PID, and
authorized landing PR #1483 (comment `32c00054-4c60-4b96-bdf4-4d5f142ab881`).
That releases the earlier pr-review stop; no further design/demo review is required.

## Shape

Use Process/ProcessLfid in Rust and the Desktop wire model; lfid and optional
pid on Process, process_lfid and parent_process_lfid for references, and
processes in SQLite. Durable LFIDs remain unique when historical PIDs collide.
Existing records retain absent PID evidence; new command records capture their
own PID. Extend the single draft directly from the released schema. FlowProcess and
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

The approved LFID/PID refinement is implemented across Rust, Swift, DTO fixtures,
current documentation and the existing migration draft. New commands retain their
PID; historical rows retain null PID, identities, parents and outcomes. Two LFIDs
sharing a PID remain independent. Flow projections now read their extra columns by
name instead of relying on the Process field count. Python's external
`importlib.exec_module` API is restored after the initial rename changed it.

The walkthrough now pins implementation `1a2c42ee3`. PR #1483 awaits
publication of this refinement and the authorized landing handoff. No further design/demo approval
is required. Installed migration remains unproved; rendering is unavailable.
The install exact-frontier fixture needs CI's materialized migration graph; it
cannot classify a store with an unpublished draft as canonical. The earlier full
Rust run exposed the repaired Flow column offsets and a changed historical fixture;
released fixture bytes are restored. Subsequent process-isolated checks pass.

Checks: `cargo nextest run -p loopflow --lib` plus nine affected integration targets, excluding `an_exact_store_is_validated_in_place`, passed 1,841 tests (13 skipped); `scripts/test_desktop.sh -Xswiftc -gnone` passed 388; `uv run pytest python/tests/ -q` passed 406; Clippy, fmt, architecture, Swift boundaries, migration immutability and disposable CLI help/list/show passed; canonical install compatibility and isolated installation proofs remain with CI.
