# DTO wire fixtures

Each fixture pins one live wire shape. Swift fixtures cover the per-Wave
listener and `lf wave status` contracts consumed by the Mac app. Every absent field
is a parse error or an explicit null.

Carve-out: `resident_deltas.json` and `resident_door.json` are the wave
listener↔resident wire (`POST /resident/deltas`, `POST /resident/attach`,
`GET /resident/context` — see `rust/loopflow/src/wave/wire.rs`). Both ends are
the same `lf` binary, so only the Rust fixture tests pin them. Swift does not
consume this wire.

`task_status.json` pins the planning/execution envelope for available, unavailable,
invalid, removed and absent planning. Rust round-trips it; Swift decodes the `TaskStatus` model
returned by `RegistryQuery.taskStatus`. These cases have no execution. Complete
Task execution/action-state parity remains acceptance work; Wave contracts have
their own fixtures below.

`task_execution.json` pins the execution boundary inside the status envelope's
optional Task snapshot (`execution.execution`).
This CLI-only contract is tested in Rust. The Mac app receives its derived
condition, reason, and actions through the existing Wave Task row.

`session.json` pins `lf session open --json`: one unresolved Task FlowStep
session, its explicit readiness state, and its exact Home-local attach route.

`session_memberships.json` pins each Session's required `flow_membership`:
the current Flow step, an earlier step, an independent conversation, a Run that
predates recorded membership (unknown, never independent), and a remote Flow
Session whose canonical name lives on its Home (`title_source: unavailable`).

`task_condition_states.json` pins the Rust-owned Task condition fold for clean
backlog, completion, external and review waits, local recovery blockers, and
unavailable evidence. Rust and Swift decode the same Task rows; consumers never
reconstruct the condition from process flags.

`activity_snapshot.json` pins `lf ps --json`: exact live Exec and provider
processes carry OS-derived state, while a provider without exact ownership
stays separate from the call tree. Rust and Swift both round-trip it; The
Podium derives no process state of its own.

`session_history_summary.json` and `wave_detail.json` pin the `SessionHistory`
shape shared by `lf runs --json` and `lf usage --json`. Captured event sequences
and native thread/turn references retain distinct outcomes. Optional counters,
missing capture/start membership, stream finality and evidence gaps stay explicit.
The optional `task_pr_id` retains the PR captured by the managed Flow;
`first_provider_attempt_at` differs from capture/import observation time.

`work_activity_snapshot.json` pins `lf activity --json`: durable Work creation,
Session capture/provider history, PR and Steer facts retain their original
identities. Rust and Swift round-trip this shared history.

`pm_show.json` pins the internal planning snapshot used by Task resolution
and status refresh: a Project carries exactly one Wave Initiative and the repository
Team; its Task carries the stable Project and Team ids used for ownership.
The shared `LOO-*` identifier and canonical Project name remain presentation;
the provider's Wave-qualified title is normalized before this reader returns.
It is not a CLI/app DTO; Wave detail and roadmap fixtures cover those boundaries.

`task_execution_stalled.json` and `task_flow_stalled.json` retain the same stalled Run and interrupt → resume reason across CLI and desktop.

`task_files.json` pins `lf task changes/diff/file/save --json`: exact comparison bases,
rename paths, scratch listing, and lossless content with a byte revision and
explicit file state, Save outcome, recovery access and late-change disclosure.
Rust round-trips the same fixture Swift decodes.
