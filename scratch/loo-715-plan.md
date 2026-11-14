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

Run evidence: final comparison (`run.py --compare --offset-ms 0 --offset-ms 200
--attempts 1`) completed 48 visible-window trials: zero failures/crashes, 64
native releases, zero retained pages, 49 first-contentful-paint observations
for 64 pages. Receipts and compiled source (SHA-256
`9538588d1d6490abb3f66ea733577c130bfd9cff416d9ab0d6aa7d87ae6c33a5`) are in the
private incident directory under `browser-lifetime/20261114T165018.890143Z/`;
the earlier 48-trial prototype under `browser-lifetime/20261114T164635.532353Z/`.
Loaded WebKit: `/System/Library/Frameworks/WebKit.framework/Versions/A/WebKit`,
UUID `1501F244-BE5E-3AD9-ACD8-E9A8880CE2E5`, bundle `21622.1.22.11.15`, macOS
26.0.1 (25A362). The incident's "WebKit 623.1.14" label is a different naming,
not evidence of a different binary. Immediate release survived every trial, so
no retirement mechanism is selected.

Focused checks passed: `scripts/test_desktop.sh --filter BrowserPageLifecycleTests`
(5), `uv run pytest python/tests/test_browser_lifetime.py -q` (7),
`cargo test -p loopflow --lib desktop_exits::tests::an_unrecorded_end_asks_the_system_log_within_the_heartbeat_window`
(1), `cargo fmt --all -- --check`, Ruff, Swift multiplatform check, `git diff --check`.

## Remaining, in order

1. Reproduce the baseline failure. The standalone harness doesn't; next widen
   toward Desktop's real trigger: SwiftUI container retention during
   `WorkspacePanesView` re-render, close during reparent, display-link activity
   (multiple windows/screens, occlusion), and more attempts per offset. If needed,
   drive an isolated Dev Desktop (own `LF_HOME`, never the live app) and read the
   lifecycle log around a forced first-paint close.
2. With a failing baseline, compare immediate release, detach+stop, bounded
   deferred release and blank navigation on identical content/timing; implement
   the winner in `BrowserPages` (owner loss, failed navigation, WebContent death,
   no timer/delegate cycle, no `isolated deinit`), replacing dictionary-drop.
3. Reporter experiments, both, before any recommendation:
   - Diagnostic Ghostty build with `-Dsentry=false` (`cmd_ghostty_build` omits
     the flag; `swift/Package.swift` pins a downloaded binary, so production is
     unaffected). Disposable child initialized like Desktop, intentional fault;
     require signal-journal line plus matching `.ips`. Check terminal init,
     artifact provenance and notices.
   - Fatal-envelope correlation in `desktop_exits.rs` with synthetic envelopes
     only: executable identity, crash time within the launch interval; reject
     unrelated Ghostty/Dev crashes, PID reuse, ambiguity; report exception and
     dump path without reading dump memory; missing/rotated stays unknown.
   Then bring the measured tradeoff and a recommendation to the primary
   conversation. Operator steer (November 14): "the production crash-reporter
   choice remains Jack Heart’s, but it does not block the already-authorized
   isolated experiments … Do not request the choice before its supporting work
   is done. Keep the missing baseline reproduction as a separate engineering gap;
   diagnostics alone do not complete the Task."
4. Gate on the final tree: `uv run python scripts/test.py --swift --loopflow`;
   Rust changes add `cargo test -p loopflow --lib desktop_exits::` (LF_* unset,
   stdin `/dev/null`), `cargo fmt`, `cargo clippy --all-targets -- -D warnings`;
   fallback build. Isolated Desktop walkthrough of Back/Forward, pane moves, two
   closes and Undo for demo.

Done when Desktop survives the demonstrated first-paint release scenario, retired
pages are reclaimed, history/Undo/pane moves still work, and the chosen diagnostic
route explains a disposable native fault. A delayed crash or a growing retirement
pool hidden by green model tests is the failure to watch for.

## Not in scope

No install, merge, OS upgrade, Apple report or evidence upload. Dumps, envelopes
and derived evidence stay outside Git. Unrelated database, Sparkle and provider
failover findings in the incident reports select no work here.
