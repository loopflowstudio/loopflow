# First-generation lfd: useful centralization, incomplete consolidation

Research scope: January–July 14, 2026; inspected historical commits, actual source/diffs, and GitHub PR bodies. Jack's current question is whether shared engine code avoids the earlier multiple-path problem. This note separates observed bugs, contemporaneous author judgments, and architectural inference. The milestone PRs inspected had no discussion comments; their bodies are not independent incident reports.

## Was the problem simply having a daemon and CLI?

### Takeaway

The strongest supported lesson is **one implementation of each complete operation**, including resolution, defaults, state transitions and cleanup. Sharing low-level engine functions helps, but the old code already did that in places. A separate process is not itself duplication.

### Cited findings

- **January 14: lfd originally consolidated authority.** The Python daemon moved agent start/stop, trigger evaluation, session tracking and SQLite writes out of CLI processes. The commit explicitly describes replacing multiple SQLite pollers with one spawning/monitoring owner, having Swift connect through its socket, and installing launchd so agents survive app restarts. These are substantially the same motivations as today's proposal, not new discoveries. [Commit `6ea523cf27`](https://github.com/loopflowstudio/loopflow/commit/6ea523cf27ebd57e59f21e1b6fa72c4f8ad97c54)
- **January 30 and February 9: concrete duplicate-path repairs.** #230 merged nearly identical Python single-step/flow execution into `execute_step()`. More decisively, #304's diff shows separate HTTP `run` and `continue` closures calling the same executor, but only `run` marked the Wave failed on error. Extracting `spawn_run_task_with_slot` fixed that mismatch. The duplicate semantics were around a shared executor, inside one server process. [#230 / `f0f33c5a79`](https://github.com/loopflowstudio/loopflow/commit/f0f33c5a79fb671015e0e17556bf9a1ea4eb89a0), [#304 / `d96f6a2eb9`](https://github.com/loopflowstudio/loopflow/commit/d96f6a2eb90c5669455cba40510d7d2773d87182)
- **By June, CLI and REST shared `ops::land`, `ops::next_branch` and `ops::combine_prs`.** However, REST separately resolved the Wave/worktree, selected options, ran the operation, synchronized PR data back to the run, and emitted events. CLI assembled its own `LandOptions` and ambient context. Thus “shared engine” already existed below transport-specific orchestration. This proves multiple wrappers with substantive behavior; it does not by itself prove each difference was a bug. [June HTTP routes, especially `land_wave_handler`](https://github.com/loopflowstudio/loopflow/blob/7099cacb1de24e6dfaeb56e3657c1ec01ad8410b/rust/loopflow/src/lfd/http/routes/waves.rs#L1215), [CLI dispatch](https://github.com/loopflowstudio/loopflow/blob/7099cacb1de24e6dfaeb56e3657c1ec01ad8410b/rust/loopflow/src/lf/commands/ops/mod.rs#L1)

Observed call graphs, simplified:

```text
June CLI:      lf op land → CLI context/options → ops::land
June Desktop: HTTP land → Wave/worktree resolution → ops::land
                         → PR/run synchronization → event publication

February run:      HTTP run      → executor → mark Run AND Wave failed
February continue: HTTP continue → executor → mark Run failed
February repair:  both          → shared spawn/finalization function
```

### Inference

Jack's hypothesis is supported, with “engine” defined broadly enough. Moving a shared projection/subscription implementation behind a socket is a reasonable process boundary. Reimplementing Task resolution or lifecycle transitions in the Machine route would repeat the demonstrated failure class even if both paths call shared git/provider utilities.

## What pressures made lfd bigger, and what was genuinely broken?

### Takeaway

The server accumulated execution ownership, automation, persistence, connection/auth and UI synchronization together. Some of that was useful product work; there are also specific durability and operational defects. The history does not support treating all expansion as accidental or all deletion as proof of failure.

### Cited findings

1. **January 28: scheduler and integration authority were present early.** Rust lfd implemented gRPC control, HTTP health/status/metrics/webhooks, SQLite RunStore, scheduling and run orchestration. GitHub webhook advancement with polling fallback and a planned Postgres backend accompanied the Rust migration. This was already much broader than a private-machine transport adapter. [Commit `d99b430532`](https://github.com/loopflowstudio/loopflow/commit/d99b430532d41f2a02ec02f73e556c992cbb6ea4)
2. **February 5: mobile transport also brought hosted discovery.** #274 replaced gRPC with HTTP/WS for mobile compatibility, added Wave actions and events, and introduced Studio registration, heartbeat and cached token validation. A custom relay was future roadmap, not demonstrated shipped behavior. [#274](https://github.com/loopflowstudio/loopflow/pull/274)
3. **February 13: restarting lfd lost running-container ownership.** #331 reports that in-memory Docker tracking made live containers invisible after restart: output routing stopped, runs were blanket-marked failed and orphan containers accumulated. Durable container IDs, startup reattachment and cleanup repaired it. The local executor still treated orphaned runs as failed. This is an ownership/durability problem independent of duplicated command implementations. [#331 / `f7af78c074`](https://github.com/loopflowstudio/loopflow/commit/f7af78c0740deebedb678ec656fca67d39cdf96c)
4. **February 28: long-running resource ownership needed real bounds.** #513 added output TTL pruning, closed output writers on completion/failure, removed reconcile locks when deleting Waves and made migrations transactional. These are concrete fixes, not retrospective complaints about complexity. [#513 / `a40a8dc907`](https://github.com/loopflowstudio/loopflow/commit/a40a8dc9070861db1f305da51fe6b3eaf56d85e2)
5. **June 30: self-hosted deployment displaced hosted assumptions.** #721 removed Studio auth/registration/discovery across Rust, Python and Swift. #740 documents actual Docker Desktop bootstrap friction—launchd PATH, Docker context, credential helpers and slow first builds—and makes native launchd the default private Mac/Tailscale host. Its script builds `lf` and `lfd` together, installs/restarts the service and checks health. [#721](https://github.com/loopflowstudio/loopflow/pull/721), [#740 / landed script](https://github.com/loopflowstudio/loopflow/commit/5af6375e1314a4e10b11d11530feb800f295df77)
6. **July 5: the product's execution model changed.** #801 replaced trigger/cron/watch/recovery/queue subsystems with resident Wave runtime, lifted storage out of `lfd`, and made webhook ingress forward attributed notifications to the Wave mind. “Sprawling subsystems” is the PR author's characterization; the actual deletions demonstrate a substantial change of responsibility, not merely a better network protocol. [#801](https://github.com/loopflowstudio/loopflow/pull/801)

### Inference

Today's separate Session/Graph owners could make a Machine-service restart less consequential than the first daemon's restart, **if that separation is preserved and demonstrated**. Host Tailscale plus native install also reduces the previous deployment/auth combinations. Neither distinction removes launchd environment, retention or upgrade work.

## Didn't the generic argv adapter already solve this before deletion?

### Takeaway

It solved a useful part and established the same direction now proposed. It did **not** finish replacing legacy handlers or unify local and remote clients before lfd was removed eight days later.

### Cited findings

- **July 6: real shared, state-free adapter.** `lf_exec.rs` explicitly describes a host-neutral engine shared by machine lfd and per-Wave listeners: validate argv using the CLI's Clap tree, execute the resolved `lf` binary without a shell, return exit code/stdout/stderr. Nonzero command exit is a successful transport response. Its `stdin` is null and `.output()` waits for completion; this is not an interactive Session transport. [Landed implementation `93960b439d`](https://github.com/loopflowstudio/loopflow/blob/93960b439d3dab0466f8150f47745b18e5fef670/rust/loopflow/src/lfd/lf_exec.rs)
- **The cutover remained unfinished.** Immediately before deletion, HTTP Wave routes still called git/worktree helpers, mutated Wave state and invoked `ops::land`/`next_branch`. Their own TODO said to convert mutation routes to exec argv or remove them: “lfd is the local face, not a hand.” [Pre-removal routes](https://github.com/loopflowstudio/loopflow/blob/be285d5d401fe150ac2f9bdfa884b5dbcc7f6366/rust/loopflow/src/lfd/http/routes/waves.rs#L29)
- **Local and remote reads still differed.** New Swift `RegistryQuery` ran `lf ls/status/runs --json` locally; its source explicitly describes remote SSH as a later seam and clients without a runner falling back to surviving REST reads. Per-Wave SSE carried live motion. There was still a CLI-query/REST-model boundary to maintain. [RegistryQuery](https://github.com/loopflowstudio/loopflow/blob/93960b439d3dab0466f8150f47745b18e5fef670/swift/LoopflowCore/Services/RegistryQuery.swift#L1)
- **Source correction:** #825's PR body says additive, no deletion and no Swift changes. Its landed commit instead changes 64 files, removes the old event bridge/WS machinery, adds Swift RegistryQuery and implements `lfq`. Prefer the landed code; do not repeat the stale body as the final implementation. The actual `lfq` client was tailored to subagent→Wave execution, with per-subagent token and Wave endpoint, rather than a complete phone client. [#825 body](https://github.com/loopflowstudio/loopflow/pull/825), [landed diff](https://github.com/loopflowstudio/loopflow/commit/93960b439d3dab0466f8150f47745b18e5fef670), [lfq source](https://github.com/loopflowstudio/loopflow/blob/93960b439d3dab0466f8150f47745b18e5fef670/rust/loopflow/src/lfq.rs)
- **July 14: removal changed much more than server topology.** #872 removed lfd, the exec door, old migration catalog and substantial Swift state/terminal infrastructure while replacing the product model with Wave/Project/Task and a common child-session layer. It retained a resident Wave server in `lf`. “Old speculative runtime” and “non-functional iOS shell” are contemporaneous author judgments. The deletion was not a controlled comparison showing that a completed thin adapter failed. [#872](https://github.com/loopflowstudio/loopflow/pull/872)

### Inference

What is new now is the opportunity to put **both clients on one existing complete Work implementation**, while retaining independent runtime owners and a sharply smaller phone workflow. Unix sockets, CLI delegation, native launchd, shared Rust and private Tailscale access individually have precedent. Calling the new service thin is not evidence; eliminating the former branch of each migrated operation is.

## Was the mobile effort disproven, and what should this attempt prove first?

### Cited findings

The history contains working slices: #410 built an iOS remote-only Wave list/detail/connection shell; #459 fixed foreground reconnect, stale retry cancellation and cross-client suggested-action state. #615 wrote designs for activity normalization, replay, iOS and relay, not implementations. #686 added accountless pairing and a durable token, explicitly excluding mobile write/chat actions and renewal from that change. The July removal labels the iOS shell non-functional, but the inspected record does not establish whether a complete phone workflow was ever tried and rejected, or identify a particular protocol defect that made it unworkable. [#410](https://github.com/loopflowstudio/loopflow/pull/410), [#459](https://github.com/loopflowstudio/loopflow/pull/459), [#615](https://github.com/loopflowstudio/loopflow/pull/615), [#686](https://github.com/loopflowstudio/loopflow/pull/686), [#872](https://github.com/loopflowstudio/loopflow/pull/872)

### Falsifiable lessons for the current design

- **Complete-operation parity:** launch the same Task action through CLI, Desktop socket and phone transport; verify identical Task/Session attribution, defaults, final state and error semantics. Transport code should not independently settle Tasks or repair runs.
- **Restart independence:** restart/upgrade the Machine service during an active Session; the provider process and identity survive, and both clients reattach. An HTTP health check alone does not prove this.
- **One read/subscription owner:** Desktop and phone consume the same projection and invalidation implementation. Reconnect after missed events must converge without a second client-side lifecycle reducer.
- **Real launchd execution:** on a clean login, verify the service invokes the intended CLI version with working provider/git credentials and paths, not merely that it accepts a socket. Historical PATH/credential friction is documented; a concrete version-skew incident was not found in this review.
- **Bound retained resources:** exercise repeated attach/detach, completion, Task deletion and service restart; assert Session retention policy while output handles, logs and subscriptions remain bounded.
- **Finish the small phone loop early:** existing Task → editable Session → background/reconnect → continue. Prior pairing, models and roadmap breadth did not establish that complete loop.
- **Reserve a narrow future hub boundary:** authenticated webhook ingress may deliver durable events to the existing machine/domain owner. Adding an optional hub should not require another Task execution implementation. This is an inference from the earlier coupling, not a historical implementation claim.

### Gaps

No reliable usage metrics, incident archive, or participant discussion was found in these milestone PRs. We cannot rank duplication as *the* biggest historical cause, infer that all launchd installs were unreliable, or infer that mobile demand disappeared. The evidence supports preserving the desired capability while testing the specific boundaries that previously drifted or lost ownership.
