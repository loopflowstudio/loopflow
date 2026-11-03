# LOO-710: Session snapshots at Rex fidelity with libghostty-vt

Status: **accepted** (Jack Heart's Task brief, 2026-11-02; mechanisms below are
this plan's choices). Reconciled 2026-11-02 against `2512cd5a3e` and gate repairs.
Preview recovery, both buffers, program colors, origin-relative cursors and
keyboard stacks are implemented. The configured-provider demo passed with the
limits recorded below; remaining delivery checks are listed under Gate results.
No product decision is pending.

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
  to the viewer's configured theme. HTML formatting is unchanged. Archive `lf6`
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

- **CI and delivery.** Local gate passed after the two repairs recorded below:
  transport and Session suites, interactive Graph input and the disposable Linux
  installation proof. Linux Clippy and four-target release-package linking remain
  CI's obligations. The configured-Claude demo passed within its recorded limits;
  source fixtures do not establish Desktop rendering or installed acceptance.
- **Meeting LOO-709.** The November 2 status read reports unfinished work and no
  PR; the prior code inspection found its branch writes `screens/<session>.vt` through
  `transport::save_last_screen`/`last_screen_path`, `Output::last_screen` and
  `terminal::last_screen(&vt100::Screen)`, and a `transport_tests` case decodes that
  file with `vt100::Parser`. Both branches call their writer at the same two owner
  ends. Whichever lands second deletes the `.vt` writer and its helpers, keeps
  `snapshot::leave` at both ends, and rewrites that test to decode `.ghostsnp`
  through `Terminal::decode`. No DTO field either way.
- **Meeting LOO-705 (PR #1799).** The November 2 status read still reports an open
  PR at demo. The `ending` arm's body changes shape (it queues
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
- The configured-provider demo attaches a second viewer to a running Claude
  conversation and compares colors, hyperlinks, alternate screen, keyboard modes
  and retained history; ending and reopening the same Session paints its saved
  display before live handoff. Fixture passes do not replace this acceptance.

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


## Fidelity reconciliation — 2026-11-02

**Accepted scope, implemented.** Jack Heart's same-screen requirement includes
retained primary content after alternate-screen attachment and explicit program
colors. The prior review found that active-screen-only formatting skipped primary
history and that disabling palette export dropped OSC 4 changes. These were
implementation gaps, not product choices. The complete review and counterexamples
remain at `9371e126f4:scratch/loo-710-session-snapshots.md`, “Concept review after
recovery”; its next-implement direction is now fulfilled.

`terminal::repaint` formats retained primary history and screen, then the active
alternate screen. Only a disposable decoded copy receives cancellation of
unfinished parser input and the switch to primary. The owner's bytes, continuation
and replies remain intact. Removing the intervening cursor-home preserves the
primary cursor saved by 1049. Live attachment and saved preview share this path.

`Terminal::format` enables palette restoration through archive `lf4`'s selective
VT patch. The override mask preserves even explicitly assigned values equal to
the owner's defaults; untouched colors remain at the viewer's configured theme.
Foreground, background and cursor overrides are emitted too. Cleanup and live
handoff reset those overrides. HTML formatting remains unchanged, providing the
rendered-color oracle for Rust fixtures.

Existing proofs now cover:

- Alternate-screen attachment followed only by `?1049l`: primary text, cursor,
  keyboard modes and retained history agree, and prior viewer history survives.
  The same fixture runs after snapshot encode/decode and checks that repaint
  leaves the live owner's encoding and split-sequence continuation unchanged.
- Indexed overrides, including one equal to the owner's default, resolve to the
  same rendered foreground. Untouched indexed/default colors retain a different
  viewer theme. Forwarded output and the return to primary keep program colors;
  cleanup restores the theme. Zig round trips also cover foreground, background
  and cursor overrides.
- Existing fresh-owner, mode-only, failed-launch and cancelled-preview proofs
  remain; PTY handoff/failure fixtures now also inspect color reset. Public resume
  fixtures retain Session history and exercise provider failure without replaying
  input. They use fixture providers, not configured Claude or Desktop.

No new public command, DTO, snapshot lifecycle or GhosttyKit decoder was needed.
Images, bounded history and cross-revision snapshot rejection remain documented
limits. Source inspection found no further bounded mismatch in this reconciliation;
the remaining acceptance and second-landing integration work is listed above.

## Concept review after fidelity repair — 2026-11-02

**Outcome: nothing to change; no open product choice.** Reviewed the current
implementation and usage docs against Jack Heart's accepted experience. The
Session remains the conversation, its owner retains live terminal state, and a
viewer temporarily displays and controls it. The saved screen is optional display
data under the same Session identity; it creates no separate recovery lifecycle.

The normal interaction stays `lf resume SESSION`: `Output::attach` repaints a
live owner; an ended owner goes through `delegate_with_preview` and
`snapshot::last_screen` before live acknowledgement. The consequential earlier
mismatch—retained state missing after a bare return to primary—is addressed by
the shared repaint path described above. Program overrides and viewer theme now
have separate, explicit ownership without adding a user-facing choice.

Failure and recovery preserve this model: `ViewerTerminal` owns cleanup from
preview through attachment, blank live output supersedes saved display, and a
failed start permits reopening the same Session without replaying input. An
unreadable or incompatible snapshot does not become a prerequisite for resume.
The existing README expresses these actions; no skill instruction needs changing.

No next-implement direction is added. Gate, package linking and the configured
provider demo remain the next proof, with the LOO-705/709 integration obligations
above preserved. This review neither waives those checks nor establishes native
acceptance from fixtures. No code or Wave memory was changed in this step.

Check: `cargo test -p loopflow --lib -- transport::terminal:: transport::viewer_tests:: transport::native:: ghostty_vt::` (inherited `LF_*` unset, stdin closed) — 25 passed after simplification at `9371e126f4` (reused for this prose-only reconciliation); macOS linker warned about compact-unwind size. Prior `cargo fmt --all` and `uv run python -m py_compile scripts/loopflow-dev.py` passed; this reconciliation passed `git diff --check` and `lf context` (both memory/scratch budgets met). Prior `ghostty-vt-build --publish` palette/hyperlink tests and four `lf4` archive readbacks remain applicable. Gate (including Clippy), configured-provider demo and four-target package linking remain later obligations.

Publication check (2026-11-02): `lf sync` merged main without conflicts; `cargo test -p loopflow --test transport_tests` (inherited `LF_*` unset, stdin closed) — 20 passed; `cargo fmt --all -- --check` and `cargo clippy --all-targets -- -D warnings` — passed on macOS. The earlier integration-binary transport filter selected zero tests and is not counted. `git diff --check origin/main...HEAD -- ':!*.patch'` passed; patch files retain required blank context-line prefixes. The full gate, Linux Clippy, four-target package linking and configured-provider demo remain with their existing Graph boundaries.

## Manual demo and simulated review — 2026-11-02

Jack Heart requested manual verification, a Mitchell Hashimoto-inspired review,
repairs, and Auto-Ship. See `scratch/loo-710-demo-review.md` and `scratch/demo.html`.
Real Claude attach, same-Session reopen, and exit snapshot decoding passed.
The review repaired origin-relative cursor/content restoration, the full Kitty
keyboard stack and cleanup, and static linking from a local Zig output directory.
Archive lf6 includes the third formatter patch; lf5 is superseded. The new
30-test focused pass, nine Python tests, formatting and macOS Clippy are recorded
in the review note. Remaining gate and package CI retain their authored steps;
the completed-Session lifetime limitation and LOO-705/709 integration remain explicit.

## Gate plan — 2026-11-02

Comparison: `origin/main...2512cd5a3e`, plus the existing uncommitted demo
records. Preserve those records and the prior implementation. Changed terminal
FFI, transport, preview and resume paths reach `transport::`, `ghostty_vt::`,
`ops::human_session::`, `transport_tests`, `agent_startup_tests`,
`session_cli_tests`, the CLI binary and the interactive Graph input proof.
`build.rs`, Cargo dependencies and archive tooling reach architecture checks,
Python tooling tests, formatting and Clippy. The changed-aware runner selects
architecture, Python and filtered Rust; no Swift UI or website paths changed.
The installation boundary additionally requires the disposable
`declared_agent_can_start_another_tasks_graph` proof. Linux Clippy and all four
release-target links remain CI obligations; native macOS linking is exercised
by the gate's CLI build using the published pin. Existing real-provider evidence
is retained without replaying a provider turn.

## Gate results — 2026-11-02

The review found two concrete delivery defects: dependency removal left the
notices inventory stale, and the macOS cleanup fixture tried to fetch the new
native archive from inside network isolation. Refreshed notices remove `vt100`,
`vte` and their unused dependencies. The gate now prepares the CLI before isolated
macOS cleanup tests; a focused plan regression verifies that order and leaves
unrelated Python-only checks without a Rust build. The script README and shared
testing commands document the boundary. No terminal-code repair was needed.

- `uv run python scripts/test.py --reuse-passing --rust-filter 'test(transport::) | test(ghostty_vt::) | test(ops::human_session::) | binary(transport_tests) | test(agent_startup_tests::) | test(session_cli_tests::) | test(interactive_graph_keeps_input_and_advances_only_after_each_provider_exits) | binary(lf)'` — architecture, inventory, formatting, published-archive CLI build, 131 Rust tests and macOS Clippy passed; initial Python run had 510 passes, one intentional nested-sandbox skip and the two failures above; log `/tmp/loo710-gate.log`.
- `uv run python scripts/third_party.py refresh` — regenerated notices for 270 components; generated text/resources need no Swift compilation, and the notice validation below passed.
- `uv run --no-sync python scripts/test_network.py uv run --no-sync pytest python/tests/test_disk_cleanup.py python/tests/test_third_party.py python/tests/test_gate_bounded.py -q` — 72 passed, including both repaired failures and the new build-order regression; log `/tmp/loo710-gate-repairs.log`.
- `uv run --no-sync pytest python/tests/test_desktop_performance.py::test_checkout_observation_preserves_read_boundary_and_detects_changes -q` — 1 passed, completing the separately sandboxed check the first Python failure prevented.
- `uv run python scripts/test_task_installation.py --test declared_agent_can_start_another_tasks_graph` — passed in disposable Linux with Rust 1.99.1, including both migration preconditions; published native archive linked, all three proofs passed and the container was removed; log `/tmp/loo710-installation.log`.
- `git diff --check -- ':!*.patch'` — passed; patch files retain required blank context-line prefixes.

All commands ran with inherited `LF_*` unset and stdin closed. No check timed out.
Rust inputs were unchanged by the Python runner/notice repairs, so the passing
Rust phase was retained without repeating it. The remaining Python tests keep
their earlier pass; this is a repaired composite gate, not a claim that the first
runner invocation exited successfully. The existing configured-provider demo was
not replayed. CI still owns Linux Clippy and the four release-package links;
this local Linux debug/install proof does not establish the release matrix.
The LOO-705/709 second-landing requirements above remain explicit. No background
process started by this gate remains.
