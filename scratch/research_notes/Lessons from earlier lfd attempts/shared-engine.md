# Shared engine versus multiple execution paths

Read-only source audit · November 3, 2026 · LOO-718

Jack Heart identified multiple implementations of the same behavior as a likely problem in earlier lfd attempts. This audit tests that concern against the present checkout and proposed Machine service. It recommends architectural boundaries; it does not record implementation approval. Current-source references are pinned to `2bc29a72e0c3479dff528a4232e8d9d18268a0b2`. No builds or tests were run.

## Does sharing the engine make this service materially different?

### Takeaway

Yes, provided sharing includes command admission, routing, execution and recovery semantics. But Desktop and CLI already share much of that engine today. Moving FFI behind a socket changes delivery, lifetime and failure boundaries; it does not establish engine sharing for the first time.

### Cited Findings

- Desktop's `RegistryQueryLocal` has mixed delivery: Rust FFI for served reads, local served writes and Work subscriptions; subprocess `lf` for other commands, remote writes and remote Task-file streams. That is multiple adapters, not evidence of independent Task implementations. [RegistryQueryLocal](https://github.com/loopflowstudio/loopflow/blob/2bc29a72e0c3479dff528a4232e8d9d18268a0b2/swift/LoopflowMac/Services/RegistryQueryLocal.swift#L12)
- `Read::read` calls the existing domain read functions. `Read::subscribe` selects the existing watch functions using `Transport::Pull`; CLI watches use `Transport::Stdio` through the same watch machinery. The C layer deserializes a `Read`, calls those methods, manages handles and translates failures. [Read](https://github.com/loopflowstudio/loopflow/blob/2bc29a72e0c3479dff528a4232e8d9d18268a0b2/rust/loopflow/src/lf/commands/read.rs#L349), [watch transport](https://github.com/loopflowstudio/loopflow/blob/2bc29a72e0c3479dff528a4232e8d9d18268a0b2/rust/loopflow/src/lf/commands/watch.rs#L188), [FFI](https://github.com/loopflowstudio/loopflow/blob/2bc29a72e0c3479dff528a4232e8d9d18268a0b2/rust/loopflow-ffi/src/lib.rs#L74)
- `Write::write` turns its operation into CLI argv, parses it with Clap, checks shared Work routing, resolves the repository and records a command Process where applicable. Its operation match calls shared Task/Session/Project functions. A remote destination returns `None` before mutation; Desktop then executes the CLI. The wrapper still maintains operation/argv/output mappings, so a shared library does not remove every drift opportunity. [Write](https://github.com/loopflowstudio/loopflow/blob/2bc29a72e0c3479dff528a4232e8d9d18268a0b2/rust/loopflow/src/lf/commands/write.rs#L391)
- Desktop New Task already runs `lf task create --run --json`. Rust starts ordinary `task run`, then admits the primary conversation. On a run failure after filing, stdout still contains the created Task receipt and the command exits nonzero; Swift preserves that receipt in `TaskCreateRunFailed`. [Swift creation](https://github.com/loopflowstudio/loopflow/blob/2bc29a72e0c3479dff528a4232e8d9d18268a0b2/swift/Loopflow/Services/RegistryQuery.swift#L772), [Rust orchestration](https://github.com/loopflowstudio/loopflow/blob/2bc29a72e0c3479dff528a4232e8d9d18268a0b2/rust/loopflow/src/lf/commands/task_create.rs#L1)

### Inferences

The right invariant is **one owner for each business decision**, not one OS process or one function stack. A local CLI can execute without the Machine service while the service delegates writes to that CLI. Both still use one execution implementation. This also avoids service → CLI → service recursion.

Separate local-UID admission and remote phone authentication are necessary adapters. Separate framing, cancellation, buffering and reconnect logic are also legitimate. Separate Task placement rules, graph admission, provider launch decisions, primary-conversation selection or retry state machines are duplication.

### Gaps

Source reuse alone does not prove equivalence of surrounding behavior: environment, repository resolution, command journaling, exit/result formatting and version skew remain observable. Existing mapping and FFI tests do not constitute a complete end-to-end parity proof.

## Where should the paths converge?

### Takeaway

The proposed service can replace Desktop's Work delivery boundary while keeping the existing command, watch and retained-terminal owners. Avoid adding a fourth business dispatcher behind the new wire contract.

### Cited Findings

The table distinguishes implemented paths from the proposal in [mobile-backend.md](../../mobile-backend.md). Current implementation evidence is the Read, Write, RegistryQuery and task-create sources above, plus shared [Work routing](https://github.com/loopflowstudio/loopflow/blob/2bc29a72e0c3479dff528a4232e8d9d18268a0b2/rust/loopflow/src/lf/commands/work_route.rs#L1) and [terminal transport contract](https://github.com/loopflowstudio/loopflow/blob/2bc29a72e0c3479dff528a4232e8d9d18268a0b2/rust/loopflow/src/transport/README.md).

| Surface / operation | Current call path | Proposed convergence | Duplication to prevent |
|---|---|---|---|
| Desktop local Work reads | RegistryQuery → LoopflowLibrary → lf_read → Read → domain reads | RegistryQuery → Unix socket → same Read | A service-owned Task projection or alternate status rules |
| Desktop local Work watches | FFI subscription → existing watch Pull transport | Socket subscription adapter → same Subscription | Second watcher/scheduler, changed unavailable semantics, durable shadow cache |
| CLI reads/watches | CLI command → same domain read/watch, stdio delivery | Remains direct | Making CLI depend on service merely for visual uniformity |
| Desktop local served writes | Write → Clap/routing/journal → shared operation | Typed socket command → canonical argv → installed lf → shared operation | Additional service implementation of Task mutation or journal attribution |
| Desktop remote writes | FFI determines remote route without writing → CLI → existing remote transport | Service delegates to CLI, preserving explicit destination and canonical routing | Independent HTTP placement rules; executing locally after remote failure |
| Remote CLI commands | Work chooses destination; SSH transports ordinary command | Retain existing route; iOS may connect directly to destination service | Two authorities for Machine/Task placement |
| Desktop create/start | RegistryQuery → lf task create --run --json → task run → primary admission | Same command from Desktop and phone, with recoverable request identity | Server manually creating Task, starting graph and constructing another primary Session |
| Terminal editing | lf resume resolves Session → retained owner socket → provider/PTY | Service bridges that owner protocol to phone; Desktop rendering stays native | New server PTY/retention manager; replaying ambiguous input |
| Native Desktop interaction | App-owned window/focus/layout interaction | Remains app-owned | Making a headless service pretend to own visible windows |
| Installation/data | Existing installation promotion chooses CLI and main-Machine schema | Add service artifact/lifecycle to that authority | Desktop helper and CLI installers independently promoting binaries or migrating one store |

- Existing owners retain execution after terminal loss, arbitrate input separately from geometry/theme, expose snapshot/output/reconnect framing, and remain the provider/PTY owners. The service should bridge these mechanics, not create another retention plane. The particular idle timeout and recovery policy are current implementation details, not newly accepted mobile requirements. [Transport contract](https://github.com/loopflowstudio/loopflow/blob/2bc29a72e0c3479dff528a4232e8d9d18268a0b2/rust/loopflow/src/transport/README.md)
- Published installation already owns main database migration under its promotion lock. Ordinary processes do not advance the main schema. Stable entry dispatch selects the installed CLI while old processes may remain alive. A new helper must fit that policy. [One main Machine](https://github.com/loopflowstudio/loopflow/blob/2bc29a72e0c3479dff528a4232e8d9d18268a0b2/docs/architecture/machines.md#one-main-machine)

### Inferences

Use canonical Rust operation mappings at the service boundary. Hand-maintained Swift argv, Rust Write argv and a new server operation→argv match can diverge even when all ultimately call `lf`. Prefer extending the existing mapping where it fits, not rebuilding it endpoint by endpoint. Complex create/start can continue as one ordinary CLI command rather than forcing it into the local Write execution switch.

Request IDs are necessary new behavior, not free transport metadata. Existing TaskCreateReceipt protects the caller when a partial-success reply arrives; it cannot prevent duplicate filing after a reply disappears. Admission, payload matching and reconciliation must reach the shared command boundary. A gateway response cache alone leaves the crash between external Task creation and cache recording unresolved. If provider reconciliation cannot prove the outcome, return unknown; never promise exactly-once external effects without evidence.

A future hub should use the same authenticated command admission and deduplication path. It must not acquire its own Task/Graph scheduler or live execution database. Webhook ingress is a later producer of commands, not a reason to redesign current execution ownership.

### Gaps

No settled socket protocol, command-admission implementation, service packaging or phone authentication exists in this draft. Exact remote read routing differs by read type and needs an operation inventory; do not advertise every existing query as transparently remote merely because writes route through SSH.

## What is the smallest coherent cutover, and what does sharing fail to solve?

### Takeaway

Prove one Desktop read/watch/write path through an independently supervised service, then complete the Work transport replacement and delete its FFI glue. Keep the CLI engine and Session-owner implementation. The service must earn its additional lifetime and failure boundary.

### Cited Findings

- `LoopflowLibrary` and `ServedWatch` are transport/ownership wrappers suitable for removal after consumers migrate. Renderer FFI is a different responsibility. [Work FFI wrapper](https://github.com/loopflowstudio/loopflow/blob/2bc29a72e0c3479dff528a4232e8d9d18268a0b2/swift/LoopflowMac/Services/LoopflowLibrary.swift)
- The existing subscription drops its inbox when canceled, reports a final error frame on failure and exposes completion; a network wrapper should preserve these facts rather than silently restart failed reads as fresh success. [Subscription](https://github.com/loopflowstudio/loopflow/blob/2bc29a72e0c3479dff528a4232e8d9d18268a0b2/rust/loopflow/src/lf/commands/watch.rs#L229)
- The current draft already proposes deleting migrated Work FFI exports and rejects a permanent direct-FFI fallback. Its installation report separately calls for real supervisor lifecycle proof. [Backend draft](../../mobile-backend.md), [installation/lifecycle research](../../reports/Machine%20service%20installation%20and%20lifecycle.md)

### Inferences

Recommended cutover:

1. Exercise Task list subscription and one Task mutation through the local service using existing Read/Subscription and installed CLI execution. Include readiness, service replacement and cancellation from the outset.
2. Move remaining Work consumers to that transport. Inventory and remove unused `lf_read`, `lf_write`, `lf_subscribe`, request/pull/free exports, their Swift wrappers and exclusive build wiring. Keep shared Rust operation code and renderer FFI. Temporary migration scaffolding needs a finite endpoint, not permanent fallback.
3. Add remote admission and terminal bridging to that same service. Add recoverable create/start at the shared command boundary before exposing mobile retries.

Tests should prove outcomes, not that wrappers called wrappers:

- Run representative operations from CLI and socket against equivalent fixtures: compare changed Task/Session state, operation results and Process attribution after normalizing generated IDs/timestamps.
- Compare initial watch readings, changes, unavailable/error state and cancellation across stdio and socket. Disconnect a slow client and prove it neither blocks healthy consumers nor changes underlying Work.
- Create successfully, lose the reply, restart the service and retry the same identity: recover one Task and the original receipt, or explicitly report unresolved outcome. Create then fail graph admission: recover the created Task instead of filing again.
- Route to a remote destination and fail its transport: prove no local mutation and no second placement interpretation.
- Kill/relaunch the installed service and quit Desktop while an agent runs: the same owner/provider survives; reconnect restores the Session without duplicating it. Stale input tokens and uncertain keystrokes must not be replayed.
- Run concurrent Desktop starts and helper upgrade under real launchd/service units: one active service, one installation authority, and no schema advancement by an arbitrary client build.

Even with a single engine, risks remain:

- **Availability:** Desktop now requires a healthy local backend. A single socket can simplify code while adding a new startup/reconnect failure mode.
- **Version skew:** a long-running service may embed one Read/schema version while the stable entry launches a newer CLI. Define compatibility and restart behavior; using the same repository is insufficient.
- **Crash boundaries:** acceptance, subprocess start, external provider effects and durable receipts are separate events. Retrying them changes semantics unless admission is explicit.
- **Supervisor ownership:** a detached child can still die when its service's supervisor cleans up the process group. This needs installed-service proof, not just process-tree reasoning.
- **Resource bounds:** orphan watches, blocked writers and terminal buffers can accumulate in a process that never exits.
- **Trust:** a local Unix client and a Tailscale proxy can both arrive from the same OS UID. Remote admission cannot borrow local trust accidentally.

### Gaps

This audit cannot establish why prior versions were removed; that requires the parallel historical investigation. It establishes a useful acceptance criterion: adding a resident service is defensible if it removes an old delivery path and preserves one owner for domain decisions. “Uses the shared crate” alone is too weak, while “there are two entry points” alone is not evidence of architectural duplication.
