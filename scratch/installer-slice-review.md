# Installer lifecycle slice review — 2026-09-26

Reviewed LOO-312, the [complete design](a-newcomer-gets-from-download.md), and
the complete branch diff from `102522bc25adc146d4b8cc3f343677ed8170f636`
through `fb6ce42dc5695787b969874d46aa7cdb88acde31`. The worktree was clean.
Jack's accepted target remains download → drag → open → coding-tool and GitHub
connection → **Open project…**, without an external Terminal command or Linear.

The implementation advances that design through the existing installer owner.
It is a lifecycle prerequisite, not yet the copied-app activation slice described
in the plan. Local process behavior passes; interruption/reopen durability has a
concrete source-level gap. Do not treat this review as acceptance of that slice
or of the newcomer journey.

## Evidence matrix

| Claim | Planned behavior | Implemented behavior | Proof | Result |
| --- | --- | --- | --- | --- |
| First-install shutdown | Stop the selected copied app and its helpers without a prior receipt; preserve other copies | Target paths participate even with `prior: None`; app-ID-wide quit removed | `cargo test -p loopflow --lib app_activation_tests`, native temporary processes | pass |
| Window intent | Helpers alone must not open a window; repeated quiesce retains intent | Only the app executable sets the existing receipt field; update is monotonic | Same command: helper-only and repeated-quiesce cases | pass |
| Reopen selection | Select prior bytes before store advance, target afterward, and copied target for first installation | Selection and tree validation reject changed bytes | Same command: first-install and interrupted-update cases | pass, selection only |
| Same-path staging | Staging must preserve a copied app at its destination | Separate staging tree | Prior unchanged-source `artifact_tests` evidence: 4 passed, recorded in design | pass, file fixtures |
| Coordinator survives shutdown | Bundled published helper executes retained bytes outside the bundle | Added `exec` handoff before ordinary promotion; receipt-directed recovery uses retained owners | Source: `promote`, `delegate_switch_recovery`; no end-to-end execution of the new handoff | gap |
| Done when 1: clean-account DMG path | Signed bundle activation through setup without Terminal | No activation command or Swift first-launch integration yet | `InstallCommand`, `LoopflowApp.init`, `bootstrapLoopflowApp`; no rendering environment or fresh account | gap |
| Done when 2: both guided providers and GitHub | Native acquisition, verified login, then working/cancelable chooser | Guided setup and package-manager-free `gh` acquisition absent from this diff | Full diff and design's outstanding scope; no real sign-in attempted | gap |
| Done when 3: reopen, shell, update, retry | Re-observe setup; fresh shell finds lf; interrupted activation resumes; destination retained; old/DMG bundles handled | Process intent and bundle verification implemented; durable completion gap below; PATH, destination, old/DMG handling unfinished | `settle_switch`, `recover_switch`, `active_install_matches_candidate`; `published.rs` still defaults to `/Applications` | gap |
| Done when 4: real conversation | Provider reply in existing project without Linear | No app conversation exercised | Real app/account boundary unavailable; no simulation substituted | gap |
| Done when 5: behavioral coverage | Installation, auth/default choice, shell, repository navigation, DTO fixtures | Four lifecycle tests plus earlier staging fixtures; broader cases and configured promotion absent | Focused test output and complete diff | gap |
| Done when 6: affected-suite gate | Affected Rust/Python/Swift/UI/download checks and static checks | Prior fmt/clippy passes recorded; this review ran the narrow process proof only | Design ledger and test output below; no affected-suite gate claimed | gap |
| Done when 7: released path | Public instructions verified against published artifacts | No newcomer documentation or public-channel trial in this branch | Complete diff; no publication performed | gap |

## Findings

### P1 — Reopen intent stops being actionable before reopening succeeds

Observed source ordering:

- Published promotion calls `settle_switch` before `resume_switch_app`
  ([install.rs](../rust/loopflow/src/lf/commands/install.rs), lines 3466–3483).
- Machine settlement archives the receipt and removes the active switch
  ([machine_install.rs](../rust/loopflow/src/machine_install.rs), lines 1021–1027).
