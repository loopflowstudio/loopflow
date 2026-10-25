# LOO-519 — Agents read back and script Desktop navigation

Status: **v2 implemented, including `enter` tile addressing; the Dev demo remains** (implement, 2026-10-25). Jack Heart's concept review (2026-10-24) set the scope: **A**, a
Rex-aligned `lf desktop` CLI, plus **B**, designated session/shell/browser panes.
Jack's v2 direction (2026-10-25, concept review of PR #1677) replaced the Apple-event
transport and the `-t`/`-b`/`do` surface; this plan describes v2 only. v1 text lives
in git (`c4c9ff9e2e:scratch/…navigation.md`). Jack waived design review; open
assumptions are in `scratch/questions.md`, judged in the demo. LOO-576 owns sidebar
scope; LOO-593 owns Desktop server architecture beyond this API; LOO-594 refines the
Wave view. LOO-568's follow-ups were folded in on 2026-10-24; LOO-453's `wait` and
`events` moved here on 2026-10-25.

## Problem

An agent arranging Jack's work through Desktop cannot fully verify what it did,
cannot tell "no such pane" from "Desktop unreachable" by exit status, cannot drive
the gestures a person uses, and from inside a Desktop Session has no caller identity.
Pane identity was weak (a free split tree of anonymous panes). Dev targeting from
#1664 has never been seen working.

## Demo (Jack, on Loopflow Dev, from a Dev shell tab)

```sh
lf desktop ls                                  # Tasks in ⌘n order; panes session shell browser; short IDs
lf desktop goto 2                              # what ⌘2 does
lf desktop back                                # what ⌘[ does
lf desktop run 'cargo test -p loopflow desktop'    # new shell tab, line typed, focus stays
lf desktop wait -p shell:2 --for 10m; echo $?  # the command's exit status
lf desktop capture -p shell:2 --trim | tail -3
lf desktop browse http://localhost:5173
lf desktop zoom -p browser --on
lf desktop events --json &                     # NDJSON frames as the above happens
lf desktop zoom -p 99 --on; echo $?            # 3: nothing changed
LF_DESKTOP=com.example.none lf desktop ls; echo $?   # 4: no socket
```

