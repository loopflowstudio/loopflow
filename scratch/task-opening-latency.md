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

## October 13 profiling implementation

Jack Heart requested separate Codex/Claude cached/resume/fresh input trials.
`agent_input.py` records the existing Desktop signposts and opt-in lf wall
milestones, process ancestry/CPU snapshots and all-process Time Profiler stacks.
The report correlates actual Task click, preparation/connect, launch lock,
fresh/resume selection, spawn, surface, key and draw with native observer captures.
Flamegraphs use real exported samples; missing exports/symbols remain unknown.
Exact-Session input plus ordered captures are required; first draw cannot produce
an endpoint. Provider/case/condition mismatches refuse before/after comparison.
Reproducible commands, provenance fields and observer limits are in
scripts/benchmarks/desktop-performance/agent-input.md.

No paired native receipts or further measured optimization is claimed. The lookup
repair above removes a known redundant serial read. Full six-case timing, CPU/wall
wait attribution, comparable before/after, native hook proof and review remain.
Neither existing app, pane nor draft was touched. No provider input/takeover,
installation or live migration was performed.

Review removed false CPU accounting: only Running time-profile samples enter the
flamegraph, and process CPU deltas exclude preexisting CPU. A real 1-second
headless export produced 6,452 samples and a valid SVG; this validates the parser,
not any provider endpoint. Its first premature export failed before trace saving;
completed export passed. Rendering exposed a quadratic tree walk, replaced by
one parent/child index (final real-export render/parse 0.38 s).

Unlaunched pre-compression debug app: `/tmp/loo462-oct13-profile/Loopflow Work Demo.app`;
validation-only bundled lf, signature verified, private build receipt `build.json`.
Initial signing rejected root SwiftPM resources; moved them under Contents/Resources.
SwiftPM resolves its existing retained-checkout build-path fallback; this private
artifact is not portable without that build directory.
This artifact predates `ae691073c9` and main integration; the later demo requires
a fresh matching build. Neither existing app nor its drafts was changed.
Check: 38 WorkNavigation tests plus two final chrome/Workflow regressions PASS; seven profiling Python tests and one Rust timing test PASS; real xctrace export/SVG, cargo fmt/all-target Clippy, Ruff/diff PASS. Native six-case timing/trust proof remains with demo; affected-suite gate remains with gate. Review kept each Rust append in one
serialized buffer so concurrent lf processes cannot interleave formatting writes.
