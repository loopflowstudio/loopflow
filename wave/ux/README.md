# UX

Make Concerto lovable. Polish the interactive session, surface code and context inline, eliminate unnecessary context-switches.

## Vision

Concerto should be self-sufficient for the "check in" workflow — glancing at what changed, what's planned, and what the agent is doing. GitHub and Cursor are for deep dives: editing code, reviewing with comments, line-by-line PRs. Concerto is for staying in flow.

The session should feel alive. Streaming text, pulsing indicators, responsive actions. Not a static chat log.

### Not here

- Full code editor inside Concerto
- Code review workflow (comments, approvals) — that's GitHub
- File editing — that's Cursor
- Session persistence/history browsing (already solid)

## Goals

- Eliminate rough edges in the interactive session experience
- Make diffs and wave content glanceable without leaving Concerto
- Build reusable components (diff rendering, code blocks, copy buttons) that compound across features
- Keep linking to GitHub/Cursor for their full capability

## Phase boundaries

- **01-session-polish**: shipped on branch `jack-heart.ux.20260225_2145`. Session feels live (thinking indicator + streaming cursor), assistant code blocks render cleanly, copy affordances are in place, timestamps collapse intelligently, and `/` can focus the interactive composer.
- **02-inline-glance**: in progress. G1 (file items show diffs) shipped — `DiffLinesView` + `synthesize_edit_diff()`. G2–G5 remain: wave diff stat expand, roadmap expansion, wave README, scratch doc glance.
- **03-run-visibility**: sketch/backlog. Improve autonomous-run visibility once inline glanceability is in place.

## Open follow-ups

- Slash-focus scope: should `/` focus the composer only in interactive waiting sessions, or also in non-interactive Chat tab states?

## Risks

- **Over-building the viewer.** The goal is glanceability, not a full document viewer. If we're adding scroll-to-line or search-in-diff, we've gone too far.
- **Markdown rendering rabbit hole.** SwiftUI's AttributedString is limited. Don't fight it — code blocks and inline formatting are enough.
- **lfd scope creep.** Inline glance needs a per-file diff endpoint. Keep it minimal — serve the data Concerto needs, nothing more.
