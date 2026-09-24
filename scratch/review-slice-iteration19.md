# Active Runs contract review — 2026-09-24

The shared CLI slice advances the accepted canvas. Fresh production-like proof
passes for an existing waiting native client, removal after exit, and explicit
unavailable evidence. No additional executable correction was established.
The preceding discovery correction is preserved, not claimed as this review's
work. The complete canvas remains unfinished.

## Evidence matrix

| Claim | Planned behavior | Implemented behavior | Proof | Result |
|---|---|---|---|---|
| Existing native clients remain discoverable | No new publication required from an older launcher | Existing client receipts supply Run ownership | Fresh real CLI with isolated Home and owned waiting provider stand-in; zero bindings created or removed | pass |
| Exit removes activity without resolving the Session | Durable metadata alone never proves liveness | Same Run disappears after its child exits | Fresh CLI read after clean stdin-driven exit | pass |
| Failed observation is distinct from empty | Missing evidence cannot say no active Runs | Corrupt receipt produces JSON gaps and text `Unavailable` | Fresh corrupt-receipt trial, asserting absence of `No active Runs.` | pass |
| Exact Task attribution, old Runs and sequential captures | Shared typed Work and verified ownership, independent of checkout/history caps | Native receipts and capture intervals join one process observation | Existing five-test ownership receipt and CLI launch/resume receipt; source hashes unchanged | pass at recorded local scope |
| Configured native clients remain visible | Active Session Run references occur in active observation | All three preceding active Session references appear among six active Runs | Fresh read-only configured CLI comparison | pass for sequential observations |
| One shared contract | Rust projects ownership/Work; Swift decodes the required shape | One capture writer, shared attribution resolver and matching DTOs | Source trace, negative searches, unchanged Rust/Swift fixture receipts | pass |
| Complete canvas | Retained Task Monitor beside terminals, bounded discovery and both measured experiences | Reader exists; pane consumer and native runners absent; native traversal grows with retained history | Reachable app source and accepted launch decision | gap |

## Demonstrated behavior

Rebuilt `target/debug/lf` successfully. Ran
`uv run python scratch/active-runs-evidence/review19/native_probe.py --expect-discovery`.
The probe adapts the prior discovery review's isolated CLI trial and adds exit
and failed-read assertions. Its local OpenCode stand-in waits on stdin; it is
not a vendor conversation or native UI proof. No configured client was changed.
The owned child exits successfully and the temporary Home is removed.

The [native receipt](active-runs-evidence/review19/preexisting-client-after.json)
records the same waiting PID and Run in both initial reads, no capture marker,
an empty complete observation after exit, and an explicit gap after corrupting
only the temporary client's receipt. The text command also reports unavailable.

The [configured receipt](active-runs-evidence/review19/configured.json) uses the
existing installed development Home with aligned control/data paths. Six Sessions
were returned; all three marked active occur in the subsequent six-Run snapshot.
This observation has no gaps. The earlier review's missing-owner gap was not
reproduced; this does not establish why that earlier process disappeared or
prove an atomic complete inventory. The LOO-291 scoped read returns its exact
Task reference and zero Runs; it supplies no positive configured attribution
example. The same-checkout positive proof remains the existing owned-process test.

Single command durations were 856 ms for the Home active read and 1,160 ms for
the scoped read. These include CLI startup, storage and process observation.
They are neither a baseline distribution nor a diagnosis of traversal cost,
and establish no rendered/usable latency, p95, frame-hitch result or budget.

## Source and review limits

Read the current slice, retained Done When/forbidden outcomes, canvas launch
decision and prior reviews. Captured the unrestricted tracked branch diff at
`/tmp/loo291-review19-tracked.patch` and compared its sections with the preceding
review snapshot. Inspected the changed executable sections and the untracked
active reader, Swift DTO and fixture. Unchanged historical implementation and
UI evidence retain their prior review scope; this is an incremental contract
review, not a fresh gate of all 454 tracked diff sections.

Traced capture publication/settlement, native receipts, process matching,
deduplication, shared Work resolution, CLI filtering, RegistryQuery and both DTO
mirrors. Capture bindings carry exact Exec intervals; native clients retain
their separate existing receipt authority. The active path does not reduce
historical events/usage or apply the history reader's age/count cap. Kernel
activity state describes process observation, not semantic agent progress.

Negative searches retain one production capture-binding writer, one shared Run
Work resolver, one Podium reader for each existing inventory and one root window
workspace registry. The removed native binding writer, SessionScope,
FlowResolutionAction and temporary Session identity writer remain absent.
No production `query.activeRuns` consumer or Monitor pane is reachable yet.

All fourteen source hashes still match the implementation receipt after this
review's CLI trials; [source record](active-runs-evidence/review19/source.json).
Its five Rust ownership tests, Rust/Swift DTO tests, CLI launch/resume test and
clippy pass are prior verification, not new test runs. No executable source was
edited, so those suites were not rerun. Fresh work consists of the build, CLI
trials, source review and `git diff --check`.

## Next implementation boundary

Settle native discovery cost without dropping older clients, then feed a
Task-bound Monitor through Podium's existing read owner and the retained
multiplexer. Preserve unknown/stale readings and native input while Monitor,
Session and companion shell coexist. Implement both defined native performance
journeys and collect their baseline before targeted optimization. Do not replace
these obligations with the single CLI durations above.

The human composition demo, original external-work trials/directive edit and
long-lived-registry budgets remain required. No installation, publication,
Task completion, PM mutation or LOO-293 closure occurred. The
[review-slice skill](/Users/jack/.agents/skills/review-slice/SKILL.md) says,
“When all applicable `Done when` claims hold and the slice is coherent, publish”.
The complete canvas's Monitor and measured-experience claims remain unmet;
publication stays pending.
