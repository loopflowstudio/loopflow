# UX

## Vision

Make Concerto lovable. Polish the interactive session, surface code and context inline, eliminate unnecessary context-switches. Not a full code editor, not a code review workflow, not file editing — those are GitHub and Cursor. Not session persistence/history (already solid).

Concerto should be self-sufficient for the "check in" workflow — glancing at what changed, what's planned, and what the agent is doing. The session should feel alive. Streaming text, pulsing indicators, responsive actions. Not a static chat log.

## Strategy

Session polish (Phase 01) shipped — thinking indicator, streaming cursor, code blocks render cleanly, copy affordances, timestamp collapsing, `/` focuses the interactive composer.

Inline glanceability (Phase 02) in progress — G1 (file items show diffs) shipped with `DiffLinesView` + `synthesize_edit_diff()`. G2–G5 remain: wave diff stat expand, roadmap expansion, wave README, scratch doc glance.

The goal is glanceability, not a full document viewer. If we're adding scroll-to-line or search-in-diff, we've gone too far.

### Open follow-ups

- Slash-focus scope: should `/` focus the composer only in interactive waiting sessions, or also in non-interactive Chat tab states?

## Goals

- Eliminate rough edges in the interactive session experience
- Make diffs and wave content glanceable without leaving Concerto
- Build reusable components (diff rendering, code blocks, copy buttons) that compound across features
- Keep linking to GitHub/Cursor for their full capability

## Risks

- **Over-building the viewer.** The goal is glanceability, not a full document viewer. If we're adding scroll-to-line or search-in-diff, we've gone too far.
- **Markdown rendering rabbit hole.** SwiftUI's AttributedString is limited. Don't fight it — code blocks and inline formatting are enough.
- **lfd scope creep.** Inline glance needs a per-file diff endpoint. Keep it minimal — serve the data Concerto needs, nothing more.

## Metrics

- Session feels live: streaming text visible, thinking indicator during agent work, responsive actions
- Diffs glanceable inline: file items expand to show changed lines without leaving Concerto
- Wave content glanceable: roadmap, README, scratch docs visible in wave detail panel
- Zero unnecessary context-switches for the "check in" workflow
