# LOO-710 manual demo and simulated review — 2026-11-02

Jack Heart requested a manually operated demo, a Mitchell Hashimoto-inspired PR
review, repairs, and Auto-Ship. This is Codex's review using explicit state,
small interfaces, terminal correctness, and reproducible builds as its criteria;
Mitchell Hashimoto did not participate or endorse it.

## Real provider demo

The candidate `target/debug/lf` ran Claude Code 2.1.316 in a disposable Loopflow
Home and Git repository. No installed binary or main Loopflow database was changed.
Claude returned `LOO710-DEMO-READY` using the configured native account. A controlling
viewer consumed actual owner output; a second passive viewer received the owner's
repaint over the Unix socket. Both were rendered through the pinned Ghostty library.
Their visible text, HTML (including colors), cursor (2,20), and alternate-screen
state matched exactly. This is native-provider/transport evidence, not a Desktop
window screenshot. Claude's screen did not exercise OSC 8 links or nested keyboard
stacks; focused fixtures cover those.

`/exit` ended the real provider and left the revision-tagged, Session-keyed snapshot.
A real PTY running `lf resume session_1e6055eec274459cb30fd85fc4b8fba9` reopened the
same native conversation and displayed its prior response. No prompt was resent.
A separate executable-removal probe did not end this owner: the resumed Session remained completed in its
isolated store, while the lifetime policy requires an unfinished Session. There
was also no `session_activity` row. This
proves neither install-supersession nor idle-timeout behavior; those remain the
sibling lifetime work's acceptance. The demo provider was then closed with `/exit`. Decoding its replacement
snapshot produced exactly the last viewer text and cursor, confirming the real
owner-end writer and Ghostty decoder agree.

Evidence: `scratch/demo-provider-evidence.json`, `scratch/demo-exit-evidence.json`,
`scratch/demo-resume-evidence.json`, `scratch/demo-snapshot-evidence.json` and the
small `scratch/loo710_*demo.py` probes. The disposable Home is
`/private/var/folders/m6/r3tllnrs1yq7yfbwm680tss40000gn/T/loo710-demo-5hb8my7m`.

## Review findings

1. **P1 — restore the keyboard stack, not just current flags.** A program pushes
   two Kitty modes, reattaches, then pops one. The owner returned flags 5 while the
   viewer returned 0. Repaint had serialized only the current flags. The formatter
   now reconstructs the meaningful stack, including ring overflow; cleanup clears
   the whole stack rather than popping only one level.
2. **P2 — cursor coordinates must respect origin mode.** With top margin 5 and
   an origin-relative cursor, reattachment moved the cursor from row 6 to row 10.
   The formatter used an absolute row in an origin-relative CUP. The patch passes
   the margin origin to final cursor formatting, including pending-wrap placement. Drawing uses screen coordinates with full
   margins before restoring origin mode and the program's margins; this also fixes
   content shifted during alternate-screen restoration.
3. **P2 — local library builds must really link statically.** Zig leaves both
   `.a` and `.dylib` files in its output directory. The macOS linker selected the
   dylib for the documented override, and the test binary then failed at launch.
   `build.rs` stages just the archive and watches the source archive for rebuilds.
   The repaired binary runs without a Ghostty dylib dependency (`otool -L`).
4. **Checked, no finding — wide-character pending wrap.** A wide character in the
   last two columns followed by more text restored correctly on the existing code.
   The regression test retains that boundary while cursor formatting changes.

The repairs stay in the pinned formatter patch; Rust does not grow a second
terminal-state implementation. Archive generation advances from lf4 to lf6 and
checks the new Zig round trip/pin map alongside existing formatter cases before
publication. Rust fixtures exercise both live and decoded snapshots, primary and
alternate cursor state, nested/overflowed stacks, subsequent output, and cleanup.

## Remaining

All four final lf6 archives were published and checksum-verified on readback.
The repaired candidate passed all 30 focused tests and macOS Clippy. A second real
Claude attach and snapshot demonstration on the repaired binary also matched.
The headless decision below advances this demo; the existing Task's Auto-Ship
path retains the affected gate, publication, CI, merge and verification. Preserve the LOO-705/709 integration
requirements in the current design and durable PR copy. This note does not claim
that gate, cross-target package linking, merge, or Task completion has happened.

Check: `cargo test -p loopflow --lib -- transport::terminal:: transport::viewer_tests:: transport::native:: transport::snapshot:: ghostty_vt::` with `LF_*` unset and local repaired archive — 30 passed; `cargo fmt --all` and `cargo clippy --all-targets -- -D warnings` passed; `uv run pytest python/tests/test_loopflow_dev.py -q` — 9 passed; `otool -L` confirms no Ghostty dylib dependency.


## Headless demo decision, pass 1 — 2026-11-02

**Advance.** Jack Heart delegated this decision to the headless run. Codex's
assessment is that the work before this step holds: the previous review intended
to repair keyboard-stack restoration, origin-relative cursor/content placement,
and static linking; commit `2512cd5a3e` contains those repairs. The recorded
30-test pass, lint, archive readbacks and second configured-Claude demonstration
supply progress beyond the earlier findings. No concrete remaining implementation
defect was established by this pass. This is a delegated assessment, not a claim
that Jack personally observed or accepted the display. No new human feedback was
received, and no design change is proposed or attributed to Jack.

The existing configured-provider demonstration was reused; no provider turn was
replayed. A read-only probe decoded its on-disk snapshot again, checked the header
against the current Ghostty revision, and reproduced the recorded final screen
and cursor. It also checked equality of the recorded live viewers' text and state.
The earlier `same_html` observation remains recorded evidence, not a new HTML
comparison in this pass. See [decision evidence](demo-decision-evidence.json),
[live attachment evidence](demo-provider-evidence.json),
[exit snapshot evidence](demo-snapshot-evidence.json), and
[same-Session resume evidence](demo-resume-evidence.json).

The latest attachment/snapshot run used `loo710-demo-i6hveez3`, Session
`session_ed81039482c94e209d46a178f0daa7ca`; the earlier resume run used
`loo710-demo-5hb8my7m`, Session `session_1e6055eec274459cb30fd85fc4b8fba9`.
These are two demonstrations, not one continuous trace. The final snapshot shows
the exit hint after Claude left; live conversation fidelity is established by the
separate live-attachment capture. Its history count of zero versus the viewer's
one row does not establish scrollback equality; retained-history behavior remains
covered by the focused fixtures. The native capture did not exercise OSC 8 links
or nested keyboard stacks. No Desktop screenshot or new installed-build proof is
claimed.

The unsuccessful executable-removal probe remains evidence of an unmet fixture
precondition (completed Session, no activity row), not lifetime-policy acceptance.
LOO-705/709 integration and four-target linking retain the owners described in the
[current design](loo-710-session-snapshots.md#remaining). Neither absence of a
person nor provider exit supplies approval. No unresolved product question requires
a backward edge to implementation.

Next action: advance the existing Graph to its delivery checks, retaining the
outstanding gate and package CI obligations. The [review page](demo.html) is a
view of this record; this headless pass requested no interactive action and
started no background process.

Check: read-only `uv run python` snapshot/evidence probe — passed at `2512cd5a3e`;
existing focused test and lint results reused because production files are unchanged.
