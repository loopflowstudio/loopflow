# Agent Embedding

## Vision

Concerto as conductor, not chat client.

The repo window opens on an attention queue that shows what needs human judgment across waves, while coding sessions stay in a real terminal. Concerto's value is everything around the terminal: attention, portfolio awareness, calibration, worktree and PR lifecycle, and native compositions of those surfaces.

Not transcription, not a chat-shell wrapper. The agent runs in a terminal. Concerto makes parallel agent work legible and steerable.

## Strategy

## Execution model

The daemon should stay **parallel-first**. A wave is configuration and grouping; a run is execution. `primary_flow` is the wave default, while each `WaveRun` owns its actual `snapshot.flow`, worktree, branch, status, and any interactive terminal session. Reactive work like CI-fix, repo triggers, cron, and listener activations should compose as additional runs, not as special cases that mutate the wave's identity.

The execution path should also converge on the real CLI. The first cut is not "daemon-owned PTYs everywhere." The first cut is that `lf` writes structured lifecycle state to a globally agreed runtime store when that store is present. `lfd` can supervise and later launch normal `lf <flow-or-step>` commands against that same store instead of carrying a second bespoke executor forever. Concerto can then consume the same runtime state whether the work started from the CLI, from the app, or from daemon automation.

Concerto can still present a calmer singular surface: one selected wave, one foreground run, one presented terminal. That is a product policy, not an infrastructure limit. Serialized waves remain useful where roadmap UX wants one thing at a time, but the base model should allow many runs and worktrees per wave.

### Algedonic escalation routing

The algedonic path should eventually route through the wave hierarchy: child wave → parent wave → root wave → human. Only the root wave's escalation surfaces as a human attention item. This makes agent-to-agent escalation possible within a wave family before bothering a human. The HTTP contract and attention types are ready; the routing policy is the remaining design work.

Keep one durable model per concept. `AttentionItem` remains the shared contract for human checkpoints, `WaveRun` remains the execution unit, and `TerminalSession` remains the shared contract for embedded coding sessions. Portfolio, lifecycle, calibration, and composition work should derive from those existing types plus wave/run data instead of inventing dashboard-only state.

The local-first terminal workspace is now in place: `lfd` persists `terminal_sessions`, emits terminal-session events, and Concerto embeds tracked Ghostty tabs keyed by terminal-session ID while keeping Work as the default surface. That gives the queue, workspace, and future portfolio views one shared state model for interactive runs instead of a separate Swift-only terminal stack. In the near term, that can stay local-first: Concerto can open ordinary local Ghostty sessions while relying on the shared runtime store for durable state.

Interactive checkpoint routing is also shipped. Every `WaitInteractive` step now produces an `interactive` attention item with typed context (step name, design path for `review-design`, mutation summary for `wave/review`, terminal session ID). The executor owns creation and resolution — steps don't need to manage attention items. This is the foundation that calibration and other step-specific views build on.

But the transport is still transitional. `attach` currently returns a local wrapped shell command built from agent argv, completion still depends on callback POSTs, and the terminal does not yet show a daemon-owned `lf <step-or-flow>` PTY. The next move should not deepen that shim. It should lean into the shared-store contract and local terminal embedding first, then ask whether remote should begin as SSH into a host/container before `lfd` grows a custom PTY transport.

The tmux architecture study (shipped, guidance propagated into `wave/lfd/` items and `04-window-composition.md`) clarifies the transport boundary. Concerto is a client in tmux terms — it should never own PTYs, session lifecycle, or process supervision. It attaches to sessions, sends input, receives output, and manages layout. All persistent state lives in `lfd`. This means agent-embedding work should:
- Build around `TerminalSession` IDs from `lfd`, not locally-invented session handles
- Treat Ghostty embedding as a rendering surface, not a session owner
- Layout serialization is shipped (`MultiplexerLayout` encodes split trees as `Codable` data, persisted per wave via `MultiplexerStore`)
- Expect multi-client size negotiation to be a daemon concern — Concerto sends its viewport size, `lfd` decides

