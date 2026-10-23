# LOO-568 demo

Build: `~/Applications/Loopflow Dev.app` (bundle `com.loopflow.mac.dev`) from this
branch: main at #1580 plus 5f7815b53e (arrange input drops `mode`). The installed
Loopflow 0.14.14 predates #1580.

October 22: every Dev-build lf call timed out (~30 s copying the 9.7 GB store,
then the semantic check rejected 84 `flow_finished` task_events). Owned by LOO-569
(fix `7bb7a00616`). The Dev build was installed with that commit applied
uncommitted; it runs on copy `~/.lf/copies/2f141eea16a1ff45` (from 12:49), so it
neither sees later shared-store changes nor writes to the shared store.

Judge, in order; record Jack's verdict beside each:

1. Terminal scrolling in a Dashboard tile and an ordinary pane (trackpad, momentum).
   Jack: Working Tasks sidebar scrolling "feels better than when it was under heavy
   load, but still not great." (Sidebar is SwiftUI, not the terminal path.)
2. Tile **Open conversation** places the primary Session in the tile.
3. Tile border + pane border: doubled frame or not.
4. Focus change / page turn: do terminals flicker on the 0.25 s spring?
5. Plan order waiting-first moves ⌘1–9 positions: acceptable?
6. Hide/restore a Dashboard occurrence (exercises the new arrange input).

Not demoable here: CLI↔native agreement (`lf desktop state` targets
`com.loopflow.mac`, so it cannot see the Dev build).

Pending Jack's OK: LOO-497 Linear rewrite — replace the scripting-parity
acceptance bullet with "Scripting parity (exit statuses, scoped context,
send/capture/wait, events) moved to LOO-519."

Found in demo, fixed (Swift passed `--json` to commands without one): creating a
pinned Dashboard and saving a pane layout both failed.

Jack, sidebar scrolling: "stutter while i drag".

Found in the 2:59 PM Working Tasks screenshot:
- LOO-553/554/555 tiles say "Not started · Start" under a "Started" heading. Their
  50+ Sessions are all headless Runs; the tile's fallback (DashboardView.swift:204)
  tests "no Sessions loaded", not `awaitsStart`, so Start would rerun `lf task run`.
- LOO-389 Resume fails: two Codex agent processes (7f8385f0…, 75943fcb…, early
  October) have no pid and no completion in the real store, so they stay "live or
  unresolved" forever. Reattachment/lifecycle owner, not Dashboard code.
- LOO-550 tile: "Acti…" label truncates beside "Move here".

## Next demo (31aa578aa7 + LOO-569 fix uncommitted; copy refreshed 15:47)

1. Sidebar drag-scroll: stutter gone? (Offset no longer observed; rows stopped
   re-rendering per frame.) Jack, after 7aaf276ef1: "scrolling is fixed!"
   (The jump was the ⌘N hover/slot-hint re-render, as Jack guessed.)
2. A started Task with only headless Runs: tile shows "No conversation open ·
   Open conversation", never Start. Opening creates its conversation in place.
3. A narrow tile with "Active in another terminal": icon + Move here, no "Acti…".
4. New Dashboard with Pin in sidebar saves; pane layout saves without a banner.
5. Still unjudged from the first list: terminal scroll feel, doubled borders,
   animation flicker, ⌘1–9 order.

Not fixed here: LOO-389 Resume (140 pid-less unfinished agent Process records;
"unknown evidence blocks" is infrastructure's rule). Dev-only race seen: an
orphaned `lf monitor work` (parent 1) outlived its app and created an empty store
at the copy path during a refresh.

## Verdicts, October 23

- Focus spring feels good, could be slower → 0.35 s (8e5192e585).
- Terminal scrolling: fine.
- ⌘1–9: Jack, "keep the numbers fixed on positions" (positional, as built).
- Pane shadow too heavy, tiles and ordinary panes → lightened (8e5192e585).
- Zoom from a Dashboard tile: trail should pass through the Dashboard and the
  expansion should animate → Working Tasks zoom keeps its trail; full Work grows
  out of the tile (8e5192e585). Jack: "ok this looks good".
