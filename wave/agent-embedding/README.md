# Agent Embedding

## Vision

Concerto becomes the conductor, not the chat client. The repo window opens on an attention queue that shows what needs human judgment across waves, while coding sessions themselves stay in a real terminal. Concerto's value is everything around the terminal: attention, portfolio awareness, calibration, worktree and PR lifecycle, and native compositions of those surfaces.

Not transcription, not a chat-shell wrapper. The agent runs in a terminal. Concerto makes parallel agent work legible and steerable.

## Strategy

Keep one durable model per concept. `AttentionItem` remains the shared contract for human checkpoints, and `TerminalSession` remains the shared contract for embedded coding sessions. Portfolio, lifecycle, calibration, and composition work should derive from those existing types plus wave/run data instead of inventing dashboard-only state.

The local-first terminal workspace is now in place: `lfd` owns terminal lifecycle, Concerto embeds Ghostty tabs keyed by terminal-session ID, and the sidebar already shows wave context beside the real terminal. Launch args, completion tokens, and run resumption still flow through `lfd`, so follow-on UI work should keep that authority instead of adding a second Swift-side callback path. The remaining work is to finish attention coverage for design review and chord calibration, then widen out into portfolio, lifecycle, and richer window composition without reintroducing chat-shaped coding surfaces.

Treat today's tabbed terminal workspace as the seam for later composition work. Split layouts, persistence, and keyboard routing should promote the existing `TerminalSession` / `TerminalWorkspaceStore` model instead of replacing it with pane-local session identities. In particular, session ordering and selection already persist per repo; compositor work needs to either extend that repo-scoped state deliberately or replace it with a richer layout model in one move.

Derive cross-wave and cross-repo views from the same stores that already power the queue and terminal sidebar. The existing portfolio window already proves the repo-card shell: basic per-repo wave counts, blocked counts, and diff summaries. Future portfolio work should deepen that surface with shared wave/run/attention/session queries instead of building a second dashboard stack beside it. The persisted `terminal_sessions` records in `lfd` are also the source of truth for adoption and latency measurement: portfolio trend lines, in-app completion rate, and resume-latency work should query those rows rather than inventing a second analytics cache.

## Goals

- Primary Concerto screen is an attention queue, not a chat view
- Every human checkpoint in build and garden flows surfaces as an `AttentionItem`
- Coding sessions happen in embedded Ghostty terminals
- Multi-repo, multi-wave status is visible at a glance
- Wave lifecycle (create, configure, start, stop) is managed from Concerto
- Human calibration moments have a dedicated UX
- Chord-wave graph and portfolio views derive from existing wave/chord relationships

## Risks

- Partial attention coverage still creates blind spots until design review and calibration checkpoints surface through canonical `interactive_step` payloads
- Local-only terminal embedding creates a temporary product split; remote repos need explicit queue/detail states until a remote PTY transport exists
- Portfolio scope can expand unboundedly; repo/chord aggregation needs store-level queries before the view goes broad
- Lifecycle or compositor work could drift from `lfd` terminal semantics if Swift starts inventing its own launch, completion, or persistence rules
- Ghostty C library linkage is build-environment sensitive; `GhosttyTerminalView` depends on the library being available at link time
- Terminal session cleanup relies on `onSessionClosed` callback; processes killed via SIGKILL can leave sessions stuck in `running` state

## Metrics

- Clicks from “I see a problem” to “I’m acting on it” (target: <=2)
- Share of unresolved human checkpoints represented as attention items (target: 100%)
- Time to assess all-waves status (target: <10 seconds glance)
- Percentage of coding sessions that happen inside Concerto vs external terminal (target: >70%)
- Terminal session resume latency from process exit to wave resumption (target: p95 <2s)
