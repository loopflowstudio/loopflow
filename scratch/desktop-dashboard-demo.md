# Desktop Dashboard demo notes

Earlier attempts and resolved findings: `f492be7f19:scratch/desktop-dashboard-demo.md`
(October 18–19) and `000d5f0970:scratch/desktop-dashboard-demo.md` (October 20 store-copy
demo: titlebar text, Started labels, Dashboard trail — fixed; the trail fix and LOO-426 Start offer await demo).
Still open: live `desktop state`/control target `com.loopflow.mac`, so a separately
identified preview cannot show CLI↔native round-trip parity. Not yet exercised: jump-bar
menus, Machine control, Dashboard arrangement. f492be7f19 (draft builds stop handing off
to the installed lf) fixes a bug also on main; keep it in the PR notes.

## Golden-grid demo — October 20, 2026 (000d5f0970, store copy, Jack Heart)

Worked: white titlebar text, Started/Paused labels, paging, focus growing a band.

1. Overflow (LOO-389/416 clipped), 3. LOO-539/520 overlap, 4. headers losing the
   identifier: content that could not shrink was centered over its slot frame and spilled
   both ways. Fixed: each slot is a hard top-leading frame clipped at its edge; TaskHeading
   truncates the title first. Demo judges whether tile border plus pane border reads doubled.
2. Full-window empty states. Fixed: tiles set `\.compactPanes`; `PaneEmptyState` and
   `SessionSlotOffer` show one line, state plus action, explanation in help.
5. `renderedGoldenSlotsFitTheirBoundsUnfocusedAndFocused`: grid hosted at 1600×1000,
   unfocused/top/bottom focus, beside a tile demanding 1400pt; in bounds, no overlap,
   slot 1 at 61.8%. Fails on the old centered frame.
6. “No primary Session” with no action. Fixed: recorded Sessions → Open conversation
   (navigates to the Task); nothing to open → Start offer.
7. LOO-312 “Server connection could not be restored · Reconnect failed” is Codex 0.163.0's
   TUI losing its `codex app-server` connection. Not the store copy: transport, `lf session
   connect`, app-server and TUI were all started October 18 by installed lf `08d39395…` and
   remain alive; Resume reattached that transport's screen. Real provider reattach gap,
   LOO-416's lifecycle; lf reports the Session live because its processes are.

Animation: focus and page turns share one 0.25 s spring. A representable takes its final
frame at once, so a terminal regrids once per change; demo judges whether that flickers.

## Golden-grid re-demo — October 21, 2026 (a5af466979, store copy, Jack Heart)

8. Jack: “scrolling speed seems pretty slow.” `GhosttyTerminalView.scrollWheel` forwards
   raw `scrollingDeltaX/Y`; Ghostty's own macOS view doubles precise (trackpad) deltas
   before `ghostty_surface_mouse_scroll`. Match Ghostty: ×2 when precise. Applies to every
   terminal, not only Dashboard tiles. Jack: “its not just slow its also choppy movement.”
   Ghostty's view also packs the momentum phase into the scroll mods (`momentum << 1`);
   ours sends only the precise bit, so inertial scrolling arrives as plain events. Match
   both, then check whether SwiftUI work per scroll event (tile re-render) adds stutter.
   Fixed: ×2 precise deltas and `momentum << 1` in the mods. The scroll path writes no
   observed state (no scrollbar callback is handled), so tiles do not re-render per event;
   demo judges the feel.
9. Jack: “Opening a conversation opens the conversation in a focused mode, i want it to
   open inline in the dashboard.” Dashboard tile **Open conversation** calls
   `openTaskConversation`, which runs `openTaskDestination` and sets `.terminals`, leaving
   the Dashboard. From a tile it must attach the Task's conversation in that tile and stay
   on the Dashboard; the zoom control remains the explicit way into the Task.
   Fixed: `DashboardConversationOffer` calls `attachTaskConversation`, which prepares the
   primary Session and places it in the tile's retained workspace without selecting;
   the tile shows any attached Session pane before the plan names it primary.
   `dashboardTileOpensConversationInPlace` passes.
