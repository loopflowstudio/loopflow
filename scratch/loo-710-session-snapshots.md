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

- **Gate and CI.** The concept-review repairs are implemented and focused PTY/CLI
  proofs pass. Gate retains the affected transport and Session suites, including
  `transport_tests`, `agent_startup_tests`, `session_cli_tests`, the interactive
  Graph input proof and disposable installation proof named in the tests README.
  Four-target package linking remains CI's obligation.
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
README paragraph on vt100 limits. The concept-review repair also removes
`human_session::paint_last_screen` (unguarded stdout) and `terminal::untouched`
(text-only freshness), preserving optional snapshot reading and truly fresh
viewers. `AlternateScreen` stays: it tracks what a viewer's terminal was sent.
The cleanup guard now owns output and modes together in `ViewerState`; no cloned
stdout descriptor or unguarded input-hint writer remains. `ViewerTerminal` names
both its cooked preview and raw attachment phases. The isolated fixtures reuse
`transport::terminal_pair` instead of maintaining another `openpty` helper.

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

## Concept review — 2026-11-02

Implementation repair (2026-11-02): transport now owns the saved display before
startup and carries its guard through live attachment. Drop and signal interruption
share cleanup; input stays cooked until acknowledgement. The live owner clears
the preview even when blank. Freshness now means no output received, preserving
mode-only changes through the full formatter. The PTY proof caught application
cursor keys surviving cleanup; explicit cursor-key/keypad resets repair that too.
The CLI fixtures reopen a real fixture Session with a primary-screen provider and
with the provider absent, preserving its title/history without replaying input.

Recovery/compress check: `cargo test -p loopflow --lib -- transport::viewer_tests:: transport::terminal:: transport::native::` — 17 passed; `pytest python/tests/test_loopflow_dev.py -q` — 9 passed (both network-isolated); `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings` passed; affected-suite gate and four-target package CI remain.

**Outcome: design changed; next implement direction below. No unresolved product
choice is needed for these repairs.** Jack Heart's accepted experience remains:
reopen the same Session, see its last screen while the provider returns, then
continue with the live owner's screen and input modes. An implementation
assumption that Claude draws on the primary screen cannot narrow that contract.

The person has one action, `lf resume SESSION`. The concepts remain Session
(durable conversation), owner (live terminal and provider), viewer (temporary
terminal control), and snapshot (the last display retained after an owner ends).
There is no additional snapshot lifecycle to operate, no new DTO field, and no
new CLI switch. `--prepare --json` remains preparation only; the command it returns
uses the same resume path when Desktop executes it. This does not require a
GhosttyKit decoder or a Desktop UI change.

### Consequential mismatch: the preview changes a terminal nobody owns

Existing: `human_session::open` calls `paint_last_screen` before
`transport::delegate_current`. That helper writes the full live repaint straight
to stdout, including alternate-screen and input modes. Later,
`ViewerTerminal::enter` starts with an empty `AlternateScreen` tracker; it observes
only subsequent transport output. Failure before attachment has no such guard at
all. A saved alternate screen followed by a failed launch, or a provider that
never toggles alternate-screen mode, can leave the shell on the saved alternate
screen. The caveat in `questions.md` was an implementation concession, not an
accepted product exception.

Next implement direction (proposed implementation, implied by accepted behavior):
transport owns both preview output and its handoff to live output. Establish
cleanup before emitting the first preview byte; track every emitted mode change
through that same ownership boundary. At handoff, the new owner's state becomes
authoritative, including when it has drawn nothing yet. On cancellation, failed
launch, detach or exit, restore the caller's usable terminal. Remove the separate
raw stdout painting path rather than adding a second cleanup mechanism around
it. Keep the snapshot reader and Session-keyed file shared with LOO-709. The
preview must not seed the new owner's parser or replay an old continuation/input
into the new provider; it is retained display, not resumed execution.

Related live-state mismatch: `terminal::untouched` checks only text, cursor and
scrollback. An owner can enable bracketed paste, application keys or Kitty
keyboard state while its screen remains blank and its cursor remains home.
`repaint` then returns only style/cursor resets and loses the accepted input
state. Preserve the no-drawing behavior for a truly fresh owner while restoring
mode-only changes. Do not infer an untouched terminal from blank text alone.

### Normal path, failure, recovery and next proof

- Normal: attach to a live owner, preserving text, links, history and input modes;
  attach after mode changes but before text exists and get the same modes. A
  truly fresh owner still preserves the viewer's prior screen/history.
- Failure: leave an alternate-screen snapshot, then fail provider startup or
  cancel before attachment acknowledgement. Returning to the shell must restore
  terminal modes and leave alternate screen correctly; preview bytes must have
  been inside the cleanup boundary even when no acknowledgement arrives.
- Recovery: reopen that same Session again, paint its snapshot, and hand off to a
  provider that draws on the primary screen without ever sending `?1049l`.
  Its new output must appear on the primary screen, and subsequent detach/exit
  must not revive the preview's modes. No provider input or Graph step is replayed.
- Missing, corrupt or revision-mismatched snapshots remain optional display:
  continue the ordinary resume path without changing Session/provider history.
  Preserve the file and atomic replacement behavior; no migration or schema is
  required. Snapshot errors must not acquire process-control authority.

Prove these counterexamples through the actual resume/transport handoff and
terminal-state assertions, with a fixture provider and isolated terminal. Existing
formatter round trips do not cover that boundary. Retain existing fidelity and
transport acceptance checks; do not rerun them in this documentation-only review.
The four-target package matrix remains CI's proof obligation. Existing bounds
(history size, palette/images, inactive-screen repaint) remain visible in the
README; this review supplies no new waiver of Jack Heart's acceptance criteria.

Proposed usage wording after implementation: “`lf resume SESSION` shows the saved
screen while the provider starts. The live owner takes over its display and input
modes. If startup fails, the terminal returns to its ordinary state; resume the
same Session again.” Keep existing `--prepare` guidance unchanged. No skill
instruction changes are needed because this adds no action or concept to learn.
