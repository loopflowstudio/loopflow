# Wave Chat linked references (W2-174, serial PR 2)

PR 1 (#934) landed durable-history-before-refresh. This PR handles the third
Chat design obligation: typed references in message bodies render as inline,
interactive objects instead of dead text — no detached context chips.

## User-visible outcome

In Wave Chat, `W2-174` and `PR #889` inside an authored sentence become inline
links. Clicking one opens a compact popover disclosing the reference type and
identifier plus one action: a Task navigates to its detail in the plan pane; a
PR opens on GitHub. Plain prose selection is unchanged.

## Source of truth

The message text itself. Detection is a pure function over the string; no new
store, DTO, or wire field. `parseChatReferences(in:)` returns typed spans.

## Detection (`Loopflow/Models/ChatReference.swift`, shared + tested)

- **Task** — Linear issue key `[A-Z][A-Z0-9]{1,4}-[0-9]{1,6}`, word-bounded,
  minus a documented denylist of common technical collisions (`UTF-8`,
  `SHA-256`, `ISO-8601`, `GPT-4`, …). Heuristic, same class GitHub/Linear accept.
- **PR** — `PR #889` / `PR#889` / bare `#889`. Canonical id is the number;
  display keeps the authored form.
- Enum carries `.project` / `.evidence` for future detectors; only Task + PR
  are wired now (the proof exercises issue + PR references).

## Rendering (`LoopflowMac/Views/ReferenceTextView.swift`)

One `NSViewRepresentable` over the existing autosizing selectable `NSTextView`,
replacing both the assistant text segment and the user-turn `Text`. Reference
ranges get a `.link` attribute (`x-loopflow-ref://kind/id`) + accent styling;
selection, copy, and drag are preserved. A link click presents an `NSPopover`
anchored at the range's bounding rect. No references ⇒ identical to before.

## Actions

- Task → `onSelectChild(WaveWorkSelection(kind: .task, id: key))` (existing hook;
  highlights the task in the plan pane).
- PR → `https://github.com/<owner>/<repo>/pull/<n>`, owner/repo resolved once from
  the wave repo's git origin (local `git remote get-url`, no network). If origin
  can't be resolved, the popover discloses the reference without an external link.

## Proof

- `ChatReferenceTests`: issue keys, PR forms, mixed message, denylisted
  negatives, no-reference fast path, GitHub URL construction.
- `ReferenceTextView` attributed-string builder test: reference ranges carry the
  link attribute and accent color; plain ranges don't.

## Exclusions

Project/evidence detection, Linear deep-links (no reliable workspace slug),
thread re-structuring, failure rollups (PR 1), Supervision/Control.
