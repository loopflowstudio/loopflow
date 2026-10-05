# Ordinary reconnect loses the native draft

```bash
uv run tests/e2e/codex_connect.py --lf target/debug/lf \
  --codex /Users/jack/.local/bin/codex --connect-performance 1 \
  --check-reconnect-draft --output /tmp/loo-378-reconnect-final-20261005
```

October 5, 2026 UTC; macOS 26.0.1 arm64. The executable SHA-256 is recorded in
each result. These are private Loopflow/provider Homes, real native Codex UIs
and a local synthetic Responses server. No configured account is used.

The final probe exits 1. The first UI displays the exact unsent draft recorded
in `final/results.json` and `final/reconnect-original.bin`. A second ordinary
`lf session connect`, without `--replace`, keeps Session, thread, generation,
engine PID and OS birth timestamp. It transfers the driver. Appending a new
marker and pressing Enter submits only that marker: `final/reconnect-input.json`
contains no original draft. The second UI therefore accepts input, but its
composer does not preserve the first UI's draft. This is not a readiness timeout.

All attempts remain here:

- `enter-only`: no request after Enter; insufficient readiness evidence, and
  process-group cleanup returned Operation not permitted, obscuring the failure.
- `input-marker`: accepted marker without draft; cleanup timeout obscured the
  primary error.
- `cleanup-retained`: same behavioral failure; cleanup error retained separately.
- `final`: retains the exact submitted user message and checks engine continuity;
  fails with `successful reconnect accepted input but lost the draft`.

The second connection's cleanup still timed out after termination/kill attempts;
the outer fixture also stops the first client, seed and its recorded engines.
A subsequent exact-path process inspection found no remaining final-probe lf
process. That does not establish reliable reconnect cleanup; the timeout remains
an unresolved failure. No unrelated process was signaled.

These evolving single-sample probes establish neither a comparable distribution
nor speedup. They do not exercise the Flow-review selector, paginated history,
authenticated providers, or physical presentation. The source review found the
Flow-review launcher already has a native UI in the tmux cradle, while its resume
path stops/recreates that UI. Moving only the engine to an app-server on first
launch cannot repair the demonstrated UI-local draft discontinuity.
