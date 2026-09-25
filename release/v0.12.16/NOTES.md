# v0.12.16

<!-- loopflow:release-notes=narrative;gate=safe -->

v0.12.16 makes human interaction a durable part of Loopflow's execution model. Sessions preserve the exact conversation and resolution boundary when a provider, terminal, app, or machine-control process exits, while Wave chat acknowledges messages promptly and delivers one deliberate response instead of a controller transcript. Operators can now resume human work or steer a running Task without confusing process liveness with approval.

## Resume human work without advancing it by accident

Readiness, process exit, and human approval are now separate facts represented by one Session projection. Interactive Runs, ad-hoc human Asks, and Task human FlowSteps remain available across restarts until a person performs the resolution action valid for that kind of Session.

- `lf session` lists and opens resumable Sessions and exposes readiness, completion, approval, and iteration as distinct actions.
- Provider exit or terminal pane closure leaves the Session resumable with its provider-native history; it no longer settles the human boundary.
- Interactive and Ask Sessions finish through Complete, while Task FlowSteps advance only through Approve or Iterate.
- Task controllers persist their Flow position and use semantic conditions around a single active Task branch and PR, so automation cannot advance merely because its process disappeared.
- The macOS Sessions surface consumes the same Session DTO, attaches native terminals, and presents only the actions valid for each Session kind.
- Desktop control follows the machine-selected Home, copied runtime binaries are authenticated by digest, and Work-scoped usage can be filtered against the same durable identity.

## Use Wave chat as an operator surface

Discord-backed Waves now treat conversation as durable channel history rather than a queue to consume. The Wave sees authorship, reply relationships, and its own previous posts, allowing it to decide whether a response is warranted and preventing listener restarts from producing duplicate replies.

- Incoming messages receive an immediate pickup reaction, followed by one completed response and a success reaction.
- A resumable Discord Gateway connection, REST cursor catch-up, and delivery reconciliation preserve continuity across listener restarts.
- The shared `wave/chat` contract and `lf reply` keep only the finished reply on the channel; intermediate work remains inside the Run.
- Wave and Project governance use focused `operate` turns, keeping chat responses deliberate instead of streaming controller activity.

## Steer long-running work while it is live

Direction now travels through durable Task and Project event streams and can also reach the active provider session at its next turn boundary. This keeps a correction useful whether the process consumes it immediately or resumes later.

- `lf task steer <task> "take the smaller approach"` sends durable direction to running work.
- `lf task interrupt <task>` ends the current turn so execution can restart with that direction.
- Codex, Claude, and OpenCode harnesses share live current-session delivery support.
- The standalone steers table and transient consumption model are gone; interruption state and direction live with the Work they affect.

## Operational notes

- Discord bindings now require **Add Reactions** and **Message Content** permissions.
- The legacy Ask tables, Ask commands, attention DTOs, and standalone steers table were removed rather than kept behind compatibility shims. Consumers must move to Sessions and Work event streams.
- Provider-native Claude and OpenCode steering checks remain opt-in; deterministic coverage exercises the shared controller contract.
- A dedicated responder process and Discord thread-to-Task mapping are not included in this release. Wave replies still run through the existing resident.
- Settled Task PRs remain as serial delivery history even though each Task now has only one active branch and PR.

## Small changes

- Temporary SQLite stores are hermetic, preventing migration tests from racing ambient machine state.
- Installation promotion avoids version-skew re-exec loops and ignores stale placements whose catalog roots no longer exist.
- Linked worktrees resolve to their main checkout during repository discovery.