The workspace multiplexer is now in place: a recursive binary split tree per wave, persisted via `MultiplexerStore`, with roadmap, README, runs, launcher, terminal, markdown, diff, and launchpad panes. The default workspace opens into roadmap + runs + terminal, and the command palette focuses existing panes before creating duplicates. The terminal pane is backed by tmux (`TmuxSession`), and focus-aware keyboard routing dispatches to SwiftUI or tmux based on first-responder detection. `TerminalWorkspaceStore` manages session ordering and selection per repo; the multiplexer extends that model with spatial layout. Remaining composition work (richer pane content, directional focus, named layouts, layout migrations) should deepen these surfaces rather than introducing parallel state.

Derive cross-wave and cross-repo views from the same stores that already power the queue and terminal sidebar. The existing portfolio window already proves the repo-card shell: basic per-repo wave counts, blocked counts, and diff summaries. Future portfolio work should deepen that surface with shared wave/run/attention/session queries instead of building a second dashboard stack beside it. The persisted `terminal_sessions` records in `lfd` are also the source of truth for adoption and latency measurement: portfolio trend lines, in-app completion rate, and resume-latency work should query those rows rather than inventing a second analytics cache.

A terminal-per-wave dashboard remains plausible, but it belongs after `wave/lfd/` ships daemon-owned tmux sessions. Until then, agent-embedding work should expose terminal presence, session state, and drill-in paths without growing a second Swift-owned terminal model.

## Goals

- Primary Concerto screen is an attention queue, not a chat view
- Every human checkpoint in build and garden flows surfaces as an `AttentionItem`
- Coding sessions happen in embedded Ghostty terminals
- Multi-repo, multi-wave status is visible at a glance
- Wave lifecycle (create, configure, start, stop) is managed from Concerto
- Human calibration moments have a dedicated UX
- Chord-wave graph and portfolio views derive from existing wave/chord relationships

## Risks

- Partial attention coverage still creates blind spots until design review and calibration checkpoints surface through canonical step-scoped `interactive` payloads
- The current launch-spec shim diverges from the shared-store-first runtime model and the eventual daemon-owned PTY design (tracked in `wave/lfd/`); Swift should avoid taking new dependencies on it
- Portfolio scope can expand unboundedly; repo/chord aggregation needs store-level queries before the view goes broad
- Lifecycle or compositor work could drift from `lfd` terminal semantics if Swift starts inventing its own launch, completion, or persistence rules
- Ghostty C library linkage is build-environment sensitive; `GhosttyTerminalView` depends on the library being available at link time
- Persisted `MultiplexerLayout` trees can go stale across schema changes if migrations and reset paths are not kept in sync
- The direct-key typing path uses `NSTextInputContext.selectedKeyboardInputSource` containing `inputmethod` to detect IME keyboards; unusual input sources may route incorrectly until validated with broader CJK and third-party input methods
- Terminal session cleanup still depends on completion callbacks; blocked POSTs or hard-killed processes can leave sessions stuck in `running` state until the shared-store contract replaces that path
- `ConcertoUITests-Runner` exits during bootstrap before establishing the UI-test connection; `xcodebuild test` passes app build and unit/package tests but the UI-test harness fails locally even after clean DerivedData rebuilds
- The product surface foregrounds one run per selected wave even though the runtime acknowledges many-run waves; portfolio and lifecycle work should not assume single-run exclusivity

## Metrics

- Clicks from “I see a problem” to “I’m acting on it” (target: <=2)
- Share of unresolved human checkpoints represented as attention items (target: 100%)
- Time to assess all-waves status (target: <10 seconds glance)
- Percentage of coding sessions that happen inside Concerto vs external terminal (target: >70%)
- Terminal session resume latency from process exit to wave resumption (target: p95 <2s)