Then Jack asks the session-pane agent to "zoom yourself": `lf desktop zoom --on`, no
`-p`, zooms its own pane. Same demo, from LOO-568: `ls`/`focus` act on Dev, not the
installed app; Jack opens a conversation inline (#1580) and judges the Dashboard
tile's doubled border. Exact flag spellings follow the compiled CLI.

## Decisions (accepted unless marked proposed)

- **Transport (Jack, 2026-10-25): a minimal socket server inside Desktop**, only what
  `lf desktop` needs. Desktop binds `$LF_HOME/run/desktop/<bundle-id>.sock` (0700 dir,
  0600 socket, same-uid `getpeereid`, stale file unlinked) and serves NDJSON
  `{v,id,method,params}` → `{id,result|error{code}}`; methods `inspect`, `perform`,
  `read_text`, `events`; a version mismatch names the fix. Path derives from
  `LF_DESKTOP` (bundle ID); the `LF_DESKTOP=1` fallback is gone. `view` launches via
  `loopflow://` when Desktop is not running.
- **No `--` (Jack).** `run LINE` opens a shell tab and types the line (Ghostty
  `initial_input`); bare `run` opens an empty tab.
- **Explicit state for scripts.** `zoom` requires `--on`/`--off`; only keys toggle.
- **One name per operation (proposed by Jack's "simplify"; Jack confirms in demo).**
  The Swift `DesktopAction` registry's names are the verbs; `do` and dotted names are
  gone; `actions` lists them. `toggle-shell`/`toggle-browser` are key-only.
- **Views contain panes; `-p/--pane` addresses parts of every view (Jack,
  2026-10-25).** View kinds: Task (`session`, `shell`, `browser`), Dashboard (tiles,
  each a Task view), Wave (its conversation plus its Tasks as tiles; simplest version,
  LOO-594 refines). Paths: `-p shell:2`, `-p 3` (third tile), `-p LOO-519`,
  `-p LOO-519/shell:2`. Root `--task`/`--wave` set the base view; subcommand `-t`
  and `-b` are gone. Pane verbs (`hide`, `show`, `move`, `resize`, `zoom`, `focus`,
  `send`, `capture`, `close`, `reopen`) work on any pane; navigation verbs (`view`,
  `enter`, `back`, `forward`, `next`, `previous`, `goto`) change the view.
- **Target precedence:** root Work flags, then the caller (`LF_TERMINAL_ID`, trusted
  for targeting), then the checkout's Task binding. Rex resolution order: full ID,
  label, 1-based position, ID suffix. Nothing follows focus; ambiguous or absent
  exits 3 and changes nothing.
- **IDs for scripts.** Receipts carry `target {window, work, pane, tab, rule}`; `ls`
  prints short IDs; docs say scripts use IDs, positions are for people.
- **`wait` and `events` (from LOO-453).** `events` streams changed windows (first
  frame = every window); `wait` ends when a pane is ready or a tab's submitted
  commands finish (exit = last status, from `GHOSTTY_ACTION_COMMAND_FINISHED`);
  `--for` elapsing exits 1.
- **No migration of existing data (Jack).** `designated_panes.sql` deletes old
  layouts; they reset to the designated three panes.
- **Exit statuses:** 0 · 1 failure · 2 bad arguments/unknown verb · 3 missing or
  ambiguous target · 4 Desktop unreachable (no socket, refused, not macOS) · 130.
  Typed server error codes decide them, never message text.
- **Dashboard pane verbs edit the shared record (proposed):** on a Dashboard,
  `hide`/`show`/`move`/`resize` act on tiles and the receipt says
  `applies_to: dashboard | window`; `arrange`, `slots`, `order` fold into those verbs.
  `enter` (open a tile as full Work) and `zoom` (a pane fills its view) stay distinct.

## What exists (v2, slice 6, 2026-10-25)

- `DesktopServer.swift` + `DesktopConnection` (`desktop.rs`); osascript, `Loopflow.sdef`,
  `ScriptCommands.swift`, `(NNN)` parsing and `LF_DESKTOP=1` deleted.
- Registry verbs: `back forward goto next previous working-tasks enter focus zoom
  hide close show reopen move resize send send-key run toggle-shell toggle-browser
  files graph-log browse page-back page-forward reload switch`; plus `ls`, `state`,
  `actions`, `events`, `wait`, `capture`. `PaneNeed::for_action` maps verbs to the
  pane they need.
- Designated panes (Rust `PaneIntent` Session/Shell/Browser with validation; Swift
  `PaneContent`); browser tabs `files`, `graph-log`, URL (`WKWebView` with page
  controls). ⌘W closes the addressed tab (last tab hides the pane), ⇧⌘T reopens the
  last close with the shell still alive. `destination: entered {from, work}` for a
  tile opened as full Work; `zoomed` means only a pane filling its Task.
- `lf desktop view <expr> --plan` derives the plan without opening Desktop (LOO-497
  reachability check).

## What exists (slice 7, 2026-10-25)

- **Wave view:** the Wave's conversation, its Tasks as tiles (`WaveTiles`, the Wave
  plan's slots) and the inventory below them. `open_work` is the Wave then its tiles;
  `-p N` and `TILE/PANE` address them as on a Dashboard. Tiles never take the
  window's focus, so the Wave stays selected.
- **Tile verbs edit the shared record, written by lf, not Desktop:** `hide`/`show`/
  `move`/`resize` aimed at a tile (no pane part) write a saved Dashboard's arrangement
  or slots, or the repository Task order on Working Tasks and Wave views (`move`
  only; the others exit 1). Each edit uses the record's current revision. Desktop
  follows through its Work stream. Receipts carry `applies_to: window | dashboard`.
- **`-p query:ID/task:ID` and `-p working-tasks/task:ID`** name a saved record's tile
  by Work key and edit it without Desktop, on any platform; Desktop's own tile drags
  and hide buttons use them. `layout query:ID [--save --revision N]` reads or writes
  a Dashboard's slots (the slot editor's add/remove/swap).
- Deleted: `lf desktop arrange`, `slots`, `order`, `DesktopOrderResult`,
  `RegistryQuery.saveDashboardArrangement`, `WorkModel.arrangeDashboard`/`hideInDashboard`.

## Concept review, 2026-10-25 (Graph, after slice 7)

- **`enter` takes a tile, in `-p` grammar, on every tiled view.** Built
  (implement, 2026-10-25): lf resolves `enter TILE` with the `-p` tile resolver
  (`Scope::entered_tile` → `tile_work`: full ID, label, 1-based position, ID
  suffix) on Dashboard, Working Tasks and Wave views, and sends Desktop the full
  Work ID; elsewhere it exits 3 and changes nothing. The bespoke Swift resolver is
  gone. `DesktopBoard`/`WorkNavigation.Board` gained `wave(id)`, so a Wave tile
  click and `enter` both produce `entered {from: {wave: ID}, work}`, the trail
  shows the Wave, and back returns to it (a Wave tile click used to plain-select
  the Task). `WorkModel.enterTile(work, origin:)` reads the board from the
  window's destination, so tile clicks and `enter` share one rule; a tile on an
  unsaved query is plainly selected.

## Remaining

1. **Demo on Dev** (Jack).

## Done when

- `lf desktop ls` in a Dev shell lists Dev's sidebar Tasks in ⌘n order with each
  Task's panes and short IDs; `state --json` reports the same.
- `goto`, `back`, `enter` produce exactly what ⌘2, ⌘[ and a tile click produce, and
  the receipt's destination and target equal the next `state`.
- `run LINE` adds a shell tab running LINE; `wait` returns its exit status;
  `capture -p shell:N` returns its output; `browse URL` shows the page.
- `move`/`resize`/`zoom --on`/`hide`/`show` rearrange panes in a Task view and tiles
  in a Dashboard or Wave view (`applies_to` says which); a relaunch restores both.
- `events --json` emits a frame for each of the above.
- An agent in the session pane runs `lf desktop zoom --on` and its own pane zooms.
- Unknown pane/tile → 3, nothing changed; unknown verb → 2; no socket → 4.
- From a Dev shell, `focus` moves focus in Dev, not the installed app (Jack, demo).

## Checks

Gate, headless, inherited `LF_*` unset, `< /dev/null`: `cargo test -p loopflow --lib
commands::desktop selection`; integration `dto_fixtures::desktop cli_discovery
reference_matches_the_compiled_cli`; `cargo clippy --all-targets -- -D warnings`;
`scripts/test_desktop.sh --filter 'DesktopServerTests|DesktopPaneControlTests|DTOFixtureTests|WorkNavigationTests|WorkspaceRetentionTests|DashboardWorkspaceTests|WorkDestinationTests'`.
Demo/review (Jack): Dev targeting, inline conversation, doubled frame, pane feel.

Last check (2026-10-25, compress `enter` tiles): Swift
`DesktopServerTests|DesktopPaneControlTests|WorkNavigationTests|DashboardWorkspaceTests` 79 pass; Rust unchanged since implement (`--lib commands::desktop` 12, `--lib selection` 63 pass).

## Exclusions

Sidebar scope and header animation (LOO-576); server roles beyond this API and a push
signal for `events` (LOO-593); Wave view refinement (LOO-594); richer keys (LOO-452);
OSC 7501 (LOO-422); external hosts (LOO-498); side-by-side shells; flattening Files'
document tabs into browser tabs; Lua, cwd/cross-repo pane moves; the doubled frame
unless Jack picks a fix in the demo. LOO-571 (`LF_DESKTOP=1` removal) closes with
this PR.

## Risks

- **`designated_panes.sql` is a released-store change** that deletes layouts and their
  file drafts; a branch build on Jack's lfdb raises the reader floor. Dev runs against
  a copied store until release.
- **Trusting `LF_TERMINAL_ID`** lets an inherited marker target a pane; blast radius
  is one pane of the same user, and a gone pane exits 3.
- **`wait` depends on the shell's OSC 133 integration**; a shell without it never
  finishes and `--for` is the only exit.
- **`events` polls every 250 ms while subscribed**; acceptable until LOO-593 designs
  a push signal.
