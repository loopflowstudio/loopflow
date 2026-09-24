# Configured UI proof — 2026-09-23

The configured AX/provider boundary is now usable in this runner. Exact draft
fidelity, configured repository/scroll retention, visual quality, and a controlled
before/after timing comparison remain open. Do not repeat a generic AX-blocked
review or count these Loopflow trials as external-product evidence.

## Build and host

Review app: `/tmp/loo291-review/Loopflow Review.app`, bundle identifier
`studio.loopflow.review.loo291`. It contains this checkout's SwiftPM executable,
its existing resource bundles, and `LoopflowDevControl.json` selecting the
installed `/Users/jack/.local/bin/lf`. No fixture/capture mode or installed-app
replacement was used. Build:

```sh
swift build --package-path swift -Xswiftc -gnone --jobs 4 --product LoopflowMac
```

The build passed. Both built and review-bundled executable SHA-256:
`046a302a6902304cae9cd14899992796cb239cce764b46a8ac0bd082f27d0d87`.
Receipt: `/tmp/loo291-iteration6-app-build.log`.

`xcrun swift /tmp/loo291-configured-navigation.swift` launched its own instance
with `createsNewApplicationInstance=true`, `activates=true`, and the actual
Task checkout. In that exact probe: AX trust true, AXWindows status 0/count 1,
one owned onscreen WindowServer window, accessible Task controls. `isActive`
was false in the navigation run and true in the provider runs. Requesting
activation and observing activation are separate facts.

The control conversation's `/tmp/loo291-foreground-probe.log` instead reports
trust false, AXWindows -25211, and an onscreen owned window. Earlier
`/tmp/loo291-configured-probe.log` reports trust true with only app/menu nodes.
Neither observation is generalized to another runner. No permission was changed
or bypassed; why the contexts differ remains unknown. If another host reports
untrusted, enable **System Settings → Privacy & Security → Accessibility** for
the application hosting that automation runner, then repeat its own trust/window/
control check. The review target app existing onscreen is not permission proof.
This runner currently needs no permission change for AX interaction.

## Product correction

The first accessible launch exposed a real planning failure: the inherited
`LF_WAVE_ID` selected the launching agent's Wave even for `roadmap --all` and
failed repository resolution from the app's `/` cwd. The existing CLI process
boundary now removes that ambient Wave variable. Explicit targets and Home
configuration remain intact. Before: `/tmp/loo291-configured-values-probe.log`.
After: [navigation receipt](configured-ui-evidence/navigation.log), deliberately
launched with an unrelated Wave variable. No new reader, DTO, or Session policy.

## Evidence and limits

| Claim | Configured observation | Result |
|---|---|---|
| Planning and navigation | Actual Task directive and Project/KRs, explicit no-Session state, A/D, exact Task search, search retained through polling | pass, AX/live registry |
| Explicit transfer | Existing user Session selection exposes Move here without transfer; only the new proof-owned Session was transferred | pass |
| Native continuation | Same provider conversation answered `marigold` after a new message submitted through the embedded terminal | pass, provider history |
| Retained process and companion | Provider PID/birth receipt unchanged through A/D, details, overview and adding a shell split; original shell responds after completion | pass, configured process evidence |
| Exact unfinished draft | Some draft survived, but final user message was `Reply with only thcontinuity word from my first message.` | gap; two missing characters, cause unestablished |
| Completion without Task completion | UI Complete removes Session pane; companion responds; details show no Session; CLI confirms Session absent and Task incomplete | pass, configured UI/CLI |
| Launch reliability | One trusted, window-present attempt exposed zero Task controls within the observation window | gap; unavailable observation is not a measured successful paint |
| Repository/scroll/visual retention | Earlier native fixtures cover these; this configured trial does not | gap |

Proof-owned Run: `run_edb7d3ad4ac94ddcbaeca8679503845f`, explicitly attributed to
LOO-291 through `lf --task LOO-291 --tui --interactive : …`. The prompt prohibited
edits/tools and asked for a fixed response and remembered word. Native provider
conversation: `01a0d0fe-7860-7393-ab3d-d6e687a7ea22`. Initial provider PID 64478
handed off cleanly; its launcher printed `Session moved to another terminal.`
The retention trial kept PID 85170 and birth `2026-09-24T01:25:29.477663Z` through
navigation. Companion PID 93956 replied `93956:AFTER_COMPLETE`. Both were absent
from `ps` after the owned app exited. No user-owned Session was moved or resolved.

