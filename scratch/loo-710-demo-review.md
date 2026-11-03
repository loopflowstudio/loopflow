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
Continue through the existing Task's Auto-Ship path. The
repaired candidate passed all 30 focused tests and macOS Clippy. A second real
Claude attach and snapshot demonstration on the repaired binary also matched. Then use the existing Task's Auto-Ship path for the affected gate,
publication, CI, merge and verification. Preserve the LOO-705/709 integration
requirements in the current design and durable PR copy. This note does not claim
that gate, cross-target package linking, merge, or Task completion has happened.

Check: `cargo test -p loopflow --lib -- transport::terminal:: transport::viewer_tests:: transport::native:: transport::snapshot:: ghostty_vt::` with `LF_*` unset and local repaired archive — 30 passed; `cargo fmt --all` and `cargo clippy --all-targets -- -D warnings` passed; `uv run pytest python/tests/test_loopflow_dev.py -q` — 9 passed; `otool -L` confirms no Ghostty dylib dependency.
