# LOO-519 demo on Loopflow Dev (2026-10-26)

Experience reviewed: `lf desktop` scripting Loopflow Dev over its socket, at PR #1677
head `139cd84e1d` plus two uncommitted demo fixes (below). Design:
`scratch/jack-loo-519-let-agents-read-back-and-script-loopflow-desktop-navigation.md`.
Decisions waiting on Jack: `scratch/questions.md`.

Setup: `scripts/loopflow-dev.py run` from the LOO-519 checkout. The branch carries a
draft migration, so Dev works on a store copy (`~/.lf/copies/5aa04e50140ca08e`, first
snapshot took 2½ minutes); the layout reset happens there only. Socket:
`~/.lf/run/desktop/com.loopflow.mac.dev.sock`.

## Agent-run proof (Claude, before handing to Jack)

Worked:
- `state`, `ls`, `goto 7`, `run LINE` (receipt returns the new tab ID), `wait -p shell:N`
  (exits with the command's status: `false` → exit 1), `capture -p shell:N`,
  `zoom --on/--off -p session`.
- **Dev targeting**, after fix 2: inside a Dev shell, plain `lf desktop zoom --on -p session`
  printed `LF_DESKTOP=com.loopflow.mac.dev`, exited 0, and Dev's `ls` showed
  `session zoomed`.

Fixed during the demo (uncommitted):
1. `swift/LoopflowMac/ThirdPartyNotices/inventory.json` — the branch changed
   `swift/Package.swift` (dropped `Loopflow.sdef`) without refreshing the inventory, so
   Dev install failed in `third_party.py package`. CI's test jobs were skipping on #1677,
   so nothing caught it. Regenerated with `scripts/third_party.py refresh`.
2. `swift/LoopflowMac/LoopflowApp.swift:10` — `enrichProcessPathForGUILaunch` copied only
   `PATH` and `LF_BIN` into the process environment, so Ghostty shells never got
   `LF_DESKTOP` and `lf desktop` from inside Dev addressed the installed app
   (`com.loopflow.mac`, exit 4). Also broken on main since #1664. Added `LF_DESKTOP`.

Found, not fixed (next implement pass):
- **Receipt IDs don't address.** `run` returns `tab: <UUID>`, but `-p shell:<UUID>`,
  `-p shell:<suffix>` and `-p shell/<UUID>` all exit 3. The error lists
  `shell/<UUID>` as a choice, which then parses as a tile. Contradicts v2 "receipts
  return IDs; scripts use IDs". Only positions (`shell:3`) work.
- **`-p shell` doesn't reach the selected tab**: `capture -p shell` → "The addressed pane
  is not a terminal."
- **`goto` takes only a position**: `goto LOO-519` exits 2 (Clap `<INDEX>`), unlike Rex
  target resolution everywhere else.
- **`run` refuses a Task without an open workspace** ("has no workspace in this Desktop
  window; nothing changed", exit 3) though it resolved the Task from the checkout;
  `focus` opens the retained workspace in the same situation.
- Tab labels are empty (`shell/<UUID> ()`); `ls` columns shift at two-digit positions.
- Wrong-socket error says `Loopflow Desktop (<path>) … not serving <path>.sock` when
  `LF_DESKTOP` holds a path; it expects a bundle ID.

## Jack's hands-on review

Jack, after trying Dev (2026-10-26): "looks cool ship it". He made no specific comment on
⌘W/⇧⌘T, pane rearranging, the doubled Dashboard frame or the inline conversation, so
the implemented behavior stands (see `scratch/questions.md`).

Next action: ship #1677 with fixes 1–2. The found-not-fixed items above go to a
follow-up Task; its proof is `run` → `wait -p shell:<receipt tab ID>` → `capture` with
the receipt's ID alone, and `goto LOO-519` by ID.
