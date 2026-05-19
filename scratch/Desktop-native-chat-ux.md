---
status: in-progress
claimed_by: f6684ab5-6f24-4bec-97a5-52dcc912fd38
claimed_at: 2026-05-19T02:01:39.027621Z
asana_id: '1214270115439574'
---
# Native chat UX

## Problem

Concerto's chat works — SSE streaming, turn state, tool-call grouping, voice
input, quote-reply all ship. But it reads like a debug console, not a place you
want to think. Three concrete gaps stop it from being a product:

- **Assistant text is unstyled.** On macOS the assistant message is a plain
  `NSTextView` with `isRichText = false`
  (`SelectableAssistantMessageTextView.swift:69,82`). Headings, lists, bold,
  links, blockquotes all render as literal `#`, `*`, `-`. Code blocks are
  monospace with zero syntax color. ` ```diff ` blocks route to the generic
  `CodeBlockView`, not the diff renderer that already exists.
- **History is unreachable.** lfd persists every session event durably
  (`session_events` table, no pruning). The store can already list a wave's
  sessions and batch-fetch their events — `usage.rs:81,88` does exactly this.
  But no HTTP route exposes it, so closing a conversation loses it. You cannot
  scroll back a week.
- **The composer is a bare `TextField`.** No file drop, no slash commands, no
  way to point the agent at the file you're looking at.

Who benefits: the conductor doing exploratory work — "how should I structure
this?" — who today drops to a terminal because the app's chat is worse than
`lf`'s. This wave's README is explicit: once the embedded build loop is
first-class, native chat is what keeps exploratory work in the app.

## Approach

Three milestones, each end-to-end and independently noticeable. Build in this
order; each lands on its own.

### M1 — Rich rendering (no new dependencies)

Replace the single `parseMessageSegments` (text vs. code) with a real block
model in `LoopflowCore` so it is testable without SwiftUI:

```swift
enum MarkdownBlock: Equatable {
    case paragraph(AttributedString)        // inline md via AttributedString(markdown:)
    case heading(level: Int, AttributedString)
    case list(ordered: Bool, items: [AttributedString])
    case blockquote([MarkdownBlock])
    case code(language: String?, content: String)
    case diff(String)                       // language == "diff" | "patch"
    case rule
}
func parseMarkdownBlocks(_ content: String) -> [MarkdownBlock]
```

`Concerto` renders each block as a native view, styled with the design system
(headings in `loopflowBurgundy`, `Spacing`/`CornerRadius` tokens). Inline spans
(bold/italic/code/links) come from `AttributedString(markdown:
.inlineOnlyPreservingWhitespace)` — the exact technique the iOS path already
uses (`SelectableAssistantTextView.swift:108`), now unified across platforms.
` ```diff ` / ` ```patch ` blocks render through the existing `DiffLinesView`
(`DiffLinesView.swift`), which already does add/delete/hunk coloring.

Syntax highlighting: a built-in synchronous `SyntaxHighlighter` covering the
languages that actually appear in agent output — swift, rust, python,
bash/sh/zsh, json, yaml, toml, diff, markdown — with a plain fallback for
everything else. No JS engine, no tree-sitter, no package. It tokenizes by
keyword/string/comment/number heuristics and themes to the palette. Good
enough for a chat surface; not an IDE.

### M2 — Conversation history

Add `GET /v0/sessions?repo=<path>&wave_id=<id>&limit=<n>&before=<iso8601>`.
It mirrors the `usage.rs` query path exactly: `list_sessions_for_wave` →
`list_events_for_sessions` → derive a summary per session.

```rust
// new DTO — follows CLAUDE.md DTO rules: no defaults, every field required or Optional
pub struct SessionSummaryDto {
    pub id: String,
    pub harness: String,
    pub wave_id: Option<String>,
    pub wave_name: Option<String>,
    pub title: String,          // first user message, truncated to 80 chars
    pub message_count: u32,
    pub status: String,         // active | ended | failed
    pub created_at: String,
    pub ended_at: Option<String>,
}
```

Swift: `LocalWaveService.listSessions(repo:waveId:limit:before:)`. UI: a
history panel reachable from `WaveSessionView` — list grouped by day, filter by
wave (the `wave_id` param) and time (the `before` cursor). Tapping a row
resumes through the path that **already works**: `joinSession(id)` +
`streamSessionEvents(afterSeq: 0)` replays the persisted event log into a
transcript. Ended sessions replay read-only (composer disabled, "Resumed from
history" system row); active sessions join live. No new resume machinery.

### M3 — Composer upgrades

- **File drop.** `.onDrop` of file URLs. Inside the session's working repo →
  insert an `@<relative/path>` token into the composer (the agent's own file
  tools resolve it — no upload, no sandbox copy). Outside the repo → a brief
  inline notice; copying arbitrary files into the agent's cwd is out of scope.
- **Slash commands.** `/` at the start of an empty composer opens a small menu.
  Commands are grounded prompt scaffolds, not magic: `/file <path>` (insert a
  repo file reference), `/code` (prefix the turn as a code request), `/search
  <query>` (instruct a repo search), `/image` (file-drop/picker an image path
  the agent can read). Small set, extensible via one registry.
- **Context awareness.** A context chip row above the composer showing the
  current wave (already in `contextSnapshot`) and a toggle to attach the
  current quote/selection. Selection injection reuses the existing `ReplyQueue`
  — no parallel context system.

## De-risking

| Question | Finding | Impact on design |
|----------|---------|-----------------|
| Does SwiftUI `Text(AttributedString)` render block markdown (lists, headings, quotes)? | No. `AttributedString(markdown:)` encodes block structure as `presentationIntent` attributes but `Text` lays out inline only. Block layout must be ours. | Custom block model + native per-block views. Inline spans only via `AttributedString`. Confirms M1 design; rules out "just use AttributedString". |
| Add a markdown package (swift-markdown-ui)? | Large dependency, brings its own theming that fights `VISUAL_DESIGN.md` (burgundy headings, cream/slate, design tokens). Repo has only ViewInspector + WhisperKit and a strong simplicity bias. | Rejected. Custom parser is ~1 file, fully testable in `LoopflowCore`, themable. |
| Is there a backend to list past sessions? | Store layer already has `list_sessions_for_wave` and `list_events_for_sessions`; `usage.rs:81-88,218-237` already calls them for token usage. Only the HTTP route + DTO + client + UI are missing. | History collapses from "design persistence" to "expose an existing query". Big de-risk. |
| Are session events durable / not pruned? | `session_events` rows are append-only via `append_session_event`; no `DELETE`/prune path found in `sqlite.rs`. Replay via `list_session_events(after_seq)` is the same path used for live reconnect today. | History resume needs no new storage or replay code — reuse `joinSession` + `afterSeq: 0`. |
| Syntax highlighting tech? | Highlightr = JSContext + highlight.js (async, heavyweight, streaming jank risk); Splash = Swift-only; tree-sitter = heavy native deps. | Built-in heuristic tokenizer, synchronous, themed. "Production quality" for chat ≠ IDE-accurate. Avoids the streaming-jank failure mode entirely. |
| Streaming perf at 30 tok/s, 100-entry transcript? | `MessageSegmentCache` keys on `content.count`, so it re-parses the whole streaming message every token. Full block parse + highlight on every delta over a 100-row `LazyVStack` is the frame-drop risk. | Split path: finalized messages parse+highlight once, cached by `(id, finalLength)`; the actively-streaming message uses a cheap path (fence split + plain text, no per-token inline/highlight) and swaps to the rich render once on `TurnCompleted`. `LazyVStack` already virtualizes. |
| Resume of an *ended* session — live join or replay? | `streamSessionEvents` replays persisted events then goes live; an ended session simply has no live tail. | Ended → read-only replay, composer disabled. Active → join live. One code path, gated by status. |
| File drop into an LLM agent — what does "picks it up" mean? | Agent runs in lfd with the repo as cwd and has file tools. | Drop inserts a path reference, not an upload. In-repo only. Keeps scope and the security boundary tight. |

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| `swift-markdown-ui` package for rendering | Fast to adopt, full CommonMark | Heavy dep, fights the design system, opaque under streaming. Custom parser is testable and themable. |
| `AttributedString(markdown: .full)` + `Text` | Zero code | Doesn't lay out blocks — verified. Lists/headings collapse. |
| Highlightr (highlight.js via JSCore) for syntax | Broad language coverage | Async + JS bridge re-tokenizes on every streaming delta → exactly the jank the metric forbids. |
| Persist transcripts client-side for history | Works offline | Duplicates a durable server log; invents a sync problem. lfd already is the source of truth (same philosophy as embedded-terminal "tmux is source of truth"). |
| New resume endpoint / transcript-fetch route | Explicit | Redundant. `joinSession` + `afterSeq: 0` already reconstructs a full transcript from the event log. |
| Full slash-command DSL / plugin system | Powerful | Scope sprawl; steals focus from the p1 build-driver. A 4-command grounded set with one registry is enough to be noticeable. |

## Key decisions

- **No new Swift package.** Markdown block model and syntax highlighter are
  hand-written in `LoopflowCore`. Justified by the de-risking: the alternatives
  either don't work (`AttributedString` blocks) or break the perf metric
  (Highlightr) or fight the design system (swift-markdown-ui).
- **Block model lives in `LoopflowCore`, rendering in `Concerto`.** Parsing is
  unit-tested with no SwiftUI host, matching the existing `parseMessageSegments`
  test pattern (`WaveSessionViewTests`).
- **History is a read path over an existing write path.** New code is one
  route + one DTO + one client method + UI. Resume is the existing reconnect
  code with `afterSeq: 0` and a status gate. `SessionSummaryDto` follows the
  CLAUDE.md DTO rules (no defaults; required-or-Optional) and gets a
  `tests/fixtures/dto/` round-trip fixture asserted in Rust, Swift, Python.
- **Streaming message uses a deliberately cheaper render than the finalized
  one.** The rich parse runs once per message at turn completion, never
  per-token. This is the design's answer to the 0-dropped-frames target, not
  an afterthought.
- **File drop inserts a path, never uploads.** Keeps the agent's filesystem
  boundary intact and the feature one-screen simple.
- **Composer context reuses `ReplyQueue` + `contextSnapshot`.** No second
  context-assembly system.

## Scope

**In scope**

- M1: block markdown parser + per-block native rendering (macOS + iOS unified);
  built-in syntax highlighter; ` ```diff ` → `DiffLinesView`; split
  streaming/finalized render path with the new cache key.
