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
