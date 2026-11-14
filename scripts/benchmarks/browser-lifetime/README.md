# Browser lifetime diagnostic

```sh
uv run python scripts/benchmarks/browser-lifetime/run.py --check
uv run python scripts/benchmarks/browser-lifetime/run.py --compare
```

`--check` compiles the standalone AppKit/WebKit child and records the macOS
build, loaded WebKit image path/UUID and framework version without opening a
window. `--compare` requires a logged-in display session. It opens only a
disposable diagnostic window in each child, never Loopflow Desktop. The children
use nonpersistent WebKit data and inline deterministic HTML, without an lf store,
provider or remote page. No server stays running.

Each pair has the same HTML, navigation anchor, release offset and observation
time. The parent sweeps 0, 10, 25, 50, 100 and 200 ms after `didFinish`, across
single release, two closes/Undo replacement, shell-close/pane-hide replacement,
reparent and workspace disposal. The ownership sequences simulate Desktop;
`BrowserPageLifecycleTests` separately exercise the production addressed Close
path. A final cleanup checks that even the page restored for Undo is reclaimed.

The baseline drops the detached page immediately. Experimental alternatives stop
loading, stop/detach with bounded retention, or load `about:blank` until navigation
completion/failure (with the same retention bound as a fallback). `--hold-ms`
defaults to **500 ms for comparison only**. No variant is a selected fix. All
variants detach because a container otherwise owns the view beyond pool removal.

Navigation completion and JavaScript animation-frame messages are **not paint
acknowledgements**. Paint PerformanceObserver entries are recorded when WebKit
supports them; unavailable observations stay unavailable. These observations do
not establish that the UI process committed the first layer tree. The parent
counts crashes, other failures, incomplete trials, native final releases and
retained pages. A missing completion record never passes, even with exit status 0.
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
Reporter-disabled Ghostty builds, native-fault diagnosis and a real Desktop
history/pane/Undo walkthrough remain separate experiments.