- M2: `GET /v0/sessions` list route + `SessionSummaryDto` + DTO fixture;
  `listSessions` Swift client; history panel with wave + time filter; resume
  via existing join/replay with read-only gating for ended sessions.
- M3: file drop (in-repo path token), slash-command menu (`/file`, `/code`,
  `/search`, `/image`) with a registry, context chip row.

**Out of scope**

- Replacing the CLI or the embedded terminal (wave README "not here").
- Governance/usage dashboards (those are `workflows`, not desktop chrome).
- Uploading or sandbox-copying out-of-repo files.
- IDE-grade syntax accuracy; arbitrary-language highlighting.
- Cross-device transcript sync; server-side full-text search of history
  (client-side filter on the listed page is enough for v1).
- Any change to the p1 embedded-terminal path.

## Done when

- A reply containing headings, a bulleted list, **bold**, a link, a
  syntax-colored ` ```rust ` block, and a ` ```diff ` block renders each as a
  styled native element — verified by `swift test --package-path swift
  --filter MarkdownBlock` plus the manual walkthrough below.
- From a fresh app launch you can open the history panel, filter to a wave,
  open a week-old conversation, and read its full transcript without touching
  a terminal; reopening an active session resumes live.
- Dragging an in-repo file onto the composer inserts its path; `/` opens the
  command menu and `/file` inserts a reference.
