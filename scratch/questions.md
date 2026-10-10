# Open choices — LOO-443

2026-10-09 implementation choice: fresh opaque attachment tokens fence each
claim and release, including A → B → A. This resolves the repeated-ID ambiguity
without another process identity or lifecycle. The single-table AgentProcess cut
now owns the token and attached LfProcess on that record. No approval beyond Jack Heart's original
Task directive is inferred.

2026-10-09: Source OpenCode launches are one-server/one-Session. Historical
shared PID/birth rows must remain recoverable and non-signallable until exact
ownership is resolved; source inventory is not an audit of configured data.

2026-10-09 implementation choice: native request attribution freezes before
sending rather than on a delayed Started observation. A later bind affects later
requests, not already-submitted work. Per-request correlation—not AgentProcess's
original capture—retains this snapshot across live takeover and multiple turns.
Crash-lost correlation remains unknown; this introduces no recovery replay or
new execution owner.

2026-10-09 implementation choice: Claude replacement is owned by the existing
capture, with an exact expected snapshot. A reserved record is usable; only
observed exit/spawn failure permits a fresh identity. Capture settlement and the
next spawn advance together; pending dispatch/history never consult the mutable
owner. Current source implements this for captured Claude; the focused fixture waits for
provider-written input before interrupting, not merely successful pipe dispatch.
Captured native launch now follows the same replacement rule, retaining an exact
wait snapshot and pre-exec recording without headless process-group setup. Native
foreground orphan cleanup, other providers and optional launches remain open.
Invocation retry now also settles/replaces the record before advancing metadata;
account failover selects a fresh native thread, while same-account retry retains it.
This extends the existing exact-owner rule, not generic harness-stop authority.
The native PTY fixture covers inherited descriptors/group, not configured terminal
interaction. Focused runtime results are recorded in the plan.

2026-10-09 implementation choice: headless admission is refused at launch rather
than typed into `Harness::start`. Every production start already carried a claimed
attachment; the optional config field remains because configs are prepared before
admission. Moving the attachment into the start signature stays open.

2026-10-09 assumption: Jack Heart's steer to publish supersedes the plan's earlier
"no partial publication" note. #1519 is published for review with the remaining
lifecycle cuts listed in the plan; it is not presented as complete or landable.
Whether those cuts land in #1519 or a following PR of this Task is Jack's choice
and is not decided here.

2026-10-09 implementation choice: native commands carry the owner's exact
attachment in memory, never serialized into stable tool provenance. The client
relay and provider endpoint are separate paths. Connection exit/interruption
records its attachment outcome without asserting provider death or invoking
provider close; independent orphan settlement still owns detached providers.

2026-10-09 implementation choice: the generation cut keeps the released
`session_events.provider_generation` and `processes.caller_provider_generation`
columns as unread history instead of dropping them. Earlier generations have no
AgentProcess to map to, and dropping the event column rewrites the store's
largest table. Nothing reads or writes them; dropping later is one statement.
Whether to delete that history outright is Jack's choice.

2026-10-09 implementation choice: `AgentCaller.agent_process_lfid` is optional
so a provider launched before the upgrade keeps issuing commands as a stale
caller instead of failing every `lf` call on an unparseable environment. New
launches always carry it. Refusing old environments outright is the alternative.
