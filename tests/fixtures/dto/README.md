# DTO wire fixtures

Each fixture pins one live wire shape. Swift fixtures cover the per-Wave
listener and `lf status` contracts consumed by the Mac app. Every absent field
is a parse error or an explicit null.

Carve-out: `resident_deltas.json` and `resident_door.json` are the wave
listener↔resident wire (`POST /resident/deltas`, `POST /resident/attach`,
`GET /resident/context` — see `rust/loopflow/src/wave/wire.rs`). Both ends are
the same `lf` binary, so only the Rust fixture tests pin them. Swift does not
consume this wire.

`invocation_surface.json` pins `lf invocation list|attach --json`: one
AgentInvocation, its supervising Run authority and containment, stable Work and
Wave identity, Home route, temporary User/parent attention and handback
evidence, and a generic attach argv.
`task_attention_states.json` pins the Rust-owned desktop attention fold for
live advancing, live human wait, dead dirty, dead authored commits, clean
backlog, completed, stale active intent, and unavailable local evidence. Rust
and Swift decode the same Task rows; consumers never reconstruct the signal
from process flags.

`context_lab_snapshot.json` is shared by Rust and Swift. It pins the atomic
invocation-set query, including explicit missing token coverage and immutable trace
addresses. Revision evidence carries both Rust's effective-content hash and the
current source-file hash used to reject a stale Task worktree without
reimplementing prompt transformations in Swift.

`turn_spend.json` is the additive `lf usage --json` wire. Each row names its
Turn, AgentInvocation, trace, and exec; the second row proves a cache-only measurement
survives while absent token and cost fields remain explicit nulls. Rust and
Swift both round-trip it.

`activity_snapshot.json` pins `lf ps --json`: attributed Exec and provider
nodes retain working/stalled state and exact token rates, while a registered
orphan stays separate from the counted call tree. Rust and Swift both
round-trip it; The Podium derives no process state of its own.

`work_activity_snapshot.json` pins `lf activity --json`: durable Work creation,
Run, PR, and Steer facts retain their existing Work, Run, author, and GitHub
identities. Rust and Swift both round-trip it; The Podium filters this one
history instead of maintaining a second activity store.

`pm_show.json` pins the repository-owned Linear hierarchy read by `lf pm show`
and the app: a Project carries exactly one Wave Initiative and the repository
Team; its Task carries the stable Project and Team ids used for ownership.
The shared `LOO-*` identifier and canonical Project name remain presentation;
the provider's Wave-qualified title is normalized before this wire boundary.
