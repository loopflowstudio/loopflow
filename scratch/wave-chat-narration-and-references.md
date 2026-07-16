# Wave Chat: narration curation + Project/evidence references (W2-174, serial PR 3)

Stacked on PR #947 (inline Task/PR references). Carries the rest of W2-174's
Chat contract. Rollups (contract item 4) verified already done on main (#934).

## What ships here

### 1. Typed Project + evidence references (authored contract)

Real Product Wave narration names Projects and evidence in prose with **no
unambiguous identifier** ("Loopflow API Project", "generation 2", "the delta-wire
design"). Per the directive, rather than silently excluding those types, define
the smallest **authored** typed-reference contract:

- `project:<slug>` → Project reference. `<slug>` is the PM identifier
  (`mac-surface-ux`, `wave-chat`, …). Click navigates to the Project in the plan
  pane (existing `onSelectChild(kind: .project)` hook).
- `evidence:<token>` → Evidence reference. `<token>` is opaque (a commit, run,
  receipt, or KR key). Disclosure-only popover — evidence has no single
  navigable target, so we type and preview it without fabricating a link.

Both extend the PR #947 parser (`ChatReference`) and `ReferenceTextView`. Task
(`W2-174`) and PR (`#889`) keep their natural syntax; only the two ambiguous
kinds require the explicit prefix. Producers (wave skills/prompts) opt in by
writing the prefixed form; unprefixed prose is left alone (precision over noise).

### 2. Narration curation — the phase-aware conclusion/steps seam

`turnPresentation` gains `conclusion` + `steps`. Messages tagged `phase:
"commentary"` are operational narration → moved behind a "Show steps" disclosure;
everything else (stream text, `final_answer`, untagged messages) is the
conclusion shown in the thread. `prose`/`hasProse` stay unchanged so the failure
rollup and its tests are untouched. MessageRow renders the conclusion prominently
and discloses steps only when present. The raw record is never altered.

## The honest boundary (recorded, not hacked)

Streamed prose can NOT be structurally curated in the Swift projection today.
`ChatTurn.absorbing` merges every `phase: "stream"` fragment into `turn.text`,
dropping its interleave with the hidden `command` items (verified on the real
journal: turn-22979 = 345 stream frags + 3 commands → one text blob). So the
"operational narration between tool calls vs. trailing conclusion" split isn't
recoverable from the wire turn, and content-regex prose stripping is forbidden
(WaveChatTranscript: "prose and decisions are the conversation").

The right-altitude fix is producer/client-side: preserve prose-segment structure
(stop merging `stream` into `turn.text`, or tag the operational preamble with a
distinct phase) so the same conclusion/steps seam curates streaming turns too.
That is a Loopflow-API/wire change, not a Swift chat PR — deferred with this note.
The seam built here is exactly where that producer signal plugs in.

## Proof

- `ChatReferenceTests`: `project:<slug>` and `evidence:<token>` detection,
  bounded matching, mixed with Task/PR, no false positives on bare words.
- `WaveChatTranscriptTests`: a real-journal-derived turn (commentary + stream +
  final_answer) splits into conclusion vs steps; a pure-stream turn keeps full
  prose as conclusion with no steps.
- Real Product Wave history: fixtures are extracted from the live journal, not
  synthesized.

## Mechanics note

`lf pr land`/`lf pr next` are blocked by the incompatible local store (they
launch an agent for PR copy → trace capture fails). This PR is therefore opened
manually via `gh`, stacked on #947. Reported once; work continued inline.
