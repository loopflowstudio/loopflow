# Agent Embedding

## Vision

Concerto becomes the conductor, not the chat client. The repo window opens on an attention queue that shows what needs human judgment across waves, while coding sessions themselves stay in a real terminal. Concerto's value is everything around the terminal: attention, portfolio awareness, calibration, worktree and PR lifecycle, and native compositions of those surfaces.

Not transcription, not a chat-shell wrapper. The agent runs in a terminal. Concerto makes parallel agent work legible and steerable.

## Strategy

Use one durable human-attention model across the product. `AttentionItem` is the shared contract for code review, design review, calibration, and failure states; the queue stays the home screen, and later portfolio/lifecycle/compositor surfaces should reuse that model instead of inventing parallel status systems.

Complete the attention queue before broadening the shell around it. The remaining work in this wave is to finish the missing design-review and calibration paths, then embed the terminal, then widen into portfolio, lifecycle, and composition views. Each later surface should reduce clicks from “I see a problem” to “I’m acting on it,” not add another place to check.

Keep the agent in a real terminal. Concerto should wrap Ghostty or another full terminal with wave-aware context, not compete with purpose-built chat clients on chat chrome.

Derive cross-wave and cross-chord views from existing wave data, run history, and attention streams. No separate dashboard-only model that can drift from lfd.

## Goals

- Primary Concerto screen is an attention queue, not a chat view
- Every human checkpoint in build and tend flows surfaces as an `AttentionItem`
- Coding sessions happen in embedded Ghostty terminals
- Multi-repo, multi-wave status is visible at a glance
- Wave lifecycle (create, configure, start, stop) is managed from Concerto
- Human calibration moments have a dedicated UX
- Chord-wave graph and portfolio views derive from existing wave/chord relationships

## Risks

- Partial attention coverage creates blind spots until design review and calibration moments surface through `AttentionItem`
- Ghostty terminal embedding is still unproven in SwiftUI and may require a fallback
- Portfolio scope can expand unboundedly; repo-scoped attention queries will need store-level aggregation before cross-repo scale
- Lifecycle UI could duplicate CLI flows instead of simplifying them if it diverges from existing wave/worktree semantics

## Metrics

- Clicks from “I see a problem” to “I’m acting on it” (target: <=2)
- Share of unresolved human checkpoints represented as attention items (target: 100%)
- Time to assess all-waves status (target: <10 seconds glance)
- Percentage of coding sessions that happen inside Concerto vs external terminal (target: >70%)
