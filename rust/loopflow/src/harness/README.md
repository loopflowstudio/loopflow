# Harnesses

Each provider (`claude.rs`, `codex.rs`, `opencode.rs`, `pi.rs`) gets launched,
observed and recorded here. The provider owns the conversation; lf adds context
at turn boundaries, interjects through hooks and records what happened.

`AgentExecution` is the invocation's shared lifecycle handle. Native launch and
headless respawn use the same attachment and settlement state; dispatch and
history retain their own immutable attachment snapshots. Final settlement belongs
to the shared state itself, including after a recording panic. Provider identity in
memory advances before best-effort history writes, so a recording failure cannot
change which account or thread a retry continues. A later observation retries an
unrecorded identity; successful repeated observations add no duplicate facts.

The shared state lives in `session_record/runtime.rs`: it owns the current
provider, account, thread and attachment. Its `SessionRecorder` keeps manifests,
receipts and history in `session_record.rs`, with no attachment or process-control
operations. Both remain under the same lock so launch, retry and settlement stay
serialized. The execution retains the store that admitted its attachment; launch,
retry and exit use that owner directly instead of reopening through the recording
directory. Completion runs attachment cleanup before recording its receipt.
A failed receipt write remains a caller error; retrying can record the same
outcome but cannot reopen the execution or change its result.

## lf never breaks the conversation

lf is a session manager, context adder and hook interjector on top of a harness,
never a middleman that can get in the way (Jack Heart, 2026-11-05). It is never in
the byte path between a person and their provider except as a pass-through that
keeps passing when lf's own state is unavailable: a store that is locked, refused
by its reader floor, missing a column or gone; a hook that fails; a lock that
cannot be read.

Every lf invocation at a turn or tool boundary is best-effort. `__context-block`
from Claude hooks, the OpenCode plugin and the Pi extension opens the store with
a one-second busy timeout; on any store error it emits the context block without
steers, exits 0 and records the gap once per native conversation in
`logs/lf.log`. The turn or tool completes without it. A recording gap is one
concept, `store::RecordingGap`: one log line per changed failure and one when the
store answers again, whether the gap lives in an owner's or observer's memory or,
for hooks that are a fresh process at every boundary, in a `.store-gap` marker
beside the delivery file. When a hook reaches the store again it writes the gap
into the Session's history as one `store-gap` observation (first and last miss)
before clearing the marker, so a turn lf never saw reads as unknown rather than
as still running. Work that can outlast a hook's timeout runs beside it: Claude's
`StopFailure` hook starts `--moment record-limit` in its own OS session and exits,
because a kill between a token reply and the profile write would lose a rotated
grant. The Pi extension and OpenCode plugin behave like Claude's
hooks: an lf they cannot run goes to their own log (`logs/pi-extension.log`,
OpenCode's stderr beside the capture), never to the conversation. The PTY
owner delivers typed input even when it cannot record it, and holds the Session
in memory instead. Quitting returns the provider's own exit within a second; exit
records finish in the background or are lost. Nothing about lf's own state ends a
conversation either: a removed, rebuilt or superseded lf executable leaves its
owner serving until the person quits, resumes elsewhere or an operator rolls over.

Codex (`codex_connection.rs`) has no relay: the Codex TUI connects straight to
its private app-server, and lf's observer subscribes beside it to record turns
and deliver Task updates. A store the observer cannot open is a recording gap it
retries; history fills it on the next launch.
