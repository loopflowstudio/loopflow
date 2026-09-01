# DTO wire fixtures

Each fixture pins one live wire shape. Swift fixtures cover the per-Wave
listener and `lf status` contracts consumed by the Mac app. Every absent field
is a parse error or an explicit null.

Carve-out: `resident_deltas.json` and `resident_door.json` are the wave
listener↔resident wire (`POST /resident/deltas`, `POST /resident/attach`,
`GET /resident/context` — see `rust/loopflow/src/wave/wire.rs`). Both ends are
the same `lf` binary, so only the Rust fixture tests pin them. Swift does not
consume this wire.

`session.json` pins `lf session open --json`: one unresolved Task FlowStep
session, its explicit readiness state, and its exact Home-local attach route.

`task_condition_states.json` pins the Rust-owned Task condition fold for clean
backlog, completion, external and human waits, local recovery blockers, and
unavailable evidence. Rust and Swift decode the same Task rows; consumers never
reconstruct the condition from process flags.

`activity_snapshot.json` pins `lf ps --json`: exact live Exec and provider
processes carry OS-derived state, while a provider without exact ownership
stays separate from the call tree. Rust and Swift both round-trip it; The
Podium derives no process state of its own.

`wave_detail.json` embeds the `RunSnapshot` row shape shared by both bundle
reads: `lf runs --json` and `lf usage --json`. Provider cumulative counters
remain optional; stream finality and evidence gaps are required, explicit
evidence.

`work_activity_snapshot.json` pins `lf activity --json`: durable Work creation,
Run, PR, and Steer facts retain their existing Work, Run, author, and GitHub
identities. Rust and Swift both round-trip it; The Podium filters this one
history instead of maintaining a second activity store.

`pm_show.json` pins the repository-owned Linear hierarchy read by `lf pm show`
and the app: a Project carries exactly one Wave Initiative and the repository
Team; its Task carries the stable Project and Team ids used for ownership.
The shared `LOO-*` identifier and canonical Project name remain presentation;
the provider's Wave-qualified title is normalized before this wire boundary.
