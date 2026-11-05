# Harnesses

Each provider (`claude.rs`, `codex.rs`, `opencode.rs`, `pi.rs`) gets launched,
observed and recorded here. The provider owns the conversation; lf adds context
at turn boundaries, interjects through hooks and records what happened.

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
store answers again, whether the gap lives in an owner's or relay's memory or,
for hooks that are a fresh process at every boundary, in a `.store-gap` marker
beside the delivery file. When a hook reaches the store again it writes the gap
into the Session's history as one `store-gap` observation (first and last miss)
before clearing the marker, so a turn lf never saw reads as unknown rather than
as still running. The Pi extension and OpenCode plugin behave like Claude's
hooks: an lf they cannot run goes to their own log (`logs/pi-extension.log`,
OpenCode's stderr beside the capture), never to the conversation. The PTY
owner delivers typed input even when it cannot record it, and holds the Session
in memory instead. Quitting returns the provider's own exit within a second; exit
records finish in the background or are lost. Nothing about lf's own state ends a
conversation either: a removed, rebuilt or superseded lf executable leaves its
owner serving until the person quits, resumes elsewhere or an operator rolls over.

The Codex relay (`codex_connection.rs`) is the one remaining in-path piece: the
Codex TUI reaches its app-server through lf. It accepts clients before touching
the store, continues through every store failure except a lost attachment, and
dispatches a person's turn unfenced when the fence cannot be read. Only the saved
initial input still waits for its receipt. Removing the relay is a separate Task.
