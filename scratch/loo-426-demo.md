# LOO-426 demo — `lf open` into cmux (October 19, 2026)

Design: [loo-426-open-any-task.md](loo-426-open-any-task.md). Walkthrough:
[pr-review.html](pr-review.html). Branch head at demo: `ab17908fbf`.

## Setup and why it is a throwaway Home

A dev build pointed at `~/.lf` hands every command to the installed release
(`installation::dispatch_default_cli`), so this branch's `lf open` only runs
with a private `LF_HOME`. Jack Heart chose "fix, then throwaway demo": a scratch
repository `demo-426` and private Home under the session scratchpad, opened from
a cmux pane with `LF_HOME=… target/debug/lf open`.

Found and fixed before the demo (`ab17908fbf`): cmux, Herdr and new terminal
windows start the connect command from the host's own environment, so a
private Home's opening connected against the main Home. The typed command now
carries `env LF_HOME=…`; `cmux_opens_the_task_conversation_through_session_connect`
asserts it.

## Observed

- Jack Heart: "i think its mostly working but its constantly flickering"
  (screenshot: a Codex repository conversation, renamed "Repository
  conversation", working in `demo-426.repo`, ready for input).
- Process tree (interpretation from `ps` and the private Home's `processes`):
  `lf session connect` delegated to relay A; the new conversation's `lf -i skill
  repo-session` delegated again to relay B. Codex output reaches cmux through
  two terminal relays. That nesting is in `human_session.rs`/`transport.rs`,
  unchanged by this branch.
- Idle Codex screen, observed through each relay with `transport attach
  --observe` for 6–12 s: ~20–24 KB/s, ~16 synchronized-update frames/s, about
  three of them full-screen (5.5 KB) repaints of the whole transcript. Relay A
  and relay B carry the same rate, so the second relay does not amplify it.
- Both relay PTYs held a constant 60×152; the stream contains no terminal
  queries (DSR, DA, OSC colour, DECRQM, focus mode, title). So neither resize
  churn nor a query loop explains the repainting.
- After the workspace was closed, both relays and Codex kept running (the
  conversation persists detached).

## Cause found and fixed (October 19)

Jack Heart: "my normal codex sessions dont flicker, but they are currently
stuck on Reconnecting". He chose to fix the flicker inside LOO-426.

- The relay's own diagnostics (`session_events`, source `codex_relay`) named
  it: `provider_to_relay: read: Message too long: 17270917 > 16777216`.
- Probing the demo app-server: `plugin/list` returns 17.3 MB (7,366
  marketplace plugins); `app/list/updated` is 6.7 MB. The Codex TUI requests
  plugin/list on every connect; tungstenite's 16 MiB default refused it, the
  relay dropped the connection, Codex reconnected into the same refusal. The
  full repaints alternated the idle prompt with "Reconnecting… (0s)". Jack's
  long-running sessions show the same failure after retries ran out ("Reconnect
  failed — check the endpoint, then relaunch").
- Superseded: the double relay was not the cause (both relays carried the same
  rate). It is removed anyway (`8ae04b4240`): a delegated `lf session connect`
  now passes its transport handoff to the conversation it launches.
- Fix `b90e902f2e`: `codex_connection::connect_provider` lifts the message and
  frame limits for the local app-server; used by the relay, history recovery,
  close inspection and the headless harness. Test
  `relay_carries_provider_messages_beyond_sixteen_mebibytes`.
- Proof: fresh conversation, idle 10 s after the first screen — 0 frames (was
  ~13/s with ~3 full repaints/s), 0 relay failures. Codex relay tests (11),
  `transport_tests`, `session_lifecycle_tests`, `session_launch_contract`,
  `process_ownership_tests`, clippy: pass.
- Jack's installed `lf` has the 16 MiB limit, so Codex conversations under it
  keep failing until this ships.

## Open

- Jack repeats the cmux demo with the rebuilt binary: no flicker; reopen while
  live attaches the same conversation without a second Codex; sidebar name.
- Desktop opening, Herdr, clicks and the four-conversation parity remain
  unproved (installed Desktop is the main build).

## Recommended next action

Jack reruns the demo commands. If clean, the Task moves on to `ship`; the
Codex relay fix is the urgent part, since every installed-`lf` Codex
conversation is affected.
