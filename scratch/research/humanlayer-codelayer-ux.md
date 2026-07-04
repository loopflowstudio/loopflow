# HumanLayer CodeLayer / WUI — supervision UX

CodeLayer (a.k.a. WUI, `humanlayer-wui/`) = Tauri + React desktop app over the Go
daemon, running many parallel Claude Code sessions with mandatory human approval
gates. Closest shipped prior art to Concerto.

## 1. Session visibility — one renderer per tool type
Transcript = event stream (`ConversationStream/ConversationEventRow.tsx`) with a
**purpose-built renderer per tool** (`EventContent/`): `BashToolCallContent`,
`EditToolCallContent`, `MultiEditToolCallContent`, `ReadToolCallContent`,
`Grep/Glob/LSToolCallContent`, `Write`, `WebFetch/WebSearch`, `Task`,
`MCPToolCallContent`, `TodoWriteToolCallContent`, `ExitPlanModeToolCallContent`,
`Assistant/UserMessageContent`, `Unknown*` fallbacks.

Rendering choices worth stealing:
- **Every tool call has `ToolHeader` + `StatusBadge`** (pending/running/denied/
  done) — in-progress vs done is a per-row badge, not a global spinner. Session-
  level `OmniSpinner`, `StatusBar`, `TokenUsageBadge`.
- **Diffs first-class** — `CustomDiffViewer` + `DiffViewToggle` render Edit/Write/
  MultiEdit as real diffs inline. File edits shown as the change, not raw args.
- **Tool results summarized inline + expandable** — `formatToolResult.tsx` +
  `ToolResultModal`; long output condensed in-row, full result in a modal.
- **Plans render as markdown** — `ExitPlanModeToolCallContent` (`MarkdownRenderer`,
  `${lineCount} lines` header); once approved **collapses to "Plan mode exited
  successfully"** (expand on demand). Denied plans show "Denial Reason:" inline.
- **Todos = persistent widget** (`TodoWidget.tsx`) separate from the transcript.
- **Subagents grouped** — `TaskGroupEventRow.tsx` + `TaskPreview` collapse a
  `Task` subagent's nested tool calls into one expandable group.
Navigation is keyboard-first (vim `j/k`, jump, command palette).

## 2. Approval / decision UX — inline, not an inbox
A pending decision renders **in place, wrapping the exact tool call it gates**
(`EventContent/ApprovalWrapper.tsx`): tool-call row renders normally, and a button
row appears directly below only when `needsApproval && event.approvalId`.
- **`Approve [A]`** and destructive **`Deny [D]`**, single-key. Approve is
  two-step confirm.
- **Deny-with-comment is the default deny path** — `DenyButtons.tsx` swaps in a
  required free-text reason (submit disabled until non-empty), fed back to the
  agent, later shown as "Denial Reason: …".
- Plan approval reuses the same `StatusBadge` — "approve this plan" and "approve
  this bash command" are the *same* interaction at different altitudes.
The approval lives at the point in the timeline where it happened — full
surrounding context instead of a decontextualized queue item.

## 3. Multi-session supervision — a triage table, status = attention
Dashboard = `SessionTable.tsx` (dense table, not cards). Columns: Selection /
Status / Title (inline-editable) / Started (relative). Triage cues:
- **Status column IS the attention signal** — "Needs Approval"/waiting pulls the
  eye; count badges on view tabs aggregate.
- **Special-mode glyphs**: `⏵⏵` (warning) = auto-accept on; `ShieldOff` (error) =
  bypass-permissions. At a glance you see which sessions run unsupervised.
- **Three views** (Normal/Drafts/Archived) via `Tab`/`Shift+Tab`; completed work
  archives out of the triage list.
- **Vim nav + bulk ops** (`j/k`, `x` select, `Shift+j/k` range, `e` archive, `gg`,
  `Alt+a` toggle auto-accept) with undo-via-toast.
- **OS notifications** fire when a session needs attention ("close your laptop,
  agents keep working").
Mental model: **the table is the outer loop.** You live in the list; you drop into
a session only when its status demands you.

## 4. Steering / interrupt UX — composer always live
`ActiveSessionInput.tsx` — never disabled while the agent runs; submit behavior is
state-dependent:
- **Running + empty** → **Interrupt**.
- **Running + text** → **"Interrupt & Send"** (stop + inject your message as new
  direction). One button, label changes with content.
- **Waiting for approval + text** → **denies the oldest pending approval using
  your text as the reason.** The composer *is* the deny-with-comment box.
- **Idle** → normal send.
Interrupt debounced 500ms; disabled "Waiting…" until the agent initializes. No
queued-message model — send-now + explicit interrupt. Composer richness:
`@`-file mentions, slash commands, model selector, additional-dir grants.

## 5. IA & supervisor mental model
"**Do not outsource the thinking.**" Humans are **active gatekeepers in the loop**,
not reviewers of finished output — approval gates are mandatory checkpoints baked
into tool execution. Phased QRSPI workflow makes each phase a place to push back.
Superhuman-style keyboard-first: a supervisor of many agents needs speed +
muscle memory. Every event persisted to SQLite as an audit trail.

## What Concerto's WaveChat should adopt (ranked)
1. **Typed, per-item rendering with status badges — not a generic message log.**
   Dedicated renderers for tool calls, diffs, plans; per-item state badge;
   in-progress vs done legible at the row level; real diff view; long output →
   preview + expand. Biggest transcript-quality lever.
2. **Inline decisions, wrapping the thing being decided** — single-key
   approve/deny + required reason on deny, in the transcript at that turn.
3. **Unify the composer with steering + denial** — always live; running = Interrupt
   & Send; gated = deny-with-your-text. One input collapses chat + interrupt +
   deny. Debounce the interrupt.
4. **Multi-wave: status = attention** — dense keyboard-navigable table; glyphs for
   autonomous vs gated; archive completed; OS notifications only on gate/attention.
5. **Supervisor-of-the-outer-loop framing, keyboard-first** — home = the multi-wave
   view; WaveChat = where you descend to steer; fastest keyboard path between them.

**Tension:** their model gates *every* risky tool call (deterministic checkpoints,
"don't trust the agent"). Waves are longer-horizon/more autonomous — adopt the
inline-decision + composer *mechanics* wholesale, but make **gate frequency a
knob** (their auto-accept/bypass glyphs are exactly that valve).

Files: `humanlayer-wui/src/components/internal/ConversationStream/EventContent/*`,
`ConversationEventRow.tsx`, `TaskGroupEventRow.tsx`, `SessionDetail/components/
{DenyButtons,CustomDiffViewer,DiffViewToggle,TodoWidget,ActiveSessionInput}.tsx`,
`StatusBadge.tsx`, `SessionTable.tsx`.
