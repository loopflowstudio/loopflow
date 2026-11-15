# Browser lifetime diagnostic

```sh
uv run python scripts/benchmarks/browser-lifetime/run.py --check
uv run python scripts/benchmarks/browser-lifetime/run.py --compare
```

`--check` compiles the standalone AppKit/WebKit child and records the macOS
build, loaded WebKit image path/UUID and framework version without opening a
window. `--compare` requires a logged-in display session and an unoccluded window.
It opens only a disposable diagnostic window in each child, never Loopflow Desktop. The children
use nonpersistent WebKit data and inline deterministic HTML, without an lf store,
provider or remote page. No server stays running.

Each pair has the same HTML, navigation anchor, release offset and observation
time. The parent sweeps 0, 10, 25, 50, 100 and 200 ms after the selected
`--anchor` (`load-request`, `navigation-commit`, `navigation-finish` [default], or
`animation-frame`), across single release, two closes/Undo replacement, shell-close/pane-hide replacement,
reparent, workspace disposal, SwiftUI retention/reparenting, window transfer and
occlusion. Window transfer uses the last screen when more than one is available;
a single-screen host exercises a window change only. The ownership sequences
simulate Desktop;
`BrowserPageLifecycleTests` separately exercise the production addressed Close
path. A final cleanup checks that even the page restored for Undo is reclaimed.

Inline canvas and CSS animations keep compositor work active while the page lives.
SwiftUI scenarios use an `NSHostingView` and observable presentation; an old
representable/container can retain a page after pool removal, as in Desktop.
Reparent waits for attachment to the new native container so SwiftUI cannot
coalesce it with the close. These scenarios simulate ownership; they do not run
`WorkspacePanesView`.

The baseline drops the page immediately; SwiftUI controls container detachment in
its scenarios. Experimental alternatives stop loading, stop/detach with bounded retention, or load `about:blank` until navigation
completion/failure (with the same retention bound as a fallback). `--hold-ms`
defaults to **500 ms for comparison only**. No variant is a selected fix. Other
scenarios explicitly detach because a container otherwise owns the view beyond
pool removal.

Navigation completion and JavaScript animation-frame messages are **not paint
acknowledgements**. Paint PerformanceObserver entries are recorded when WebKit
supports them; unavailable observations stay unavailable. These observations do
not establish that the UI process committed the first layer tree. The parent
counts crashes, other failures, incomplete trials, native final releases and
retained pages. A missing completion record never passes, even with exit status 0.
A trial must observe an unoccluded window before navigation and a nonzero visible page before
release; a console session alone is insufficient. Unavailable displays are
reported separately and stop the sweep immediately. An absent animation callback
never supplies an anchor.
A final-release marker means native destruction began, not that WebKit drained.

```sh
uv run python scripts/benchmarks/browser-lifetime/run.py --compare \
  --scenario two-close --offset-ms 25 --attempts 10
```

Logs and results default to the private incident directory under
`~/Library/Logs/Loopflow/desktop-exit-minidumps-2026-11-02/browser-lifetime/`.
`--output` must be outside Git checkouts. Each run saves its compiled source,
executable hashes and toolchain alongside the receipts. Keep crash dumps, envelopes and derived
private evidence outside Git and do not upload them. Interrupting or timing out
kills and reaps the owned trial process. The probe starts no provider or server.
Do not run intentional faults in an installed Desktop.

Survival without a reproduced baseline crash does not establish a repair.
The [native reporter diagnostic](reporter/README.md) checks the isolated
reporter-disabled build against the signal journal and macOS reports. A real Desktop
history/pane/Undo walkthrough remains separate.

```sh
uv run python scripts/benchmarks/browser-lifetime/run.py --scenario swiftui-reparent \
  --anchor animation-frame --offset-ms 0 --attempts 10
```
