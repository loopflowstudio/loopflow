# Wave Chat narration curation — make it fire on real turns

## Finding

#950 shipped `turnPresentation`, which curates a turn into a `conclusion` plus
`commentary` **steps** by iterating `turn.items` for `.message(phase: "commentary")`.

But the shared fold rule `ChatTurn::absorb_item` (and its Swift mirror
`ChatTurn.absorbing`) folds **every** message — any phase — into `turn.text`:

```
Message{phase:"stream"} -> text.push_str   (streamed conclusion)
Message{phase:_}        -> push_text        (newline-joined into text)   <-- commentary too
_                       -> items.push
```

So a `commentary` message never reaches `turn.items`. `turnPresentation` iterates
items, finds none, and every turn reads as full-text with zero steps. The
curation is **inert on real wire data**.

The real journals disagree that this is theoretical: `"phase":"commentary"`
appears **81 times** (46 in the Product Wave), each a discrete `turn_item`
message the provider tagged as operational narration. They all fold into text
today.

## Fix (this PR)

Route `commentary` messages to `items` in the one shared fold rule, both
languages. `stream` and every other message (final_answer, untagged) keep
folding into `text` (the conclusion). No wire-shape change; the journal events
are untouched — only reconstruction changes, so old turns retroactively curate.

- `rust/.../chat/turns.rs` `absorb_item`: `commentary` -> items.
- `swift/.../ChatTurn.swift` `absorbing`: `commentary` -> items.
- `rust/.../lf/commands/thread.rs`: `lf chat --follow` renders only `turn.text`;
  render `commentary` items as `wave ›` prose lines so the CLI does not silently
  drop the 81 narrations that used to live in text.
- Docstrings that claim "a Message folds into text" corrected.

`turnPresentation` needs **no change** — it already consumes commentary from
items. This is the missing producer-to-projection link.

## Still deferred (honest boundary, unchanged)

Pure token-streaming turns (Claude harness) emit prose only as `stream`
fragments and tag nothing `commentary`, so they still read whole. Splitting a
streamed turn's operational preamble from its conclusion needs the harness to
tag streamed segments — a producer signal no harness emits today. That is the
next follow-up; the conclusion/steps seam is where it plugs in.
