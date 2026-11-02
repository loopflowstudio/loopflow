# LOO-710: Session snapshots at Rex fidelity with libghostty-vt

Status: **accepted** (Jack Heart's Task brief, 2026-11-02; mechanisms below are
this plan's choices, implement 2026-11-02).

## Outcome

Returning to a conversation shows the screen its owner had: colors, hyperlinks,
alternate screen, keyboard modes, and scrollback. The owner keeps a libghostty-vt
terminal instead of `vt100`, repaints viewers through Ghostty's VT formatter, and
writes a Ghostty-format (`GHOSTSNP`) snapshot when it ends so `lf resume` and
`--prepare` can paint the last screen before the provider is back.

## Decisions

- **Prebuilt per-target archives, no Zig in cargo or CI.** `scripts/loopflow-dev.py
  ghostty-vt-build` cross-compiles `libghostty-vt.a` for the four release targets
  from the pinned Ghostty revision with Zig 0.16.0 (`-Demit-lib-vt -Dsimd=false
  -Dcpu=baseline -Dvt-features=-all,+snapshot,+formatter,+selection`), packages each
  as `libghostty-vt-<rev7>-lf<N>-<target>.tar.gz`, and publishes to the `bin` R2
  bucket like GhosttyKit. `build.rs` downloads the archive for `TARGET` with
  `curl`, verifies its pinned SHA-256, and links it statically. The same artifact
  serves `lf`, the test binaries and `loopflow-ffi` (GhosttyKit exports none of the
  `ghostty_terminal_*` symbols, checked with `nm`). `LOOPFLOW_GHOSTTY_VT_DIR` points
  at a local build for offline work. Why not Zig in CI: four runners would each need
  a Zig toolchain and Ghostty's lazy dependency fetch on every build; the archive is
  built once per revision bump, which is already how Desktop's framework works.
  Why not commit the archives: 6.5 MB per target per revision in git history.
  The lf build uses upstream Ghostty unpatched: the embedder patches concern
  command blocks, launch and rendering, none of which an owner's terminal needs.
- **Hand-written `extern "C"` bindings** in `ghostty_vt.rs` for the dozen calls
  used; no bindgen (libclang on every machine) for a stable-enough surface.
- **Repaint shape.** History rows first (VT formatter over a history selection,
  newest 2,000 rows at most and under 768 KiB), then a screen height of newlines so
  the viewer's own scrollback survives, then cursor home and the active screen with
  every formatter extra on (cursor, style, OSC 8, Kitty keyboard, charsets, palette,
  modes including ?1049, scrolling region, tabstops, pwd). On the alternate screen
  only the active screen is painted; the primary's history stays with the owner.
- **Queries answered by the library.** `GHOSTTY_TERMINAL_OPT_WRITE_PTY` collects DSR
  and mode-report replies; a device-attributes callback answers DA1/DA2 as Ghostty
  does. Replies reach the program only while no viewer holds control, as before.
- **Report insertion boundary** uses `GHOSTTY_TERMINAL_DATA_VT_GROUND` and the
  continuation API (64 KiB cap) instead of the `vte`-based `Boundary`.
- **Exit snapshot.** When the owner ends it writes `<Machine home>/screens/
  <session>.ghostsnp` (draft then rename, so a reader never meets a half-written
  stream): one header line `lf-terminal-snapshot/1 ghostty=<full revision>\n` then
  the GHOSTSNP bytes. Format v1 has no compatibility guarantee, so a snapshot from
  another revision reads as absent. `snapshot::leave` runs at both owner ends: the
  `ending` arm the lifetime policy signals (idle, executable removed or replaced,
  and LOO-705's superseded, which PR #1799 routes through the same arm as a
  `Response::Moved`) and the program's exit. `lf resume` on a Session whose owner is
  gone paints the snapshot's screen before launching the provider, when its stdout
  is a terminal. Path and key follow LOO-709 (Jack, 2026-11-02: one path and one
  reader between the two Tasks); no DTO field.
- **VT hyperlinks need a patch.** Upstream's VT formatter emits OSC 8 only for the
  cursor's current link, not per cell (HTML gets anchors). `ghostty-vt-patches/
  0001-vt-hyperlinks.patch` mirrors the HTML path for VT; its Zig test runs in the
  archive build.
- **Scrollback** kept by the owner: 8 MiB (Ghostty's C default is about a page).

## Remaining

- **Meeting LOO-709.** Its branch still writes `screens/<session>.vt` through
  `transport::save_last_screen`/`last_screen_path`, `Output::last_screen` and
  `terminal::last_screen(&vt100::Screen)`, and a `transport_tests` case decodes that
  file with `vt100::Parser`. Both branches call their writer at the same two owner
  ends. Whichever lands second deletes the `.vt` writer and its helpers, keeps
  `snapshot::leave` at both ends, and rewrites that test to decode `.ghostsnp`
  through `Terminal::decode`. No DTO field either way.
- **Meeting LOO-705 (PR #1799).** The `ending` arm's body changes shape (it queues
  one `Response::Moved` instead of an input hint); `snapshot::leave` stays before
  `kill_process` in the merged arm.

## Delete — do not maintain

Done in this branch: `vt100` and `vte` dependencies, `transport/terminal.rs`'s
`Queries`, `Boundary` and `snapshot(&vt100::Screen)` with their tests, and the
README paragraph on vt100 limits. `AlternateScreen` stays: it tracks what a
viewer's terminal was sent. Nothing further to remove.

## Done when

- `cargo test -p loopflow --lib transport::terminal::` and `ghostty_vt::` pass:
  a repainted viewer terminal matches the owner's text, hyperlink, alternate-screen
  and bracketed-paste state; history is painted; an encoded snapshot decodes to the
  same screen; DSR is answered only while detached; a split escape is retained.
- `transport_tests` (gate) pass unchanged.
- Package builds for the four targets link the archive.

Check results: `cargo test -p loopflow --lib -- transport::snapshot:: ghostty_vt::
transport::terminal::` 11 passed after the 2026-11-02 sync onto main (conflict-free)
and the draft-then-rename write; earlier on the same day `cargo test -p loopflow
--test transport_tests` 20 passed, `cargo clippy -p loopflow --all-targets -D
warnings` clean, `pytest python/tests/test_loopflow_dev.py` 10 passed, archives `lf3`
downloaded by `build.rs` from `bin.loopflow.studio`. Package builds for the other
three targets: CI `package-build`.
