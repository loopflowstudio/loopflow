# Open choices (e166ff)

Made in kickoff with nobody present; each is reversible.

- **iPhone ramp values.** display 34, title 26, lede 20, text 17, textStrong
  17, meta 13, mono 14, terminal 12. The brief fixes only the 12 pt floor and
  44 pt targets.
- **Pairing screen loses its three-icon row** (desktop ↔ phone). The headline
  "Your work, within reach." stays word for word.
- **"What do you want?" is `.serif(.display)` on the phone.** DESIGN.md's ramp
  puts it on `display`; the Mac's `NewTaskSheet` ships it on `title`.
- **The Mac's own empty and failed states are not moved to the new shared
  state view in this Task.**
- **Simulator look for the Task list, a Task and New Task needs a paired
  Machine** (`~/.lf/mobile-same-device` plus mobile access on this Mac). Build
  does not turn those on. If they are off, those three screens are checked
  through the phone tests' stand-in client and the live look moves to demo.
- `swift/README.md` describes a `--fixture` launch argument for the
  LoopflowiOS scheme that no code implements; the plan deletes the sentence
  rather than building the fixture.

Made in concept review with nobody present; each is reversible. The page is
`scratch/concept-review.html`.

- **The phone's status words become the Mac's.** "Running" → "Busy", "Ready"
  → "Available", "Open" and "No conversation yet" → the lifecycle word
  (Idle, Active, Moving, Unstarted). Passed over: keeping the phone's words
  (two vocabularies for one Task), and moving the Mac to the phone's (the
  prototypes say Available, Blocked, Moving).
- **The phone's filter names stay** ("Ready for you", "Working now"), so the
  "Working now" list holds rows that read "Busy".
- **The phone says "Auto-Ship" / "Stop Auto-Ship"**, the Mac's and the
  prototype's words, in place of "Auto on" / "Auto off". The row still says
  "Auto" and Rust's sentence says "Auto stands in…", on both surfaces.
- **The phone gains "Run"** for a Task stopped or waiting at a step, in a
  "Task actions" menu with Auto-Ship. Passed over: showing the sentence with
  no way to act on it.
- **The workflow sentence wraps on the phone** (two lines at 17 pt for a
  66-character sentence on an SE), which costs terminal rows. Passed over:
  one truncated line, and setting it in `meta`.
- **State titles keep today's one-line messages**; the state view's sentence
  is optional. "Disconnected" is the one new title.
- **The row's status written once in Rust** (word, tone, since, ask on the
  list reading) is left as its own Task; not filed.

Made in write-docs with nobody present; each is reversible.

- **The state view is named `StateView`** and the platform's control height
  `HitTarget.control`; the plan had left both unnamed.
- **The phone's Notice reads "LOO-5 Auto-Ship requested"** as the plan says,
  while the Mac's reads "LOO-5 Auto on requested · Undo". Open: which wording
  both surfaces use.
- **DESIGN.md gains a Desktop pointer** under "Two of the same thing" for its
  hand-made empty and failed states, since the phone will use `StateView`.

Made in implement (slice 1) with nobody present; each is reversible.

- **The row's status type is `TaskRowStatus`, not `TaskRow.Status`.** The
  trailing slot makes `TaskRow` generic, and a type nested in a generic is a
  different type per trailing view.
- **A status that could not be read keeps the Mac's info mark**
  (`TaskRowStatus.unread`), separate from the phone's word "Unavailable"
  (`.unavailable`, a Task whose reading lists reasons). They are two facts:
  the Mac's is a failed Sessions read, the phone's is the Task's own reading.
  Passed over: the word on both, which changes every Mac row when Sessions
  cannot be read.
- **14 pt calls**: a warning or a sentence the screen depends on went to
  `text` (17); hints, footers and paths in secondary ink went to `meta` (13).
- **The reply button's arrow** is `lede` at `imageScale(.large)`, about 26 pt
  where it was 32; its 44 pt frame is unchanged.
- **A phone row with neither a conversation state nor a lifecycle state in
  its reading shows no status word.** The old row said "Open".