- A repeat promotion can return “already installed” from
  `active_install_matches_candidate` without checking window/reopen completion.
- `recover_switch` returns immediately for a settled machine; its partially
  settled receipt branch also settles and returns without reopening. Before
  store advancement, recovery clears the switch before reopening as well.

Therefore an exit after settlement but before `open`, or a failed `open`, can
leave a correctly installed app closed with no automatic retry of the recorded
intent. The archived fact survives; the recovery obligation does not. This is
an existing surrounding lifecycle limitation exposed by the new first-install
requirement, not a claim that this branch introduced the settlement ordering.

The four passing tests call `app_to_resume` or repeat quiesce; they never cross
settlement, execute `open`, or retry promotion. They cannot disprove this failure.
The failure sequence is inferred from source, not reproduced by interrupting a
real installation.

Resolve completion ordering in the existing receipt owner and prove interruption
on both sides of settlement. Do not simply move `open` earlier without checking
which installation the reopened app's helpers may use. This spans machine
registration, startup authorization, and recovery; it needs an implementation
slice and configured-path proof, not an isolated reorder in this review.

### P2 — The new retained-helper handoff has no boundary proof

`promote` stages and re-execs the current bundled helper, preserving its PID,
arguments, environment, and inherited descriptors. That removes its executable
from the selected shutdown paths. No current test executes that branch or
terminates its GUI parent. Survival of the actual parent/child lifecycle and
output handling remains unproven. Exercise the real helper from a temporary
bundle in a disposable account, including parent exit and recovery; do not use
an ordinary HOME override as machine-install isolation.

## Architecture and forbidden outcomes

Searched installer launch/recovery paths, machine receipt types and writers,
Swift bootstrap and bundled helper selection, shell installer validation, and
published update destination selection.

- `SwitchReceipt.app_was_running` remains the sole restart-intent field.
  Production writes remain receipt construction and `quiesce_switch_app`;
  receipt validation prevents clearing previously observed intent.
- `ArtifactSet`, `ActivationTargets`, the existing binary store, and machine
  receipts remain the owners. No second install registry, credential store,
  provider preference, onboarding boolean, or Swift promotion logic was added.
- No reachable app-ID-wide quit remains in Rust, release, or Swift source.
  The shutdown path matches executable paths before sending signals.
- Signature verification remains in `release/install.sh`; the Rust tree/helper
  checks are not signed-bundle acceptance. An activation entry must reuse shared
  signature verification rather than bypass it.
- `install/published.rs` and `release/install.sh` still choose `/Applications`
  absent an explicit override. The actual copied destination is not yet an
  update authority. The existing retired-app cleanup is upgrade handling, not
  a newly introduced competing installer.

No forbidden duplicate authority was introduced. This is useful groundwork;
it does not create a dead end, but the next slice must close lifecycle completion
and expose the signed activation path before claiming the first design slice.

## Proof and next action

This review ran `cargo test -p loopflow --lib app_activation_tests` on macOS:
**4 passed, 0 failed**, 0.52 seconds after compilation. Native `/bin/sleep`
copies under temporary bundles exercised real path discovery and signals.
These are process fixtures, not a real app, signed release, GUI reopen, machine
promotion, or clean-account trial. Existing artifact and static-check evidence
is retained from the design for unchanged source; it was not rerun for ceremony.

No production code changed. No production installation, account, login, or
planning record was mutated. Corrected the stale selected-step note and recorded
this review; the earlier compress review remains historical evidence.

Next implementation should preserve pending reopen through interruption in the
existing owner, add the signed copied-bundle entry, and prove the retained helper
and registration/recovery path in a disposable macOS account. Shared PATH setup
and destination retention remain in that slice. Obtain the running-app baseline
before UI edits, then proceed with setup actions and the complete clean-account
acceptance. No new product decision is needed for these engineering steps.

The applicable copied-app activation and Task acceptance claims do not all hold,
so this review does not publish, land, complete the Task, or select Flow navigation.
