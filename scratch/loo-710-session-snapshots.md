# LOO-710: Session snapshots at Rex fidelity with libghostty-vt

Status: **accepted** (Jack Heart's Task brief, 2026-11-02; mechanisms below are
this plan's choices). Reconciled 2026-11-02 against `bb7c8944a8`; the
preview recovery repairs are retained. This implementation pass addresses the
subsequent review's primary-buffer and program-color gaps. No product decision
is pending.

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
  The lf build applies its VT hyperlink patch below; Desktop's embedder patches
  concern command blocks, launch and rendering and are not applied to this archive.
- **Hand-written `extern "C"` bindings** in `ghostty_vt.rs` for the dozen calls
  used; no bindgen (libclang on every machine) for a stable-enough surface.
- **Repaint shape.** History rows first (VT formatter over a history selection,
  newest 2,000 rows at most and under 768 KiB), then a screen height of newlines so
  the viewer's own scrollback survives, then cursor home and the primary screen
  with formatter extras on. When the alternate screen is active, a disposable
  decoded snapshot reveals primary state; its unfinished parser input is cancelled
  only on that copy. The live owner's active alternate screen is formatted last,
  preserving primary contents and cursor for a later bare return. The owner and
  its continuation are never mutated. Both live attach and saved preview use this
  one path; the transport response remains bounded at 4 MiB.
- **Program colors, viewer theme.** `0002-vt-program-colors.patch` changes VT palette
  emission to Ghostty's explicit override mask, including overrides equal to an
  owner's default, plus foreground/background/cursor overrides. Untouched colors
  stay at the viewer's defaults. Cleanup and preview handoff reset program colors
  to the viewer's configured theme. HTML formatting is unchanged. Archive `lf4`
  carries the patch; its build runs palette round-trip/pin-map and hyperlink tests.
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
  Linux Clippy and four-target package linking remain CI's obligations. These
  checks and the interactive demo are still acceptance work; source fixtures do
  not establish installed/native-provider behavior.
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

Removed: the alternate-screen history skip in `terminal::repaint`,
`palette: false` in `Terminal::format`, and the pre-formatter cursor home that
overwrote the primary cursor before 1049 saved it. Ghostty homes after entering
the active buffer; its palette export now carries only program overrides.
Repaint is one ordered pass through history, primary and optional alternate,
without recursive repaint or duplicate resets. The test child's `Deref` forwarding
and one-use `_ghostty_vt_archive_name` wrapper are also removed. Existing
continuation, preview cleanup, fresh-owner, primary-buffer and rendered-color
proofs remain. No local deletion targets remain; the LOO-709 integration cut above
still belongs to whichever branch lands second.

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
- The affected transport and Session gate suites pass, including the isolated
  startup-failure, pre-acknowledgement cancellation, alternate-to-primary, blank
  and mode-only attachment fixtures. Fresh owners preserve the viewer's screen.
- `transport_tests` (gate) pass unchanged.
- Package builds for the four targets link the archive.

## Recovery reconciliation — 2026-11-02

Jack Heart's accepted experience remains: reopen the same Session, see its last
screen while the provider returns, then continue with the live owner's screen
and input modes. The concept review rejected two implementation assumptions:
primary-screen provider behavior cannot excuse unowned preview modes, and blank
text does not imply an untouched terminal. The historical review and repair
record is retained at `bb7c8944a8:scratch/loo-710-session-snapshots.md`.

The implementation now matches that direction:

- `human_session::open` enters `delegate_with_preview`; transport establishes
  `ViewerTerminal` before emitting saved bytes and retains it through attachment.
  `ViewerState` owns output and mode tracking together; Drop and interruption use
  the same cleanup. Input stays cooked until acknowledgement.
- Handoff resets the preview's modes and clears its display before applying live
  state, including a blank primary screen. It never seeds the owner's parser or
  replays saved input. Application cursor keys and keypad modes are explicitly
  reset along with paste, mouse, focus and keyboard state.
- `Terminal::received_output` replaces the text-only freshness heuristic. Mode-only
  output goes through the full formatter; a genuinely fresh owner without a
  preview leaves the viewer's existing screen and history in place.
- Isolated child-process PTYs cover startup failure, cancellation before
  acknowledgement, blank-primary and mode-only handoff. CLI fixtures reopen a
  real fixture Session with primary-screen output or a missing provider, assert
  mode/termios restoration, and retain title/history without replaying input.
  These use fixture providers, not configured Claude or Desktop.

Missing, corrupt and revision-mismatched snapshots remain optional display.
Atomic replacement, Session identity, the shared `screens/<session>.ghostsnp`
path and both owner-end writers are unchanged. `--prepare --json` remains
preparation only: Desktop executes the returned resume command. No DTO, schema,
CLI switch or GhosttyKit decoder was added. README now explains preview, live
handoff and failure recovery, while retaining the image limit. Primary-buffer and palette limits recorded in
that recovery review are repaired by the subsequent implementation above.

The inspected upstream base is `4e9ade2d04` (#1798). The LOO-705/709 overlap above
remains a second-landing integration obligation, not evidence those branches are
already combined. Infrastructure and its immediate Release child's complete
GOAL/MEMORY files were read; Release's entry-point lesson reinforces the CLI
resume proofs and the remaining package/installed acceptance distinction.


## Concept review after recovery — 2026-11-02

**Outcome: design changed; implemented in the following pass.** The live viewer must carry the owner's
primary screen through an alternate-screen attachment, and preserve explicit
program color changes. No unresolved product choice: these follow from Jack
Heart's accepted same-screen, colors, alternate-screen and scrollback requirements.
The previous preview ownership and mode-only repairs remain valid and complete.

### Experience and concepts

One action remains: `lf resume SESSION`. A Session is the durable conversation;
its owner retains the live terminal; a viewer temporarily displays and controls
it; the snapshot retains display after the owner ends. A person should not have
to learn whether the program happened to enter its alternate screen before
attachment, or request a redraw to recover retained content.

| Action | Existing implementation | Required continuation |
| --- | --- | --- |
| Attach while a program uses its alternate screen | `Output::attach` sends `terminal::repaint`, then reports and the unfinished VT continuation | Initialize the viewer with the retained primary screen/history as well as the active alternate screen, before forwarding live bytes |
| Program returns to primary without redrawing | Raw `Response::Output` forwards the screen switch; the viewer reveals its own prior primary contents | Reveal the owner's primary contents, cursor and modes; keep the viewer's earlier scrollback |
| Reopen an ended Session | `snapshot::last_screen` decodes the file and uses the same repaint path | Show the saved display under `ViewerTerminal`; live state still supersedes it and no saved continuation reaches the new provider |
| Startup fails or attachment is cancelled | The existing guard restores terminal modes | Preserve this behavior, including any additional state the new repaint changes |

### Consequential mismatch: retained state never reaches the viewer

`transport/terminal.rs::repaint` skips history whenever `alternate_screen()` is
true. `ghostty_vt.rs::format` selects only active-screen coordinates. Subsequent
output is forwarded unchanged, with no restoration when the program returns to
primary. A full GHOSTSNP on disk does not repair this: `last_screen` decodes it,
then passes it through that same active-screen-only repaint.

Concrete counterexample: the owner prints `kept primary` and several history rows,
enters `?1049h`, and prints `alternate`. A new viewer attaches and sees
`alternate`. The program sends only `?1049l`. The owner now shows `kept primary`;
the viewer shows its own previous primary screen. The attach-time SIGWINCH is
only a redraw request; a program need not repaint its inactive buffer or handle
that signal. Existing tests assert the active alternate screen but never follow
the later switch back.

The color counterpart is explicit in `ghostty_vt.rs::format`: `palette: false`
preserves a viewer's theme but also drops the program's OSC 4 palette changes.
A program can set an indexed color, draw with it, then detach. A second viewer
receives the color index without its new value. Keeping default theme choices
does not imply discarding explicit program colors.

### Next implement direction and proof

Keep one terminal model and one repaint implementation shared by live attach and
saved preview. Revise the formatter path to initialize the necessary retained
buffers and their cursor/mode state, ending in the owner's active buffer. A
temporary decoded copy may be used to inspect the inactive buffer; do not switch
or mutate the live owner's parser to paint a viewer. Keep the existing history
and transport bounds explicit. Preserve explicit program color values, either
through selective palette restoration or resolved cell colors, while retaining
default theme behavior and terminal cleanup. The exact library mechanism is an
implementation decision; no new public command, DTO, or snapshot lifecycle is
needed. GhosttyKit decoding and Desktop UI remain outside this Task.

Prove attachment during alternate-screen use followed by a bare return to primary:
compare screen text, cursor, input modes and retained history with the owner.
Repeat after encode/decode to exercise saved state. Include an explicit indexed
palette override and compare rendered colors, not merely escape presence. Retain
the fresh-owner, mode-only, failed-launch and cancelled-preview proofs. Never
restore an old snapshot into a new provider or replay input. If the library cannot
express required state, preserve that evidence rather than narrowing acceptance
in README prose. Images, retention bounds and cross-revision snapshot handling
remain separately documented limitations; this review grants no new waiver.

Proposed usage wording, after implementation: “Reattach while a program uses its
alternate screen; returning to its primary screen keeps the conversation and
history. Program-defined colors survive reattachment; default colors follow the
viewer's theme.” Keep the already-correct resume, preparation and failure-recovery
instructions unchanged. No skill instructions need changing because the action
and concepts stay the same.

Check: `cargo test -p loopflow --lib -- transport::terminal:: transport::viewer_tests:: transport::native:: ghostty_vt::` (inherited `LF_*` unset, stdin closed) — 25 passed after simplification; macOS linker warned about compact-unwind size. `cargo fmt --all`, `git diff --check` and `uv run python -m py_compile scripts/loopflow-dev.py` passed. Prior `ghostty-vt-build --publish` palette/hyperlink tests and four `lf4` archive readbacks remain applicable. Gate (including Clippy), configured-provider demo and four-target package linking remain later obligations.
