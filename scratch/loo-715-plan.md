# LOO-715 — Keep Desktop alive through browser close

## Outcome and boundary

Jack Heart said “lets do it” on November 14 after the incident review
(`loo-715-direction.md`). Desktop must survive closing pages during first paint,
including replacing the one close held for Undo. Back/Forward history, pane moves,
hide/show and Undo keep the same live page. Discarded pages eventually deallocate.
Future abnormal exits must name available crash evidence rather than imply that
an absent signal record excludes a crash.

This serves Infrastructure's recovery and actionable-failure objective and the
chapter's uninterrupted Session work. No numeric performance KR was supplied.
No install, merge, OS upgrade, Apple report or evidence upload is authorized;
review precedes shipping. Production reporter policy remains Jack's decision.

## Evidence and current architecture

- November 2's investigation found three preserved EXC_BAD_ACCESS dumps in
  `RemoteLayerTreeDrawingAreaProxyMac::displayLink`, reached by a layer-tree
  commit after page destruction. A fourth exit matches only by signature.
  Breakpad's Mach exception handler wrote dumps then called `_exit(1)`, bypassing
  BSD signal/atexit hooks and macOS reports. The release trigger in Desktop is
  still unobserved. Preserve that distinction; the incident is stronger than a
  guess, weaker than a reproduced application trigger.
- `BrowserPageView.swift:BrowserPages` owns `[tab ID: WKWebView]`; `view(for:url:)`
  starts navigation immediately. `release` only removes the dictionary entry.
  `BrowserPageSurface` reparents the same view across containers. Containers and
  SwiftUI values may retain a view after dictionary removal: log actual final
  destruction as well as retirement intent.
- `SessionsWorkspace.closePane` in `SessionsView.swift` releases the *previous*
  `ClosedOccurrence`, returned by `MultiplexerStore._record`, not the tab just
  closed. Closing a shell or hiding a pane can displace a browser Undo entry.
  `undoClose` restores tab identity. `SessionsWorkspace.pages` also dies when its
  owning workspace/registry dies; view disappearance alone is not tab disposal.
- `DesktopActions.webTab/page` resolves the addressed tab and uses this same pool
  for Back/Forward/Reload. `WorkspacePanesView` renders it. No DTO, persisted
  layout or schema change is needed. `MultiplexerStore.lastClosed` remains the
  authority for Undo; the pool owns native lifetime, not a second tab model.
  A hidden pane is not a discarded tab, even after its Undo entry is displaced;
  workspace loss disposes all its pages.
- Compared checkout `4e9ade2d04` with local `origin/main` `53a62ddfa7` on November 14:
  `BrowserPageView.swift` and `desktop_exits.rs` are unchanged. `SessionsView.swift`
  has substantial intervening changes, including retained conversations and
  shared-server work. Integrate current workspace ownership before editing it;
  do not transplant the old file. Launch identity and database-skew repairs are
  already separate delivered work, not prerequisites to reinvent here.

## API findings and alternatives

