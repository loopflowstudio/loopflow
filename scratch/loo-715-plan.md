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

## Current slice — expose and reproduce the lifetime

1. Add lifecycle diagnostics through `DesktopLog`: workspace/tab identity, page
   instance, create/load/retire/detach/final-release reason and monotonic age.
   Log URL scheme only; omit credentials, host, path, query and fragment. Include
   close replacement and workspace loss. Logs failing must not alter behavior.
2. Add a disposable AppKit/WebKit harness under `scripts/benchmarks/browser-lifetime/`.
   It uses deterministic local HTML, a visible window, separate child processes,
   no installed Desktop or real store, and a parent collecting exit status.
   Sweep release at 0, 10, 25, 50, 100 and 200 ms after navigation completion;
   separately record first-paint observations without claiming `didFinish` is paint.
   Include rapid two-tab close/Undo replacement, shell/pane replacement, reparent,
   and workspace teardown. Report attempts, failures, actual final release and
   retained page counts per case; identical content and timing for paired variants.
3. Keep dumps, envelopes and derived private evidence outside Git under the
   existing private incident directory. Record OS build and loaded WebKit image
   version for every trial. No simulated crash runs inside Jack's live Desktop.

Focused check: `scripts/test_desktop.sh --filter BrowserPageLifecycleTests`
(new suite) verifies redaction, reuse and close/Undo ownership without a window.
Display trials are separate capable-host diagnostics, never a prerequisite to
finish this headless slice or a fabricated headless success.

## Remaining implementation and diagnostics decision

Implement the winning retirement mechanism in `BrowserPages`, removing immediate
dictionary-drop disposal and correcting its “until tab closes” documentation.
Handle owner teardown through the same resource owner, with no second page cache.
Prove eventual weak-reference release after containers and observers detach;
no timer/delegate cycle may keep a workspace alive. Preserve existing tab/pane
tests and add production-action coverage for the replacement actually discarded.

Evaluate both reporter routes before recommending production policy:

- In a separate diagnostic artifact build the pinned Ghostty with `-Dsentry=false`.
  `scripts/loopflow-dev.py:cmd_ghostty_build` currently omits that flag, while
  `swift/Package.swift` downloads a pinned binary: editing only the local builder
  would not change production. Test an intentional native fault in a disposable
  child initialized like Desktop; require signal journal plus matching `.ips`.
  Check terminal initialization and artifact/provenance/notices implications.
- Prototype fatal-envelope correlation in `desktop_exits.rs`, using synthetic
  envelopes only in Git. Match executable identity and crash time within the
  specific launch interval; reject unrelated Ghostty/Dev crashes, PID reuse and
  ambiguous matches. Report exception and existing dump path without reading or
  printing dump memory. Missing/rotated/unreadable evidence stays unknown.
  No runtime consumer of these files was found in Swift, Rust or scripts;
  third-party notices do include Sentry/Breakpad.

Present measured results and a recommendation to Jack before changing reporter
policy. Correct the misleading Mach-port assumptions in `ExitSignals.swift` and
`desktop_exits.rs` regardless of the route. Diagnostics alone do not finish the fix.

## Gate, demo and done when

Headless gate runs once on the final tree:

```sh
uv run python scripts/test.py --swift --loopflow
```

The Swift suite includes `BrowserPageLifecycleTests`, `ToolTabTests`,
`MultiplexerStoreTests`, `LaunchJournalTests` and `DesktopLogTests`; focused
implementation checks do not require running the whole gate early.

If Rust correlation changes, run `cargo test -p loopflow --lib desktop_exits::`
with inherited `LF_*` unset and stdin `/dev/null`, then `cargo fmt` and
`cargo clippy --all-targets -- -D warnings`. Expected: real addressed close →
replacement retirement → Undo of the latest tab retains its page/history;
reparent/hide retains identity; discarded pages deallocate; unrelated crash
evidence never becomes an attributed crash. The app and fallback build compile.