- DTO fixture `tests/fixtures/dto/session_summary.json` round-trips in Rust,
  Swift, and Python fixture tests.
- `cargo test --all`, `uv run pytest python/tests/`, `swift test
  --package-path swift`, and the Concerto UI suite are green.
- Manual: extend `scripts/concerto-dev.py` (or a sibling `scripts/`
  walkthrough) so one command launches lfd + Concerto into a seeded chat that
  exercises rendering, history, and the composer.

## Measure

The streaming-smoothness target ("0 dropped frames at 30 tok/s, 100-entry
transcript") needs a number, not a vibe.

- **Baseline:** before M1, time the current
  `MessageSegmentCache.segments(for:)` on a representative 2 KB assistant
  message and record it.
- **Budget:** at 30 tok/s the per-delta work must stay well under one 60 fps
  frame. Target: block parse of the streaming message's cheap path **< 1 ms**
  per delta; the one-time rich finalize (parse + highlight) of a 2 KB message
  **< 8 ms**.
- **Test:** add a `LoopflowCore` performance unit test that feeds a 100-block
  synthetic transcript and asserts the streaming-path per-delta cost stays
  under the 1 ms budget (fail the build if it regresses).
- **Manual confirm:** the `scripts/` walkthrough drives a scripted 30 tok/s
  feed into a 100-entry transcript; watch for scroll/layout jank with the
  finalized rich render mixed with one live streaming row.

## Wave alignment

- **Vision:** README — "Once that daily build loop feels first-class, polish
  native chat so exploratory work can stay in the app too." This item is that
  polish; it does not touch the p1 embedded-terminal path.
- **Goals advanced (item "Done when"):** "Markdown / syntax highlighting /
  diffs render at production quality" → M1. "Browse and resume past sessions
  without dropping to terminal" → M2. "Composer supports file drop and slash
  commands" → M3. "Streaming stays smooth — 0 dropped frames at 30 tok/s,
  100-entry transcript" → the split render path + the Measure section.
- **Risks (README) checked in wild-failure below.** New risk introduced: a
  hand-written highlighter/parser is a maintenance surface — bounded by keeping
  the language set small and the parser fully unit-tested.

## Imagine wild success

Six weeks out, the conductor opens Concerto, not a terminal, to ask "how should
I structure the session-history store?" The answer comes back as styled prose
with a syntax-colored Rust snippet and a clean diff block. They drag
`store/sqlite.rs` onto the composer, the path drops in, the agent reads it.
Mid-thought they `/search` for prior art. A week later they reopen that exact
conversation from history to recover a decision — the transcript is all there,
read-only, exactly as it streamed. The surprise: history makes chat feel
*durable*, and durable conversations get used like a notebook, not a scratchpad.

## Imagine wild failure

We shipped a markdown renderer that re-parses every assistant message on every
token; at 30 tok/s with 100 rows the transcript stutters and people go back to
the terminal — the precise README risk ("Chat UX should not steal focus") and
the exact metric, missed. Or the hand-written highlighter mis-tokenizes Rust
lifetimes and every code block looks subtly wrong, which is worse than no
color. Or history "works" but resuming a 2-hour session replays thousands of
events and the app beachballs. The design's answers: the split render path
(streaming never does the expensive work), a small audited language set with
golden tests, and history resume reusing the already-proven reconnect/replay
path that production already runs on live reconnect. If any of these slip in
review, fix the path — don't ship the regression.