The [public WKWebView header](https://github.com/WebKit/WebKit/blob/main/Source/WebKit/UIProcess/API/Cocoa/WKWebView.h)
offers `stopLoading`, which cancels resource loading, but no public close/drain
acknowledgement for pending layer commits. Apple's
[didFinish contract](https://developer.apple.com/documentation/webkit/wknavigationdelegate/webview(_:didfinish:))
reports navigation completion. Treating it as a rendering teardown barrier is
unsupported. Loading `about:blank` introduces another navigation and may fail or
never finish; it is an experimental alternative, not the chosen completion rule.

[WebKit commit 2b7262d](https://github.com/WebKit/WebKit/commit/2b7262df1ad38a5e28bfd3abf78f103a8e25fded)
repairs display-link client deregistration that depended on a still-valid page
and display ID. It establishes related internal lifetime risk, not identity with
this commitLayerTree crash or availability in a released macOS build. It cannot
be applied through public WKWebView APIs. No upgrade is part of this Task.

Choose one explicit retirement path in the existing pool, covering explicit
discard and owner loss. First candidate: detach and stop a discarded view on the
main actor, then bounded deferred final release so queued work can drain. Compare
against immediate release, detach/stop alone, and blank-navigation retirement in
the reproduction. Do not select a delay or ship this candidate until paired
trials reproduce the original failure and distinguish it. Failure to reproduce
leaves the mechanism unresolved; a headless pass cannot choose it by default.
Never retain all closed pages, wait indefinitely for a callback, use private SPI,
or destroy a page still eligible for Undo. Any bound needs measured justification
and tests for failed navigation/WebContent death; an expiry is not itself proof
of safety. Main-actor cleanup must also work with CI's older Swift toolchain,
without `isolated deinit`.

## Delete — do not maintain

- The harness's hardcoded `didFinish` scheduling is replaced by one selectable
  anchor; all teardown variants use the same scheduler and content.
- No production retirement predecessor is deleted before a baseline reproduces.
  If selected, remove dictionary-drop disposal in `BrowserPages.release` and
  cover pool loss through the same retirement owner; preserve Undo/history.
- Reporter experiments remain isolated. A selected production route replaces the
  relevant unknown-exit diagnostic, rather than adding a second reporter owner.

## What exists (November 14)

- Diagnostics: `BrowserPages` allocates a `BrowserPage` (`WKWebView` subtype)
  carrying `BrowserPageLifetime`; `DesktopLog` records create/load/attach/detach/
  retire/final-release/load-failed/web-content-terminated and workspace loss, with
  pool/tab/page identity, scheme only and monotonic age. Disposal policy is
  unchanged: `release` still drops the dictionary entry immediately (the baseline).
  Native destruction can lag retirement while a container owns the view.
- `BrowserPageLifecycleTests` cover addressed background-tab Close, two closes
  plus Undo identity, shell/pane displacement (through the production pane
  action), reparenting, workspace loss, reclamation, redaction and an unwritable log.
- `ExitSignals.swift` and `desktop_exits.rs` no longer claim a fault reaches
  Ghostty's reporter via BSD signals; doctor's no-record text names a native
  crash reporter and says the cause is unknown. `swift/README.md` describes
  Close/Undo, history, pane moves, reclamation and exit-evidence limits; it makes
  no crash-survival claim until a repair is demonstrated.
- Harness `scripts/benchmarks/browser-lifetime/` (Trial.swift + run.py) runs six
  ownership scenarios × four teardown variants in reaped child processes; the
  second two-close page is preloaded so creation cost doesn't shift the offset.

Earlier run evidence: final comparison (`run.py --compare --offset-ms 0 --offset-ms 200
--attempts 1`) completed 48 visible-window trials: zero failures/crashes, 64
native releases, zero retained pages, 49 first-contentful-paint observations
for 64 pages. Those trials did not record window occlusion, so window creation and
paint entries alone establish no display-link activity. Receipts and compiled source (SHA-256
`9538588d1d6490abb3f66ea733577c130bfd9cff416d9ab0d6aa7d87ae6c33a5`) are in the
private incident directory under `browser-lifetime/20261114T165018.890143Z/`;
the earlier 48-trial prototype under `browser-lifetime/20261114T164635.532353Z/`.
Loaded WebKit: `/System/Library/Frameworks/WebKit.framework/Versions/A/WebKit`,
UUID `1501F244-BE5E-3AD9-ACD8-E9A8880CE2E5`, bundle `21622.1.22.11.15`, macOS
26.0.1 (25A362). The incident's "WebKit 623.1.14" label is a different naming,
not evidence of a different binary. Immediate release survived every trial, so
no retirement mechanism is selected.

## Second implementation pass — November 14

The reproduction now has selectable load-request, navigation-commit,
navigation-finish and animation-frame anchors; continuous CSS/canvas animation;
and SwiftUI close/reparent, second-window transfer and occlusion scenarios.
SwiftUI owns container detachment in its scenarios. Reparent now waits for the
new representable to attach; same-turn state changes could otherwise coalesce.
That final path compiled but remains unexercised on an unoccluded display.
Others detach explicitly.
No production disposal change is selected.

36 immediate-release trials at navigation commit (four new scenarios × 0/25/200
ms × three attempts) completed with 36 native releases, zero retained pages and
zero crashes. Only 12 FCP observations and **no animation-frame callbacks** were
recorded. Receipts: private incident directory,
`browser-lifetime/20261114T170455.001031Z/`. These trials lacked an occlusion
check and are not evidence of active display-link coverage. A later 800×600
single-window trial reported `window_visible=true`, `unoccluded=false` and no
animation callback (`20261114T171103.064257Z/`). One screen was available.
The initial animation sweep was stopped after seven completed anchor failures;
its interrupted child was killed and reaped. The first attempted sizing change
also had no callback (`20261114T171020.938852Z/`), so zero-size layout was not the
established cause.

The harness now requires an unoccluded window before loading and a visible,
nonzero page before release. Missing display readiness is `unavailable` and
stops the sweep; it never passes on console-session presence alone. Final
reparent preflight `20261114T172417.940997Z/` returned unavailable before
navigation. `20261114T172230.420442Z/` also verified the comparison driver
stops after one unavailable child instead of continuing the sweep.
This corrects the measurement, not the browser lifetime. All new experiments
used macOS 26.0.1 (25A362), the same WebKit image UUID/version recorded above.

Both authorized reporter experiments are measured:

- Built the pinned Ghostty source with all current embedder patches using Zig
  0.16.0 and `-Doptimize=ReleaseFast -Demit-macos-app=false -Di18n=false
  -Dsentry=false -Dxcframework-target=native -Demit-docs=false`. Native archive
  SHA-256 `602b9e28df796d41d2cc76f617ed511c5f3c95e54f88b83bbd71545b02593806`.
  No Sentry/Breakpad exported symbols. Source, patch hashes, flags and notices
  are retained privately in `reporter-disabled/build-provenance.json` and
  `reporter-disabled/source/`; no artifact or production pin changed.
- The disposable reporter probe initialized Ghostty/config/app/Metal surface
  with external I/O and cleanly exited. A second child, using production
  `ExitSignals.swift`, intentionally wrote to address 1: return `-11`, exactly
  one SIGSEGV journal line, PID 52319, 17:13:04 UTC. Matching macOS report:
  `LoopflowReporterDiagnostic-2026-11-14-091306.ips`. Both children denied
  network access; no shell/provider/Desktop/store was launched. Receipt and
  report remain private under `reporter-disabled/20261114T171301.962685Z/`.
  The initial matcher missed the report because macOS redacted `procPath`;
  posthoc PID + Mach-O UUID + signal-second matching verified the same fault,
  without repeating it. The original receipt is retained as `initial-result.json`.
- `desktop_exits::envelope_experiment::` is test-only until policy review.
  Synthetic fatal envelopes correlate exact executable, PID, kernel creation
  second and crash interval; unrelated Ghostty/Dev, PID reuse, ambiguity,
  torn/missing/rotated evidence stay unknown. Metadata reads seek over memory
  streams; a reader that forbids those reads verifies it. Output identifies
  EXC_BAD_ACCESS and the embedded dump's envelope/attachment without extracting
  memory or claiming a nonexistent separate dump file. Existing launch records
  lack executable/process-birth identity; production integration would add them.
  The parser targets native explicit-length envelopes; unsupported forms stay
  unknown. No real incident dump enters tests.

**Recommendation for review, not a policy decision:** disable Ghostty's reporter
in the production artifact after its normal packaging/build checks. This probe
restored the current signal journal and standard macOS report with no runtime
report parser. Keeping the reporter is viable but requires exact launch identity,
metadata parsing and handling rotating envelope files, while retaining native
dump memory on disk. Neither alternative repairs WebKit. Headless review leaves
the product choice open; no install, merge, upload or reporter change occurred.

Focused checks: `scripts/test_desktop.sh --filter BrowserPageLifecycleTests`
passed (5, previous pass; production Swift unchanged); `uv run pytest
python/tests/test_browser_lifetime.py -q` passed (9); `cargo test -p loopflow
--lib desktop_exits::envelope_experiment::` passed (4, including interval/lint cleanup); original no-record diagnostic test passed (1, unchanged).
Ruff and Swift multiplatform checks passed. Final fmt, `cargo clippy --all-targets -- -D warnings` and diff checks passed.
`cargo test -p loopflow --test integration documented_commands::` passed (4).
The test link emitted the existing large `__eh_frame` compact-unwind warning;
no test failed.

## Concept review — November 14

The interaction stays Close → Undo, with the same live page and history. A second
close replaces the one Undo entry; hiding or moving a pane keeps its contents.
`MultiplexerStore.lastClosed` owns recoverability, `BrowserPages` owns native
pages, and SwiftUI containers own presentation. Retirement intent and native
destruction are observations of that ownership, not another persisted lifecycle.
Keep that division through any repair: no new user action, DTO, schema or second
Undo/cache owner is needed.

Normal path: close A, close B, Undo B; A becomes disposable and B returns with
its history. Failure: pending WebKit work can fault after native destruction;
neither pool removal, navigation completion nor two animation frames acknowledges
a drained layer tree. Recovery: relaunch restores access, without promising live
page history after process death. Owner loss, failed navigation and WebContent
death must still reclaim discarded pages. A retained hidden pane remains live.

The measurements reject selecting a retirement policy now: immediate release
survived, while the wider harness exposed missing unoccluded rendering. The next
implementation direction remains conditional on a failing baseline on a capable
display; do not turn missing display input into another delay or survival sweep.
Both reporter experiments are complete, so repeating them is unnecessary.

**Open product choice for loop-or-next (Blocked):** should production disable
Ghostty's reporter and use the signal journal/macOS reports, or retain it and add
exact launch-to-envelope correlation in `lf doctor`? Disabling is the measured
recommendation, not a selected policy. It changes which crash evidence is kept:
the alternative retains native dump memory and needs launch-identity fields plus
rotation-aware metadata reads. Neither choice fixes WebKit. The missing
unoccluded reproduction session is a separate experimental dependency, not a
product decision. Show the failing-baseline comparison and the chosen diagnostic
route before review for shipping.

## Remaining, in order

1. Obtain an unoccluded display session with animation callbacks and reproduce
   the baseline. Current host supplies an occluded console session. Run the new
   SwiftUI/animation/window scenarios there; if they still survive, use an
   isolated Dev Desktop to observe the actual release trigger. No live Desktop
   crash and no screen unlock is part of this work. This is the missing input
   for further dependent lifetime implementation.
2. With a failing baseline, compare all four retirement variants on identical
   content/timing; implement the distinguished strategy in `BrowserPages`,
   covering owner loss, failed navigation and WebContent death with bounded
   reclamation and no callback/timer cycle or `isolated deinit`.
3. Review the measured reporter recommendation. If disabling is selected, rebuild
   and package Ghostty with correct notices/provenance, then update its pin. If
   correlation is selected, move the tested candidate into the real doctor read,
   capture launch executable/process birth, and delete the test-only placement.
4. Gate on the final production tree: `uv run python scripts/test.py --swift
   --loopflow`; Rust diagnostic tests, fmt/clippy and fallback build. Isolated
   Desktop walkthrough of Back/Forward, pane moves, two closes and Undo for demo.

Done when Desktop survives the demonstrated first-paint release scenario, retired
pages are reclaimed, history/Undo/pane moves still work, and the chosen diagnostic
route explains a disposable native fault. A delayed crash or a growing retirement
pool hidden by green model tests is the failure to watch for.

## Not in scope

No install, merge, OS upgrade, Apple report or evidence upload. Dumps, envelopes
and derived evidence stay outside Git. Unrelated database, Sparkle and provider
failover findings in the incident reports select no work here.
