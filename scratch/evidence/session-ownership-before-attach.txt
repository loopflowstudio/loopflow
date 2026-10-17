# Session ownership, lfdb and provider continuity

October 16, 2026. Research requested by Jack Heart in the LOO-416 conversation.
Jack Heart subsequently accepted this architectural direction. Implementation
mechanisms and unresolved restoration limits still need proof. Source baseline: `70f6a628e2`.

## Finding

No evidence found that Session continuity requires one shared resident server.
It requires an owner whose lifetime exceeds all viewers, including for shells.
An on-demand lf process per conversation is a viable candidate. That owner must
retain provider control, drain output, and serve attachments while no viewer exists.
A background shell alone does not supply those responsibilities.

A shell owner is an ordinary recorded lf Process associated with its durable pane
identity. It need not create an agent conversation or pretend to be an AgentSession.

The difficult independent choice is terminal-state ownership. A transparent PTY
relay plus repaint does not establish exact scrollback or terminal-mode recovery.
Library-backed headless terminal state is a candidate, but would extend LOO-415's
current transparent-relay design. It needs explicit reconciliation, not a second
holder implemented in LOO-416. No shared service, new schema or provider was launched.

## Research evidence

The [full research record](evidence/session-ownership-research-full.txt) retains
Rex primary-source links, limits, the zmx comparison and the shell probe.
Rex documents a shared server/client boundary; no public implementation or local
Rex runtime was inspected. zmx documents a per-session daemon/socket with terminal
snapshots, supporting the alternate topology without proving exact restoration.
The disposable PTY probe preserved bash PID, variable and cwd after launcher/viewer
exit, completed detached foreground work and drained 2 MiB. It did not test full
terminal state or providers. Shared-server necessity is unsupported; terminal
restoration, endpoint cost and executable-version handling remain to be proved.

## State ownership proposal

| State | Authority and persistence |
|---|---|
| lf Session identity, Work binding, title, primary selection, completion | Existing lfdb Session records; completion is separate from provider/viewer exit. |
| Provider conversation/thread selection, provider/account route and native history locator | lfdb references opaque native identity and retains historical selections; provider owns its native conversation. Never recreate history by scraping terminal text. |
| Owning lf Process and AgentProcess, Machine/path, PID with OS birth, endpoint, exit facts | Existing process records in lfdb. Endpoint/PID is evidence requiring current validation, not a durable claim of life. |
| Provider attachment and viewer control | Existing lfdb attachment fences the executor/provider. The October 17 implementation gives viewers separate fresh runtime tokens; passive viewing takes none. A → B → A fences stale viewer input without transferring provider authority. |
| Captured input, provider turn IDs, observed outputs, pending decisions and outcomes | Existing Session event/capture owners. Persist meaningful boundaries and delivery uncertainty, not a second writable provider transcript. |
| Work layout, pane membership, hidden/open set, zoom and selection | Candidate: one shared Work-keyed layout record accessible through Rust/lfdb and CLI/Desktop. Supersedes the draft's assumption that an app-only file is settled. Machine replication remains a separate policy. |
| Shell heap, cwd/environment/jobs, PTY file descriptors, provider sockets | Live per-Session/per-shell owner; cannot be restored by serializing a Session row. Native provider history is recovery after loss, not live continuation. |
| Terminal parser/grid/modes and bounded scrollback | Runtime owner plus optional checkpoint/history artifacts with explicit offsets. Keep byte-rate state off shared SQLite's write path; database stores locator/generation if needed. Reuse capture storage where semantics fit. |
| Draft text | Native TUI drafts remain in the surviving process. App-owned composers/file drafts need durable recovery text keyed to their existing owner. Client IME composition has separate limits; never restore by submitting. |

Multiple lf processes already use SQLite and per-Session fences. Shared durable
metadata does not imply a resident database service or a global execution owner.
Lifecycle writes should be short transactions, with no transaction held across
provider/network I/O. Rendering caches are derived. Live provider authority must
not move when a tile mounts, a viewer reconnects or a layout snapshot is restored.

Current source: `session.rs` (`LfSession`, `SessionEvent`), `process.rs`
(`AgentProcess`, `SessionAttachment`), `store/sqlite/processes.rs`
(`session_connection`, `claim_session_attachment`, `record_session_connection`).

## Provider interface: preserve the strongest available continuity

The long-lived owner keeps the provider adapter, its stream and in-flight turn.
Clients talk to that owner. Existing `Harness` already separates send, steer,
interrupt and stop; extend concrete owners rather than invent a provider factory.
The external contract should distinguish:

- Attach/observe existing execution, with a cursor/snapshot sufficient to join
  without an output gap. No prompt submission and no provider replacement.
- Submit input or answer a pending provider request under current control. Retain
  request/turn identity and delivery result; unknown acknowledgement never causes
  automatic resend. Local IDs cannot promise provider-side exactly-once delivery.
- Interrupt the current turn, explicitly reporting whether that provider keeps
  its process. Stop execution and resume saved native history remain distinct.
- Detach one viewer; keep execution, unanswered requests and output consumption.
- Resume history only after execution is gone and deliberately selected; report
  the lost continuity. A new provider process is not the same running turn.

| Adapter at the source baseline | Evidence and implication |
|---|---|
| Codex | `harness/codex.rs` and `human_session::connect_live_codex` use the existing engine endpoint/native thread. `os_process/lifeline.rs` kills the provider group when its last lf holder exits. Keep a durable owner holding that lifeline; a live socket alone is insufficient. |
| Claude | `harness/claude.rs` keeps a persistent stream-json process across turns. Current interrupt kills it and later input resumes native history. The generic Harness comments were corrected during October 16 reconciliation; runtime behavior is unchanged. Preserve the stream owner across UI detach. |
| OpenCode | Current adapter owns an HTTP/SSE server and native session. Keeping its owner alive can retain that channel; the checkout's native-open route currently invokes stop-and-resume for headless work. Do not claim live native-client takeover without testing the provider path. |
| Native terminal providers and shells | A surviving PTY owner preserves the actual process and TUI. A structured headless provider cannot be transformed into its native TUI by replaying text; either attach through a provider-supported protocol or retain the native TUI from launch. |

Automatic restoration must replace `human_session::stop_headless_for_resume` as
its route, while retaining explicit recovery behavior where intended. One stable
provider owner and optional viewer input claims must remain distinct; duplicate
provider authority would violate Session ownership.

## Accepted direction and proof still needed

Accepted direction: per-Session lf owners, including a shell PTY owner where
shell retention is wanted; shared durable metadata in lfdb; client-independent
provider adapters. Keep a shared on-demand server as a comparator if measurements
show endpoint/process cost or cross-Session coordination dominates. Current evidence
does not justify abandoning the serverless direction.

Prove one Session through: submit once, remove every viewer, observe continued
execution, reopen, answer the original pending request, and verify native thread,
turn and process identity. Then test owner crash separately: retain honest history
and uncertain delivery rather than replaying work. For terminals add alternate
screen, scrollback, resize, keyboard modes, query responses and detached output.
For scale use twelve actual live Tasks and measure total owner+viewer memory,
CPU, reconnect latency and SQLite contention. Shell fallback-to-cwd is not a
substitute for the continuity now under investigation.

Jack next requested a historical audit of loading, stalled selection and forced
restarts: [failure analysis](session-continuity-failures.md). Jack also wants a
pleasurable, deliberate fresh-context restart later. Reliability comes first;
preserving continuity must not make old context compulsory. Context transfer and
successor interaction remain downstream design work.
