# Task opening latency — October 13

Jack Heart requested Task-click → rendered, responsive native coding-agent input
in LOO-462. No paired baseline/after endpoint timings are available. Native facilities are
unavailable; neither demo app nor any retained terminal was touched. No native probe,
trust approval, conversation input, live-store mutation or installation occurred.

## Observed path and repair

Retained switching uses the existing workspace/surface without acquiring another
Session. Reconnect/resume and fresh launch acquire the primary through `ensure`,
refresh the exact Task, prepare `connect --json`, then run foreground connect in
Ghostty. Rust owns the fresh-versus-resume choice, attachment lock and provider
launch; Swift does not duplicate that decision.

"Opening conversation" originally covered primary acquisition; "Opening session"
covered connect/terminal preparation. Both now say "Opening conversation". The
avoidable serial stage between them was a
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

Check: `scripts/test_desktop.sh --filter 'WorkDestinationTests|SessionsStoreTests' -Xswiftc -gnone` PASS (63 tests); `git diff --check` PASS. No Rust changed; fmt/Clippy not applicable. Lookup changes checkpointed in `0bb13e9261`.

## Profiling evidence and remaining proof

The recorder and six-case acceptance contract are maintained in
`scripts/benchmarks/desktop-performance/agent-input.md` and the design's
**Profiling and optimization** section. Implementation: `5b62453be7`; reduction:
`ae691073c9`. Exact-Session ordered captures are required; failed or incomplete
trials never supply elapsed comparison values. CPU samples cannot explain waits.

A real 1-second headless Time Profiler export produced 6,452 samples and an SVG;
its first premature export failed. Review restricted stacks to Running samples,
excluded preexisting CPU and replaced a quadratic tree walk (0.38 s final
parse/render). This validates the parser, not a native provider endpoint.
Full evidence, checks and the earlier unlaunched app/signing failure remain at
`69f7005b05:scratch/task-opening-latency.md`, **October 13 profiling implementation**.
The current review artifact is recorded in `get-me-using-lf.md`.

No paired native timings, measured speedup or hook approval-once proof exists.
Codex/Claude cached/resume/fresh trials and native trust remain with demo; affected
suites remain with gate. Existing apps, retained panes and unsent drafts survive.