Diagnostic command: `uv run python scripts/benchmarks/browser-lifetime/run.py --compare`.
On a capable display host it opens only its own disposable window and reports
baseline versus experimental candidates across the sweep. If unavailable, record the
missing display proof explicitly and retain it for demo/review; do not call the
race repaired from headless tests alone. A separate isolated Desktop walkthrough
shows Back/Forward, pane moves, two closes and Undo without restart or lost history.

Done when Desktop survives the demonstrated first-paint release scenario, retired
pages are reclaimed, history/Undo/pane moves still work, and the selected diagnostic
route explains a disposable native fault. Wild success is uneventful rapid tab use;
wild failure is a delayed crash or growing retirement pool hidden by green model
tests. Both are counterexamples the paired trials and reclamation checks must catch.

## Checks and proof limits

- Documentation step (November 14): `swift/README.md` describes intended browser
  Close/Undo, history, pane-move and reclamation behavior, and clarifies the limits
  of exit evidence. These edits precede implementation and establish no repair
  or release proof. Read this file in the next implementation step.
- `lf open --host desktop --file swift/README.md` — no Desktop Files view
  attached; nothing opened. The original doc remains at that path.
- Open product question for Jack Heart: Should production disable Ghostty's
  crash reporter after isolated validation, or retain it and correlate fatal
  envelopes in `lf doctor`? Neither route is settled; the docs promise neither.


## Delete — do not maintain

- Replace the unobserved `BrowserPages` raw `WKWebView` allocation with one native
  subtype carrying diagnostic identity. Keep the one dictionary and the existing
  Close/Undo owner; do not add another tab cache or disposal authority.
- Remove the claim that a catchable signal necessarily reaches Ghostty's reporter
  and the doctor text that omits native reporters from unknown exit causes.
- Once a retirement mechanism wins paired trials, replace dictionary-drop disposal
  in that same owner. Immediate disposal remains the experimental baseline until
  then; it is not silently repaired by logging.
- Curate the two inherited incident reports after preserving their complete
  uncommitted originals outside Git. Unrelated database, Sparkle and provider
  failover proposals select no work here. The curated reports name the originals
  and SHA-256 checksums.

## November 14 implementation evidence and remaining work

The diagnostic slice now traces the production page pool and native view without
changing disposal policy. Native destruction can lag pool retirement while a
container owns the page. Tests cover addressed background-tab Close, two closes
and Undo identity, shell/pane displacement, reparenting, workspace loss,
reclamation, redaction and an unwritable log. The single pool remains the owner.
The reporter policy is unchanged; missing system evidence now explicitly leaves
the cause unknown.

The standalone visible-window harness covers all six ownership scenarios and
four teardown variants, with each child reaped by the parent. Source snapshots,
executable hashes, toolchain, environment and event receipts stay in the private
incident directory. The first prototype comparison completed 48 trials (single
and two-close, one attempt for each offset 0/10/25/50/100/200 ms and variant),
with zero failures/crashes and no retained pages. Its two-close sequence created
the second page after the timing anchor; the current harness preloads it so
creation cost does not shift the requested retirement offset. This is harness
validation, not evidence selecting a workaround.

The actual loaded WebKit image in new trials is
`/System/Library/Frameworks/WebKit.framework/Versions/A/WebKit`, UUID
`1501F244-BE5E-3AD9-ACD8-E9A8880CE2E5`, bundle version `21622.1.22.11.15`,
on macOS 26.0.1 (25A362). The historical report's different WebKit version label
is not evidence of an OS upgrade or a matching/different binary.

Remaining: demonstrate a baseline failure (including the actual SwiftUI/Undo
release trigger if the standalone case cannot reproduce it), then select and
implement a measured retirement mechanism covering owner loss, failure and
WebContent death. Complete both crash-reporter diagnostic alternatives before
recommending policy to Jack Heart. A real isolated Desktop Back/Forward, pane,
Close/Undo walkthrough and full gate remain later acceptance. No race repair,
reporter recommendation, installation or shipping is established by this slice.

