# Task opening latency — October 13

Jack Heart requested Task-click → rendered, responsive native coding-agent input
in LOO-462. No paired baseline/after endpoint timings are available. This run is
headless; neither demo app nor any retained terminal was touched. No native probe,
trust approval, conversation input, live-store mutation or installation occurred.

## Observed path and repair

Retained switching uses the existing workspace/surface without acquiring another
Session. Reconnect/resume and fresh launch acquire the primary through `ensure`,
refresh the exact Task, prepare `connect --json`, then run foreground connect in
Ghostty. Rust owns the fresh-versus-resume choice, attachment lock and provider
launch; Swift does not duplicate that decision.

“Opening conversation” covers primary acquisition; “Opening session” covers
connect/terminal preparation. The avoidable serial stage between them was a
complete repository Session enumeration, even though ensure had already returned
the authoritative Session. WorkModel now uses that record and merges only it into
the latest retained inventory. Exact Task refresh, membership/location checks,
explicit Move here, connect revalidation and foreground ownership remain.
Explicit Session links absent from retained readings still use their existing
lookup path. Provider/network/trust waits are unchanged.

Read-only existing `.lf/desktop-cache/timings/reads.ndjson` inspection:

| Read | Successful samples | Median ms | Maximum ms | Failures |
|---|---:|---:|---:|---:|
| session ensure | 25 | 127.1 | 608.9 | 6 |
| session list | 1061 | 254.8 | 32988.9 | 0 |
| session connect | 27 | 93.6 | 4867.3 | 4 |
| wave show | 25 | 1914.6 | 3779.9 | 0 |

These mixed historical reads lack Task-click correlation; do not sum them, infer
an endpoint baseline or claim improvement in native usability. Prior failed
trust/timeout attempts remain in work-first-demo.md and its referenced receipts.

## Remaining native proof

`recordPaneLive` records surface creation. `task_workspace_ready` restarts at
Session placement and ends at a scheduling/focus proxy. `terminal_key_to_draw`
ends on a Ghostty draw. None alone establishes provider input readiness.
Measure separately: retained switching, reconnect/resume and fresh launch, from
the actual Task click through visible provider input and a rendered response to
unsent draft input. Preserve failures and provider/network/trust waits in each
receipt; do not substitute terminal creation or preparation latency. No numeric
acceptance target was introduced.

Review fixed stale-inventory overwrite during preparation and preserved existing
inventory errors. Regressions cover both, foreign membership, missing Task,
planning failure, cancellation and retained workspace reuse.

Check: `scripts/test_desktop.sh --filter 'WorkDestinationTests|SessionsStoreTests' -Xswiftc -gnone` PASS (63 tests); `git diff --check` PASS. No Rust changed; fmt/Clippy not applicable. Changes left uncommitted for the main contribution.
