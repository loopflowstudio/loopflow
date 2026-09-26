# Iteration 13 review — 2026-09-24

The editor advances the accepted design. Its configured Cancel proof holds,
and this review closes the mounted rejection/draft-retention gap with an
injected transport failure. No production defect or source correction was
established. A real authorized external edit remains unproven.

## Evidence matrix

| Claim | Planned behavior | Implemented behavior | Proof | Result |
|---|---|---|---|---|
| Configured local editing | Exact directive loads; Cancel discards only local input | Real editor reads authoritative text and restores it on reopen | Iteration 13 configured receipt; repeated in this review | pass, configured reads/UI |
| Rejected Save | Keep exact draft, show error and allow retry | Mounted sheet retains multiline text/error through a polling interval and another Save action | Fresh AX trial with intercepted PM transport | pass, production-like failure injection |
| Exact target and authoritative success | Captured Task/Wave; refreshed provider text; old polls cannot undo Save | Existing shared update and Podium generation/readback | Unchanged three-test receipt and source trace | pass, model; real write gap |
| Work/Session identity and retention | Typed joins, explicit absence/unavailability, retained native workspaces | Existing shared readings and window-local owners remain | Prior native/configured receipts and source checks | pass at recorded levels; configured row/nested gap |
| One authority | No duplicate reader, writer, workspace or Session policy | One Podium inventory reader, one root registry, shared PM writer | Negative searches and Rust/Swift contract comparison | pass for ownership; LOO-284 fields absent |
| Full Task proof | Shared Session actions/display path, external edit/trials and published budgets | Those obligations remain outstanding | Current directive and design ledger | gap |

## Demonstration

At 2026-09-24T14:07:44Z, launched owned PID 55642 from disposable
`/tmp/loo291-review13-rejection/Loopflow Rejection.app`. It copies the existing
editor build, with a distinct bundle id and development CLI pointer. Re-signing
and strict signature verification passed. No production source was modified.

The temporary transport forwards only read commands to this checkout's real CLI,
using the same aligned installed development Home. It rejects all PM commands
before invoking the CLI/provider. The app uses real planning and Session reads;
the Save failure is deliberately injected, not a real Linear rejection or write.
The installed app and human demo are unchanged.

The exact runner was Accessibility trusted with one owned accessible window.
Global focus lookup returned -25204; the app was inactive. All actions targeted
owned AX elements, with no global input or activation. The trial:

- selected exact LOO-291 and loaded its full shared directive;
- entered a two-line draft and pressed Save;
- observed the injected error and identical editor text;
- waited sixteen seconds, retaining the draft and error, then pressed Save again;
- confirmed unchanged shared planning, cancelled, reopened the original text,
  and cancelled again.

The waiting interval spans the normal polling period; it is not independent
proof of a completed refresh. The trial exited 0. Before/after Session payloads
match; owned PID 55642 is absent. No provider was launched, opened, transferred
or resolved, and no PM write reached the real CLI. Launch-to-Task AX was
3,869.7 ms and Edit-to-editor AX 1,059.9 ms, including probe overhead. Neither
is pixel paint, a published budget or a qualifying trial series.

[Executed probe](configured-ui-evidence/iteration13-review/rejection-probe.swift),
[transport](configured-ui-evidence/iteration13-review/lf-read-only.sh),
[log](configured-ui-evidence/iteration13-review/rejection.log),
[receipt](configured-ui-evidence/iteration13-review/receipt.json).

## Source review and disposition

Read the Task, current slice, integration amendment, Done When and forbidden
outcomes. Recovered the unrestricted tracked diff in
`/tmp/loo291-review13-tracked.patch`: 207 sections, 1,115,129 characters.
Compared every section with review 12: 203 unchanged, four changed scratch
documents, no executable delta. Inspected the untracked editor test and new
probes separately. Prior executable review remains applicable.

Traced the demonstrated sheet through RegistryQuery's literal argument transport,
CLI error presentation, Rust PM ownership/update/snapshot refresh and Podium's
authoritative readback. Draft, submission error and read generation retain
separate responsibilities. A real PM error may follow a provider mutation when
snapshot refresh fails; this injected pre-write error does not cover that case.

Searches retain one Podium caller each for roadmap, Sessions and Activity, and
one root workspace registry. SessionScope, SessionContext, SessionGroup,
SessionRowItem, PodiumConsole, PodiumSurface, _loadHierarchy and providerLaunch
remain absent. Current main's SessionRecord still lacks shared legal actions
and display path. No new production adapter, writer or Session policy was added.

All four editor source/test hashes match the prior passing three-test receipt;
no unit tests or broad gate were rerun because executable content is unchanged.
The fresh AX proof above is additional behavioral evidence. `git diff --check`
passes.

Next work should use the existing editor for a human-selected external Task and
authorized text, verifying authoritative readback after a real save. Do not repeat
Cancel or injected rejection as substitutes. Preserve the separate configured
Session-row/nested-workspace procedure and earlier provider/viewport receipts.
LOO-284 integration, other scopes/destinations and full external/performance
trials remain open.

No publication, landing or Task completion. The
[review-slice skill](/Users/jack/.agents/skills/review-slice/SKILL.md) permits
publication “When all applicable `Done when` claims hold and the slice is
coherent.” The remaining configured claims leave that condition unmet.
