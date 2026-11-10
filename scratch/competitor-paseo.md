# Paseo: code investigation, 2026-11-07

Inspected [getpaseo/paseo](https://github.com/getpaseo/paseo) at `71439e51a555bfca21dc21e74bcfb39f1eb5e760`, locally `/tmp/lf-research-paseo-20261107`. This is current source, not an exercised released binary. The Loopflow comparison includes LOO-593's working checkout at `5ea4f4e16794417b006e6380c5f6f997a366d08d` plus its uncommitted edits. No app was executed and no performance claims were measured.

## Main findings

Paseo has a reusable remote-agent platform beneath a shared Expo/React Native app. The host owns agents, workspaces, files and provider persistence; desktop, phone, browser and CLI observe/control that host. The closest comparison to Loopflow is its daemon/provider/client boundary, not its hierarchy of product objects: Loopflow puts durable Tasks, Graphs and execution authority at the center; Paseo puts interactive agents and development workspaces at the center.

**Terminal and structured chat coexist, and native conversation history can continue in either, but the verified round trip is manual import/resume/reload—not an atomic switch of the same live process.** That distinction is particularly important when comparing it with Loopflow's explicit attachment ownership.

## 1. Architecture and best code

The codebase is TypeScript, with an Expo/React Native app compiled for native iOS/Android and web, an Electron desktop shell, a host daemon, provider adapters, a framework-independent client SDK, shared protocol schemas, and an optional E2EE relay. App features call the same client boundary instead of implementing a separate mobile backend. [App dependencies](https://github.com/getpaseo/paseo/blob/71439e51a555bfca21dc21e74bcfb39f1eb5e760/packages/app/package.json), [provider contract](https://github.com/getpaseo/paseo/blob/71439e51a555bfca21dc21e74bcfb39f1eb5e760/packages/server/src/server/agent/agent-sdk-types.ts), [daemon client](https://github.com/getpaseo/paseo/blob/71439e51a555bfca21dc21e74bcfb39f1eb5e760/packages/client/src/daemon-client.ts).

The strongest concrete pieces are:

- **Provider-native continuity.** Adapter interfaces distinguish create, resume, import, history, model/mode controls and permissions. Import preserves the provider's native handle and converts provider history into the common timeline. This enables a common mobile interface without making the common UI the sole owner of the native conversation. [Import adapter](https://github.com/getpaseo/paseo/blob/71439e51a555bfca21dc21e74bcfb39f1eb5e760/packages/server/src/server/agent/provider-session-import.ts).
- **Observations have explicit source and teardown ownership.** `SessionDelivery` keys sources by physical socket, creates server-owned subscription IDs, ties cancellation and emissions to that source, and marks an owner inactive before asynchronous teardown. Two tabs or direct/relay paths sharing a logical client ID therefore do not accidentally overwrite each other's subscription. This is more useful than treating reconnect as simply recreating a WebSocket. [Owned subscriptions](https://github.com/getpaseo/paseo/blob/71439e51a555bfca21dc21e74bcfb39f1eb5e760/packages/server/src/server/session/owned-subscriptions/index.ts).
- **Small latest-state synchronization model.** `VersionedCollection` stores the newest value per entity, its sequence, and at most 5,000 deletion tombstones. Expired/out-of-range cursors fall back to a complete snapshot; a daemon generation separates restarts. This supports efficient directory/sidebar recovery without inventing a durable event log for every change. [Collection](https://github.com/getpaseo/paseo/blob/71439e51a555bfca21dc21e74bcfb39f1eb5e760/packages/server/src/server/directory-sync/internal/versioned-collection.ts), [generation owner](https://github.com/getpaseo/paseo/blob/71439e51a555bfca21dc21e74bcfb39f1eb5e760/packages/server/src/server/directory-sync/index.ts).
- **Creation survives client disappearance without pretending uncertain effects are safe to repeat.** `CreationService` persists a request fingerprint and allocated resource IDs before work, records the current stage, and retains `outcomeUnknown` after an interrupted stage. Same key/different request conflicts; uncertain work is returned for reconciliation instead of blindly repeated. This is a useful operation-specific pattern for future mobile mutations. [CreationService](https://github.com/getpaseo/paseo/blob/71439e51a555bfca21dc21e74bcfb39f1eb5e760/packages/server/src/server/creation/index.ts).
- **PTY lifetime is separate from daemon lifetime.** A launcher starts the terminal worker detached, waits for readiness, and exits so the worker leaves the daemon's process tree. The daemon can restart and reattach to surviving PTYs. Retained ANSI screen/history is restored from the worker, not reconstructed by the phone. This resembles the boundary LOO-593 already preserves with existing execution owners. [Worker launch](https://github.com/getpaseo/paseo/blob/71439e51a555bfca21dc21e74bcfb39f1eb5e760/packages/server/src/terminal/terminal-worker-launch.ts), [worker manager](https://github.com/getpaseo/paseo/blob/71439e51a555bfca21dc21e74bcfb39f1eb5e760/packages/server/src/terminal/worker-terminal-manager.ts).

These are distinctive strengths relative to the examined Loopflow interfaces, not claims of exclusivity across all competitors. Paseo also has CLI/SDK/MCP orchestration, schedules and plugins; it should not be described as merely a chat wrapper.

## 2. Mobile ↔ desktop synchronization

```text
desktop / mobile / web / CLI
        │ common client SDK
        │ direct WebSocket OR optional encrypted relay
        ▼
host daemon ── provider adapters ── native agent sessions
        │
        ├── workspace files and Git worktrees
        └── separately surviving terminal worker ── PTYs
```

The devices converge on the same host's state. This is not offline multiwriter synchronization of repository copies or independent agent runtimes. Host-owned state can be viewed from multiple clients; reconnect restores observations and fetches authoritative history/directory state. The host must be reachable for new work. [Transport implementation](https://github.com/getpaseo/paseo/blob/71439e51a555bfca21dc21e74bcfb39f1eb5e760/packages/client/src/daemon-client-relay-e2ee-transport.ts).

Different data gets different recovery:

| Data | Recovery contract |
| --- | --- |
| Project/workspace/agent directory | Latest entity values plus tombstones after a cursor; generation mismatch or expired cursor gives full snapshot. |
| Structured timeline | Epoch/sequence gap detection and paged history catch-up; restored live subscription is explicitly distinct from having recovered missing history. |
| Terminal | Worker screen/history replay plus live output, with revision-aware deduplication and bounded pending bytes. |
| In-flight creation | Persistent request receipt and stage; reconnect can observe it, without replaying uncertain side effects. |

The SDK's timeline subscription literally documents that it restores live delivery while the consumer owns history reads and gap recovery. Client directory transactions fence late responses by both client generation and connection epoch. [Timeline subscription](https://github.com/getpaseo/paseo/blob/71439e51a555bfca21dc21e74bcfb39f1eb5e760/packages/client/src/timeline-subscription/index.ts), [timeline reducer](https://github.com/getpaseo/paseo/blob/71439e51a555bfca21dc21e74bcfb39f1eb5e760/packages/app/src/timeline/session-stream-reducers.ts), [directory transaction](https://github.com/getpaseo/paseo/blob/71439e51a555bfca21dc21e74bcfb39f1eb5e760/packages/app/src/runtime/directory-sync/transaction.ts).

Provider history remains the production transcript authority. The manager retains an optional durable timeline interface for injected stores/tests, but production bootstrap does not inject one and removes obsolete `agent-timelines` data. Do not confuse the optional interface with a production event-sourced timeline database. [Bootstrap](https://github.com/getpaseo/paseo/blob/71439e51a555bfca21dc21e74bcfb39f1eb5e760/packages/server/src/server/bootstrap.ts#L629), [in-memory timeline](https://github.com/getpaseo/paseo/blob/71439e51a555bfca21dc21e74bcfb39f1eb5e760/packages/server/src/server/agent/agent-timeline-store.ts).

Terminal geometry has an explicit claimant: a claim changes the owner, while passive updates from another owner are ignored. This keeps background rendering/refits from repeatedly resizing another device's PTY. [Size ownership](https://github.com/getpaseo/paseo/blob/71439e51a555bfca21dc21e74bcfb39f1eb5e760/packages/server/src/terminal/terminal-size-ownership.ts).

Code sets the physical socket outbound cap to **64 MiB**, despite an older architecture paragraph still saying 8 MiB. It also has an application-owned socket lease of 45 seconds, renewed by activity after the first application ping. A native transport explicitly reports no drain support because React Native's `bufferedAmount` does not reveal its real queued bytes. These details matter when evaluating backpressure claims. [Physical socket](https://github.com/getpaseo/paseo/blob/71439e51a555bfca21dc21e74bcfb39f1eb5e760/packages/server/src/server/websocket/physical-socket.ts), [native transport](https://github.com/getpaseo/paseo/blob/71439e51a555bfca21dc21e74bcfb39f1eb5e760/packages/client/src/daemon-client-websocket-transport.ts#L42).

Relay security is implemented at the endpoints, not merely described as opaque forwarding. Updated peers use Noise NK with authenticated records; client pins prevent downgrade after protected mode is established. However, legacy mode remains admitted by default unless the daemon's protected-security requirement is enabled. The security documentation explicitly calls out replay risk for legacy recordings. Thus “E2EE relay” should not be inflated into “every supported connection has replay-safe modern security.” [Secure channel](https://github.com/getpaseo/paseo/blob/71439e51a555bfca21dc21e74bcfb39f1eb5e760/packages/relay/src/secure-channel.ts), [security contract and limitation](https://github.com/getpaseo/paseo/blob/71439e51a555bfca21dc21e74bcfb39f1eb5e760/docs/relay-endpoint-security.md), [default false](https://github.com/getpaseo/paseo/blob/71439e51a555bfca21dc21e74bcfb39f1eb5e760/packages/server/src/server/config.ts#L341).

## 3. What users like, and limits of that evidence

The best firsthand evidence found is a Reddit comment by `cakedayy`, reporting roughly a month of OpenCode usage: praise for a cohesive, smooth interface and the relay saving connection setup time. The same comment criticizes long voice replies timing out, interrupting speech and showing poorly formatted text. This supports smooth interaction and connection convenience; it does not justify calling every voice feature universally loved. [Firsthand comment in the launch thread](https://www.reddit.com/r/ClaudeCode/comments/1s4tml6/i_built_a_super_polished_mobile_app_for_claude/).

Another user, `Street-Preference-88`, reports choosing Paseo after trying other mobile workflows, but specifically complains about the number of taps needed to switch agents/models. The creator acknowledges that friction. This is counterevidence to a blanket claim that provider switching is effortless. [Mobile workflow discussion](https://www.reddit.com/r/opencodeCLI/comments/1tg8dai/remote_mobile_coding_opencode_web_openchamber/).

The product's notable implemented UX features include native phone access to agents/files/terminals/diffs, split panes, provider choice, local-first dictation/voice, and reusable tools for orchestration. These are capabilities; the evidence above is what supports actual user affection. Public feedback sampled here is anecdotal and some months older than current main.

## 4. Terminal, chat and the same conversation

**Both UI surfaces:** yes. Structured agent chat and ordinary terminal panels can coexist within the workspace. They are distinct execution surfaces; the terminal panel is not automatically a native-agent TUI rendering of the selected structured chat.

**CLI → chat:** yes for providers with session-listing/import adapters. The import UI chooses a host and native session. `import-sessions.ts` serializes imports by provider/native handle, refuses an already-active imported record, and restores archived records rather than creating duplicates. The adapter resumes the same native `sessionId`/`nativeHandle` and streams provider history. It is import/resume, not a transcript-only fork. [Import coordination](https://github.com/getpaseo/paseo/blob/71439e51a555bfca21dc21e74bcfb39f1eb5e760/packages/server/src/server/agent/import-sessions.ts), [common provider import](https://github.com/getpaseo/paseo/blob/71439e51a555bfca21dc21e74bcfb39f1eb5e760/packages/server/src/server/agent/provider-session-import.ts), [import UI](https://github.com/getpaseo/paseo/blob/71439e51a555bfca21dc21e74bcfb39f1eb5e760/packages/app/src/hooks/use-import-session.tsx).

**Chat → native CLI:** the tab menu has “Copy resume command.” It copies a command using `runtimeInfo.sessionId` or persisted native session ID. Templates include `codex resume`, `claude --resume`, `pi --session`, `omp --session`, `opencode --session` and `hermes --resume`. No fork flag is inserted. Crucially, the handler only writes the clipboard; it does not stop the Paseo runtime, grant a terminal ownership token or perform a process handoff. [Copy handler](https://github.com/getpaseo/paseo/blob/71439e51a555bfca21dc21e74bcfb39f1eb5e760/packages/app/src/screens/workspace/workspace-screen.tsx#L2156), [templates](https://github.com/getpaseo/paseo/blob/71439e51a555bfca21dc21e74bcfb39f1eb5e760/packages/app/src/utils/provider-command-templates.ts).

**CLI → existing Paseo chat again:** “Reload agent” calls `refreshAgent`; the host interrupts an active Paseo run, closes its existing provider session before opening the replacement, resumes the same persisted handle, wipes the cached timeline into a new epoch, and rehydrates provider history. The source comment explicitly names changes made outside Paseo as the reason for this action. The app sends the old cursor so the changed epoch triggers a full reset. [Reload UI](https://github.com/getpaseo/paseo/blob/71439e51a555bfca21dc21e74bcfb39f1eb5e760/packages/app/src/screens/workspace/workspace-screen.tsx#L2188), [refresh handler](https://github.com/getpaseo/paseo/blob/71439e51a555bfca21dc21e74bcfb39f1eb5e760/packages/server/src/server/session.ts#L4996), [reload lifecycle](https://github.com/getpaseo/paseo/blob/71439e51a555bfca21dc21e74bcfb39f1eb5e760/packages/server/src/server/agent/agent-manager.ts#L1743).

**Concurrent writers:** the manager's reload explicitly closes its prior runtime because the persisted thread must have one writer, but the copied external resume command is outside this boundary. I found no cross-process takeover fence coordinating that external CLI with the retained Paseo session. Do not promise simultaneous chat and native-TUI mutation of the same session, automatic synchronization of arbitrary external CLI changes, or identical behavior across every provider. Sequential same-conversation continuation is supported by the inspected paths; atomic live switching is not established.

## Lessons appropriate for LOO-593

1. **Keep the service outside execution lifetime.** Paseo's independent terminal worker corroborates 593's choice to bridge existing owners. A phone reconnect or control-service restart should not spawn another provider or retire the old one.
2. **If full Work readings later become too large for mobile, use latest-state deltas with bounded tombstones and generation reset before adopting a universal durable event log.** This is a future optimization, not a defect in 593's current complete-reading contract.
3. **Differentiate live subscription restoration from state synchronization.** A connected socket is not evidence the timeline or terminal has caught up. Fence cached responses by connection generation; install snapshot and cursor together.
4. **Keep accepted mutation execution independent from subscription/socket cancellation.** Durable creation receipts demonstrate a useful future mobile write design, while retaining explicit unknown outcomes at crash boundaries. They do not deliver general exactly-once execution.
5. **Carry subscription authority with the physical source.** Two screens with the same logical client identity can legitimately coexist. Releasing one observation must not release a sibling, and an asynchronous teardown must not emit after release.
6. **Preserve explicit geometry ownership and distinguish resume, fork and handoff.** Paseo's manual same-ID continuation shows why returning the native identity alone is not an ownership transfer contract.
7. **Remote admission is separate work.** 593's socket is same-user/local and explicitly not forwardable. Future remote transport needs authenticated device admission and endpoint replay/downgrade rules; copying a relay URL around the existing socket would omit those guarantees.

No recommendation above requires replacing Loopflow's domain model or its existing terminal protocol. The immediate server memory-budget concern is better illustrated by the T3 investigation; Paseo contributes precise observation, recovery and receipt semantics.
