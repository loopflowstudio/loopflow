# Navigation implementation proof

2026-09-26 · LOO-303 · Slice 1

Jack's approved scope remains light only, preserving LOO-291. This pass implements
keyboard navigation and Task links. The parent model, bind and attempt UI are not
converted here. The complete design remains in `workspace-ux-on-data-model.md`.

## Observed

- Resource preflight passed before execution (93.2 GiB free) and after builds
  (89.6 GiB free). Verification commands use isolated LF authority, bounded
  process groups (900 seconds), low priority and bounded workers.
- `cargo test -p loopflow --test status_tests
  exact_task_roadmap_retains_history_without_starting_work -- --test-threads=1`:
  one real CLI integration test passes (final command 29.9 seconds). Retained
  W2-127 and planning-only PRD-52 resolve exactly, a missing issue returns no
  candidates, no active PR is required, and Run/event counts remain unchanged.
  Ambient Wave identity does not narrow the explicit lookup.
- `swift test --package-path swift --jobs 4 --no-parallel -Xswiftc -gnone
  --filter 'WorkspaceDestinationTests|SessionChromeProofTests/paletteRetainsTerminalInput|WorkspaceNavigationTests'`:
  30 tests pass (12.6 seconds including build; 6.8 seconds executing). The real
  AppKit event path opens ⌘K over two owned Ghostty cat PTYs. Search reaches
  neither PTY; Escape restores the prior responder; draft echo plus child reply
  survives; Return selects Task details and retains both surface identities.
- After the final modal and search-field changes, the seven destination/native
  checks pass again (27.4 seconds including build; 3.5 seconds executing).
  The native proof also dispatches Down and Return over recent destinations and
  verifies the exact Wave selection.
- Link tests exercise the production parser/model and window router: one decoded
  identifier, rejected malformed paths/query, historical selection and breadcrumb
  retained after an empty current-plan refresh, exact-ID palette ranking, 20
  recents, late-link cancellation by a click, pending cold delivery to one mounted
  window, warm single-window delivery, ambiguity, empty results and read failure.
- `cargo clippy --all-targets -- -D warnings`, `cargo fmt --all --check`, working
  whitespace and `scripts/check_swift_multiplatform_boundaries.py` pass.
- XcodeGen plus signed ad-hoc `xcodebuild build-for-testing -scheme LoopflowMac`
  passes through the documented fallback configuration, including the final
  search-field bytes (39.0 seconds, `xcode-final.log`). This compiles the app and
  runners; it does not run hosted UI tests or activate the installed app.
- Refreshed the generated website documentation copy with `website/dev.py
  sync-docs --source docs`; the generated copy is not tracked. User documentation
  describes the implemented Task lookup and keyboard/link routes.

Logs live under `.lf/tmp/workspace-navigation/`. The first Swift invocation
compiled before the new test file existed and ran zero tests; it is compile
information only and is superseded by the executed filters above.

## Counterexamples and repairs

- Rust compilation caught an incorrect optional filter binding; Clippy then
  caught an unnecessary `expect`. Both were repaired before the recorded pass.
- The first new Swift fixture assumed every Wave had readable Task items. The
  existing fixture intentionally includes unavailable planning. Exact-result
  fixture construction now supplies explicit evidence fields; production keeps
  unavailable reads separate from no match.
- The existing no-Session inspection test failed after direct historical Session
  lookup treated a Task without runtime Work as an unknown reading. The reader
  now distinguishes available empty inventory from failed inventory; all 30
  focused tests subsequently pass. No production default masks read failure.
- Dispatched Down followed by Return initially selected the first recent Task:
  SwiftUI's move command did not receive arrows from the active text editor.
  The search field now handles AppKit editor commands for arrows, Return and
  Escape. The same dispatched regression passes; the failing observation remains
  in `swift-keyboard.log` and the repaired run in `swift-keyboard-fixed.log`.
- Source review found that a current-only plan could hide a linked historical
  page and breadcrumb. The existing selected-Task evidence now retains it. There
  is one selected evidence owner, and historical Tasks never enter the current
  chapter list merely because they were inspected.

## Limits

The native proof uses fixture transport and owned cat PTYs. It establishes local
input and retained terminal behavior, not configured provider behavior, compositor
performance, live screenshots or Jack's acceptance. Router/model proof is not a
Launch Services `open` demonstration of the installed candidate. The OS cold
launch, cross-repository collisions in the real CLI, completed Task variants,
two links racing a click through mounted production windows, and all configured
acceptance remain owed. No fake provider or parser-only result counts as those.

No table-dependent UX, parent maintenance, Task/PR mutation, publication, installed
promotion, provider takeover or Flow navigation occurred.