The final harness comparison (`run.py --compare --offset-ms 0 --offset-ms 200
--attempts 1`) completed 48 visible-window trials across all six scenarios:
zero failures/crashes, 64 native page releases, zero retained pages. Receipts
and the compiled source are at the private incident directory's
`browser-lifetime/20261114T165018.890143Z/`; compiled source SHA-256 is
`9538588d1d6490abb3f66ea733577c130bfd9cff416d9ab0d6aa7d87ae6c33a5`.
The earlier prototype receipts are under `browser-lifetime/20261114T164635.532353Z/`.
Paint entries were supported in the final trials: 49 first-contentful-paint
observations for 64 pages. Missing observations remain missing; none acknowledges
UI-process layer-tree completion. With no failing baseline, these trials select
no production retirement mechanism. The source README's crash-survival behavior
remains an acceptance target, not an established repair.

Focused checks: `scripts/test_desktop.sh --filter BrowserPageLifecycleTests` — pass (5 tests, including two replacement cases); `uv run pytest python/tests/test_browser_lifetime.py -q` — pass (7 tests); `cargo test -p loopflow --lib desktop_exits::tests::an_unrecorded_end_asks_the_system_log_within_the_heartbeat_window` — pass (1 test). Inherited `LF_*` was cleared and stdin closed. `cargo fmt --all -- --check`, Ruff, the Swift multiplatform boundary check and `git diff --check` pass. Full gate/Clippy and fallback build remain with gate; crash repair, reporter experiments and product walkthrough remain open.

Review finding resolved: the shell replacement fixture mixed a pane incarnation
with a tab target and was correctly rejected. It now addresses the selected pane
through the production action, matching keyboard Close semantics. No production
identity check was relaxed. All diagnostic child processes and test runs exited;
no server or watcher was started.

## Concept review — November 14

The experience stays ordinary: Close A, Close B, Undo restores B with its live
history; A becomes disposable. Moving or hiding a pane keeps its contents live.
A hidden pane is not a discarded tab, even after its Undo entry is displaced.
Workspace loss disposes all its native pages. No new tab state, recovery command,
DTO or persistence is needed. `MultiplexerStore.lastClosed` owns Undo;
`BrowserPages` owns native resources, and containers can extend actual lifetime.
Diagnostic lifetime objects are evidence only, never a second disposal authority.

The biggest mismatch is the README's unqualified “Closing a page while it is
still loading keeps Desktop open.” That is the required outcome, not a behavior
proved by this diagnostic change. Next implementation must visibly mark this
claim as proposed until the release trigger and repair are demonstrated, and
clarify that hidden pane contents still count as live pages. Leave the existing
Close/Undo interaction and clear benchmark instructions alone; no skill change
is needed. A bounded retirement mechanism remains conditional on discriminating
evidence, not the default next implementation.

Normal path: Close A → Close B → Undo B preserves B's page identity and history.
Failure: the recorded native crash bypasses journal hooks; exit status 1 and
absent system reports cannot name its cause. Recovery: relaunch restores access
but does not repair the race or promise restoration of live WKWebView history
across process death. Future diagnostics must identify available evidence or
say unknown, without dumping memory. The 48 surviving trials, including immediate
release, remain the counterexample to claiming a delay has fixed the crash.

Open product choice for Jack Heart: should production keep Ghostty's reporter
and correlate local fatal envelopes in `lf doctor`, or disable it after isolated
signal-journal/macOS-report validation? This determines which evidence survives
and whether the shipped Ghostty artifact changes. Neither route has completed
its diagnostic comparison; this review supplies no policy recommendation and
Jack's “lets do it” does not settle the choice. Evidence must stay private under
either route. No upload, install, merge or OS change is included.

Next demonstration: a failing isolated release sequence and a paired repair that
also reclaims pages; then the two reporter experiments and their measured tradeoff.
Missing reproduction is an engineering evidence gap, not a request for a new
product preference. Outcome for loop-or-next: **Blocked** on the reporter policy
question above. The documentation clarification is next implement direction;
no production code or execution was changed during concept review.
