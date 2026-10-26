# LOO-597 findings (pre-repro)

Two lf-emitted sequences explain both symptoms; neither comes from the provider.

1. Screen clear on attach — `rust/loopflow/src/transport/terminal.rs:46`
   `snapshot()` sends `ESC[?1049l ESC[3J ESC[H ESC[2J` whenever the owner's main
   screen has content, then replays the owner's history. Claude Code draws inline
   (main screen), so `lf resume`, `lf connect`, takeover and any attach that lands
   after the provider printed wipes the visible screen *and scrollback* (`3J`).

2. Vanishing exit hint — `rust/loopflow/src/transport.rs:905` (`RawTerminal::drop`)
   Order on exit: provider's own `?1049l` (kept by `NativeClient::controls`), then
   lf's "Left … Continue with: lf resume …" line, then `RawTerminal::drop` writes
   `ESC[?1049l` again. 1049-reset restores the cursor saved at 1049-set (DECRC) in
   xterm-family terminals even when already on the main screen, so the cursor jumps
   back above the hint; the shell prompt redraws there and ZLE clears below it.

Open: confirm both in Jack's terminal (cmux/Ghostty) before fixing.

## Confirmed and fixed (2026-10-25)
Jack's capture of a fresh `lf` launch showed both sequences. Fix on branch:
- `snapshot()` scrolls the viewer's screen into scrollback (no `3J`/`2J` on content).
- `RawTerminal` tracks alternate-screen state from what it wrote; drop sends
  `?1049l` only while still entered.
Next: Jack tries `target/debug/lf` for launch, exit, `lf resume`; then clippy + PR.

## Concept review (2026-10-25, with Jack)

Experience Jack asked for: `lf` behaves like any command in Jack's terminal.
Output flows from the cursor; leaving puts lf's last line directly above the
prompt Jack was at. Nothing printed earlier is erased.

Mismatch: the transport treats every viewer as a reattaching remote screen
(tmux model). The owner spawns the provider before any viewer connects
(`transport::serve`), so even the launching `lf` gets a repaint snapshot and
races the provider's first output. The exit line goes through the owner's pty
stream, then the viewer restores the terminal after it — the ordering that
erased the hint.

Proposed contract:
1. Launch is not an attach: the launching viewer is connected before the
   provider starts and receives the raw stream. No snapshot, no repaint.
2. lf's exit line belongs to the viewer: the owner withholds the provider's exit
   text (as now) and sends the line with `NativeEnded`; the viewer restores the
   terminal, then prints it — the path Ctrl+] detach already takes.
3. Reattach (`lf resume`/`connect`/takeover of a live Session) repaints. OPEN:
   what it repaints (see below).

Decided (Jack, 2026-10-25): reattach redraws only the visible screen (b); the
viewer's screen scrolls into its scrollback, owner history is not replayed.
Implemented in `snapshot()`. Jack demoed the branch build: hint stays, no clear.
Proposals 1–2 remain proposed (not yet accepted or built).

Options considered for reattach:
- (a) owner history replayed into scrollback (current branch fix): full
  transcript scrollable, but each resume appends up to 2000 duplicate lines.
- (b) only the visible screen; earlier transcript lives in the provider's view.
- (c) alternate screen while attached; exit restores Jack's terminal exactly.
  Breaks Claude Code's inline scrollback while attached.
