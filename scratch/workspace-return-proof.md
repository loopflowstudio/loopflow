# Folded return detail and recursive keyboard disclosure

2026-09-27 · LOO-303 · implement

This focused cut follows the current concept review. Jack's approved light-only
workspace and the complete design remain governing. Existing partial edits were
preserved at `502ba4601`; the supplied concept review was already committed at
`31050c4f6`. No parent storage or invocation authority is introduced.

## Observations and repair

The stabilized mounted test reproduces **Iterate returns to 0** at the root and
**Iterate returns to 3/fix/0** inside an XOR alternative when the target is folded.
The same original graph now supplies node detail and recursive target lookup;
the projected graph still owns layout and visible arrow endpoints. No execution
edge, template field, label cache or YAML reader changes.

Dispatched Tab exposed a second defect: disabled, retained Ghostty views still
accepted first responder and could consume keys while Task details were visible.
AppKit responder eligibility now follows the existing SwiftUI enabled input.
Focus requests and enabled input remain separate: an unfocused visible pane must
still be clickable. Surface lifetime and hidden-host mounting are unchanged.

The shared DisclosureGroup style has one focusable button per group, including
empty groups and XOR alternatives. It is applied at each recursive group because
style inheritance alone left nested groups with the stock keyboard behavior.
It uses the existing DisclosureGroup expansion state: Right/Left expand/collapse;
Space/Return toggle.
No extra expansion store or navigation object was retained. Backward traversal
initially failed after collapse in the unhosted test window; explicitly driving
AppKit's key-view recalculation alongside its layout repaired that fixture.
No application key-loop manager was added.

## Verification

The final focused Swift check passes **9 tests in 5 suites**, 96.4 seconds
including build and 72.0 seconds executing (`return-final-swift.log`). It runs:

```sh
swift test --package-path swift --jobs 4 --no-parallel -Xswiftc -gnone \
  --filter 'TaskFlowTests|TaskFlowProofTests|SessionChromeProofTests/paletteRetainsTerminalInput|TaskMonitorTests/mixedMonitorRetainsInput|GhosttyTerminalInputTests/pasteShortcutFollowsFirstResponder|GhosttyTerminalInputTests/pointerFocusFollowsClickedPane'
```

The mounted production workspace proves both return targets through
folded/expanded/folded states at the root and inside XOR, with distinct target
names so an unrelated detail cannot satisfy the assertion. Both return labels
remain distinct. Captured detail retains the target and saved traversal count.
Dispatched Tab/Shift-Tab, arrows, Space and Return exercise recursive and empty
groups and the XOR path, asserting the focused label and visible disclosure.
No Flow control occurs during inspection. Both PTY buffers remain byte-identical
during those interactions; the same two surface identities retain the draft and
child reply afterward. Palette, Monitor, click-focus and paste proofs cover the
neighboring users of the terminal's enabled-input boundary.

The initial five-test Flow check also passes after key-loop fixture correction
(`return-keyboard-recalculate.log`, 113.7 seconds including build, 62.7 seconds
executing). Commands use `.lf/tmp/workspace-navigation/check.py`:
isolated LF authority, four workers, nice +10, serial Swift tests, and a
900-second process-group limit. Resource preflight passed at 93.5 GiB free.
The later resource preflight also passes at 93.5 GiB free (`return-resource.log`).
After Xcode, final preflight passes at 93.2 GiB free with this checkout's build
outputs at 5.1 GiB of 12 GiB (`return-resource-final.log`).
Swift is Apple 6.2.3. Logs are under `.lf/tmp/workspace-navigation/return-*`.

- `return-detail-reproduced.log` preserves root and nested numeric-target failures.
- Earlier `return-detail-current.log` / `return-detail-fixed.log` stopped at an
  unreliable XOR pointer interaction; they supply no nested passing result.
- `return-keyboard-sequence.log`, `return-keyboard-backtab.log`,
  `return-keyboard-group.log`, `return-keyboard-style.log`,
  `return-keyboard-loop.log`, and `return-keyboard-recursive.log` retain failed
  focus approaches and fixture behavior, not acceptance.
- A one-second sample during the later native run found bitmap capture in
  `settle`; no timeout occurred. Bitmap capture supplies local materialization
  for accessibility inspection, not compositor or installed-app evidence.

Swift platform boundaries and whole active-PR whitespace pass. XcodeGen and signed
ad-hoc Xcode build-for-testing pass (`return-xcodegen.log`, `return-xcode.log`;
235.9 seconds for the build). The app, bundled CLI helpers and test runners compile;
hosted UI tests did not run and the installed app was not replaced.
No Rust, DTO or migration bytes change, and no Rust behavior suite is repeated.
Prior receipts retain only their recorded scope. The test-only enhanced
accessibility call emits an SDK
deprecation warning; the native proof executes successfully. Ghostty uses the
existing timer-renderer fallback.

## Review and remaining boundaries

The simulated source review retains separate original/projection graph roles and
uses the existing recursive node lookup. It removes the attempted duplicate
keyboard state instead of keeping an adapter. No Flow operation, Run or Session
writer, provider launch, or terminal pool is added. The enabled-input flag carries
an existing view input across the SwiftUI/AppKit boundary; it grants no process
authority and does not release a PTY.

Full-design acceptance remains open: the explicit LOO-298 contract checklist,
exact attempts and independent-Run counterexample, room/shared-shell one-mount
proof, permanent bind confirmation and races, installed cold/warm Task links,
configured providers/drafts, live captures at both requested widths, Jack's
verdict and final deletion. The inherited `wave_chapters` map gap remains from
the prior review; the architecture inventory is not rerun for this view-only cut.
Current source still has no Bind in `SessionCommand` or node/iteration/current-attempt projection on
`Run`, so no parent readiness is inferred.
No installation, live Home migration, publication, Task disposition or Flow
navigation decision is part of this cut.
