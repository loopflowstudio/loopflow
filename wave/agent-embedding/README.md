# Agent Embedding

## Vision

Concerto becomes the conductor, not the chat client. The repo window opens on an attention queue that shows what needs human judgment across waves, while coding sessions themselves stay in a real terminal. Concerto's value is everything around the terminal: attention, portfolio awareness, calibration, worktree and PR lifecycle, and native compositions of those surfaces.

Not transcription, not a chat-shell wrapper. The agent runs in a terminal. Concerto makes parallel agent work legible and steerable.

## Strategy

## Execution model

The daemon should stay **parallel-first**. A wave is configuration and grouping; a run is execution. `primary_flow` is the wave default, while each `WaveRun` owns its actual `snapshot.flow`, worktree, branch, status, and any interactive terminal session. Reactive work like CI-fix, repo triggers, cron, and listener activations should compose as additional runs, not as special cases that mutate the wave's identity.

The execution path should also converge on the real CLI. `lfd` still decides when runs start, but it should start them by forking normal `lf <flow-or-step>` commands in the correct worktree and executor environment instead of carrying a second bespoke executor forever. Interactive clients should attach to daemon-owned shells or PTYs — SSH-like in product feel whether or not the first transport is literal SSH — and run the same `lf` commands there. In that environment, `lf` should detect that it is running under `lfd` and emit structured lifecycle events so the daemon observes execution instead of scraping terminal output or receiving one-off launch-spec requests.

Concerto can still present a calmer singular surface: one selected wave, one foreground run, one presented terminal. That is a product policy, not an infrastructure limit. Serialized waves remain useful where roadmap UX wants one thing at a time, but the base model should allow many runs and worktrees per wave.

Keep one durable model per concept. `AttentionItem` remains the shared contract for human checkpoints, `WaveRun` remains the execution unit, and `TerminalSession` remains the shared contract for embedded coding sessions. Portfolio, lifecycle, calibration, and composition work should derive from those existing types plus wave/run data instead of inventing dashboard-only state.

The local-first terminal workspace is now in place: `lfd` persists `terminal_sessions`, emits terminal-session events, and Concerto embeds tracked Ghostty tabs keyed by terminal-session ID while keeping Work as the default surface. That gives the queue, workspace, and future portfolio views one shared state model for interactive runs instead of a separate Swift-only terminal stack.

But the transport is still transitional. `attach` currently returns a local wrapped shell command built from agent argv, completion still depends on callback POSTs, and the terminal does not yet show a daemon-owned `lf <step-or-flow>` PTY. The next risky infrastructure step is to replace that shim with a real `lfd`-owned PTY transport that keeps executor choice, auth, and attach semantics in one place for local, container, and future remote runs.

Treat today's tabbed terminal workspace as the seam for later composition work. Split layouts, persistence, and keyboard routing should promote the existing `TerminalSession` / `TerminalWorkspaceStore` model instead of replacing it with pane-local session identities. In particular, session ordering and selection already persist per repo; compositor work needs to either extend that repo-scoped state deliberately or replace it with a richer layout model in one move.

Derive cross-wave and cross-repo views from the same stores that already power the queue and terminal sidebar. The existing portfolio window already proves the repo-card shell: basic per-repo wave counts, blocked counts, and diff summaries. Future portfolio work should deepen that surface with shared wave/run/attention/session queries instead of building a second dashboard stack beside it. The persisted `terminal_sessions` records in `lfd` are also the source of truth for adoption and latency measurement: portfolio trend lines, in-app completion rate, and resume-latency work should query those rows rather than inventing a second analytics cache.

## Goals

- Primary Concerto screen is an attention queue, not a chat view
- Every human checkpoint in build and tend flows surfaces as an `AttentionItem`
- Coding sessions happen in embedded Ghostty terminals
- Multi-repo, multi-wave status is visible at a glance
- Wave lifecycle (create, configure, start, stop) is managed from Concerto
- Human calibration moments have a dedicated UX
- Chord-wave graph and portfolio views derive from existing wave/chord relationships

## Risks

- Partial attention coverage still creates blind spots until design review and calibration checkpoints surface through canonical `interactive_step` payloads
- The current launch-spec shim diverges from the eventual daemon-owned PTY design (tracked in `wave/lfd/`); remote repos stay blocked until transport is upgraded
- Portfolio scope can expand unboundedly; repo/chord aggregation needs store-level queries before the view goes broad
- Lifecycle or compositor work could drift from `lfd` terminal semantics if Swift starts inventing its own launch, completion, or persistence rules
- Ghostty C library linkage is build-environment sensitive; `GhosttyTerminalView` depends on the library being available at link time
- Terminal session cleanup still depends on completion callbacks; blocked POSTs or hard-killed processes can leave sessions stuck in `running` state
- The product surface foregrounds one run per selected wave even though the runtime acknowledges many-run waves; portfolio and lifecycle work should not assume single-run exclusivity

## Metrics

- Clicks from “I see a problem” to “I’m acting on it” (target: <=2)
- Share of unresolved human checkpoints represented as attention items (target: 100%)
- Time to assess all-waves status (target: <10 seconds glance)
- Percentage of coding sessions that happen inside Concerto vs external terminal (target: >70%)
- Terminal session resume latency from process exit to wave resumption (target: p95 <2s)
