# v0.12.8

<!-- loopflow:release-notes=narrative;gate=safe -->

v0.12.8 makes active work easier to trust while it is still running. Provider usage now has one live accounting path across the CLI and Mac app, blocking human interventions survive lost presentation surfaces, and the Podium turns selected Work into a direct control surface. The same release tightens the handoffs around provider login, Discord ownership, and immutable release downloads.

## See live provider work without double-counting

Usage is now derived from cumulative provider checkpoints rather than separate per-surface estimates. Operators can inspect output, cache activity, reasoning, cost, context pressure, and Work attribution through a shared snapshot while a Turn is active, without missing measurements being presented as zero or provisional readings being counted again when the Turn completes (#1166).

- `lf usage` reports fixed 5-second, 5-minute, 1-hour, and 24-hour windows across global, repository, Wave, Project, Task, Exec, and Invocation scopes.
- `lf ps` and `lf top` embed the same canonical snapshot used by the Mac Podium and telemetry dashboard.
- Claude and Codex emit cumulative checkpoints during a Turn; SQLite retains provisional and final receipts and backfills existing Turn usage through the schema migration.
- Headless Codex launches use the app-server harness so session identity and streamed usage remain connected.
- The Mac activity surfaces refresh live state every two seconds instead of waiting for completed-Turn spend.

## Intervene without losing the work

An Ask is now a durable session rather than a turn-local prompt. A provider exit, closed shell, or failed presentation no longer erases a blocking decision, and waiting for a person does not consume provider turns (#1175).

- `lf ask --user`, `lf ask list --user`, and `lf ask open <ask-id>` create, inspect, and resume the User-attention queue.
- Ask sessions carry explicit origin, target, state, result, Invocation ownership, and presentation fences through claim, resolution, decline, release, escalation, or cancellation.
- `human: true` flow nodes wait through the durable Ask lifecycle; direct TTY runs remain human-present.
- Loopflow.app can present attention in embedded Ghostty or an external terminal while sharing the same authorized session lifecycle as the CLI.
- `lf --as task|project|wave:<name>` runs a skill as existing Work, and the focused `design` and `launch-plan` flows preserve a reviewed design while launching independent follow-up Tasks.

## Control selected Work from the Podium

The permanent Wave Score sidebar has been replaced by a path-based console that gives selected Work the full center surface. Navigation remains visible through temporary Wave → Project → Task drawers, while faders combine runtime signal and legal lifecycle actions (#1195).

- Fader color shows runtime state, fill shows five-minute output, and pressing a fader starts or stops Wave and Task agents through existing `lf` lifecycle commands.
- Wave, Project, and Task selections each receive a dedicated surface; the unselected view remains focused on cross-Wave NOW triage.
- Repository scope now lives in the Podium bar, and Activity follows the selected node.

## Connections and listeners recover cleanly

Provider authorization and Discord startup now favor a hands-free happy path without weakening their recovery and ownership fences (#1196, #1197).

- `lf auth connect claude` waits for the browser callback and pasted-code fallback concurrently, allows up to ten minutes for interactive OAuth, and exits promptly once either path succeeds.
- A Discord binding starts only when its configured Home, durable Wave placement, and local Home agree; this prevents copied repositories or stale placement from creating competing listeners.
- Discord restart reconciliation distinguishes repeated delivery parts, so an earlier identical provider echo cannot confirm a later part.
- Chat parsing is isolated from unrelated Wave policy, while unknown Discord binding fields are rejected instead of ignored.

## Operational notes

- Bare `lf` now opens the terminal control conversation. Launch the Mac app with `lf desktop` (#1175).
- Before starting a Discord listener, set `chat.home_id` to the Wave’s placed Home. Ownership is validated before Loopflow reads the Discord token or contacts the provider (#1197).
- Existing usage data is migrated into the checkpoint ledger and backfilled from prior Turn usage (#1166).

## Small changes

- `scripts/install.py refresh` resolves the latest release tag before downloading either `SHA256SUMS` or `install.sh`, then fetches both from that immutable tag. Signed release-asset CDN redirects can no longer be mistaken for release tags (#1189).