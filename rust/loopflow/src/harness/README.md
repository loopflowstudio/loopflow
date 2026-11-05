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
`logs/lf.log` (a `.store-gap` marker beside the delivery file, cleared when a
later boundary records again). The turn or tool completes without it. The PTY
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