[Provider messages](configured-ui-evidence/provider-messages.json) separate the
exact successful continuation at 01:21:52–54 UTC from the later imperfect draft
at 01:25:45–49 UTC. The executed retention script accepted a correct reply as
draft proof, which was too weak. Its archived source now also requires the exact
submitted text; that strengthened assertion was not rerun against the completed
Session and is not a passing receipt. The missing characters may have been lost
during synthetic input or navigation; neither explanation is established.

The initial provider probe also missed a real button because SwiftUI inherited
its parent AX identifier; inspecting the actual AX label corrected the probe.
The first retention probe counted historical dead client receipts as live; it
was corrected to require a live PID and preserve its exact receipt across the
trial. These were probe assumptions, not production fixes.

Raw configured receipts: [provider](configured-ui-evidence/provider.log),
[retention/completion](configured-ui-evidence/retention-completion.log),
[unobserved launch](configured-ui-evidence/unobserved-launch.log).
Final CLI receipts: `/tmp/loo291-iteration6-final-{sessions,roadmap}.json`.
LOO-291 remains `completed: false`; the proof Session is absent. This was an
ordinary interactive Session, not a promoted Ask or blocked-caller release.

## Scoped timings

Monotonic clock around LaunchServices submission and AX observation; warm
SwiftPM development build, long-lived registry. Polling/traversal overhead is
included. These measure accessible controls, not rendered pixels. Population
changed when the disposable Session was added, so these are not a controlled
before/after comparison, cold-launch series, p95 budget, or twenty-trial result.

| Observation | Milliseconds |
|---|---:|
| Launch → observed Task, navigation run | 2535.7 |
| Launch → observed Task, three-selection run | 2647.0 |
| Task selection → observed directive, trials 1/2/3 | 441.7 / 320.6 / 345.1 |
| Launch → observed Task, provider run | 4148.9 |
| Launch → observed Task, first retention probe | 2943.5 |
| Launch → no Task controls observed, deadline reached | 21224.2 |
| Launch → observed Task, final retention run | 4725.7 |

The failed observation is retained, not folded into successful latency. Its log
prints `launch_to_observed_task_ms` before the failed assertion; that label does
not establish an observation. Its cause was not captured. The following bounded
attempt included a diagnostic control dump but succeeded, so supplies no cause.
The 8/12-second provider startup waits and 16-second polling wait are setup
intervals, not readiness/interaction measurements. Three selection timings are
in [the timing receipt](configured-ui-evidence/timing.log).

## Next configured procedure

Open the review build above on the same repository. Use a newly authorized
proof conversation; the recorded one is completed. Historical probe sources
under `configured-ui-evidence/` target that retired identity and must not be
reused against an arbitrary matching title or another person's Session.

1. Verify this runner's trust, owned window and controls independently. Open
   LOO-291 details and select its exact proof Session. Transfer only that owned
   client if required. Preserve its provider conversation and client receipt.
2. Type a short unfinished draft through normal native input and inspect its
   exact composer text before navigation. Keep a running companion shell. Toggle
   A/D, inspect Project/Task details, search/collapse, switch repositories and
   return. Compare exact draft, client, layout, focus and both scroll positions.
3. Submit the retained draft and compare the exact provider-history user text,
   not just a plausible reply. Complete that disposable Session through UI and
   verify pane removal, Task survival and the same companion response.
4. Record one fixed planning/Session population and defined paint/readiness
   endpoints for both builds. Retain timeouts/failures as outcomes. Publish the
   resulting scoped budgets before scoring the required trials.

Full LOO-291 still includes shared LOO-284 integration, bounded conversations,
lf-new workspace integration, directive editing, human-selected external work
and its original trial/budget obligations. No publication, landing or Task
completion is established by this implement pass.
