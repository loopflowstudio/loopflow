# Installer completion review — 2026-09-26

Reviewed LOO-312, Jack's accepted [design](a-newcomer-gets-from-download.md),
growth memory, and the complete branch diff from `102522bc25adc146d4b8cc3f343677ed8170f636`
through `c64bfbc621928fe5b0c1ab39454bc33db9b29320`. The worktree started clean.
The implemented scope is the installer lifecycle prerequisite and its completion
repair, including retention-model compression. The full target remains
download → drag → open → coding-tool and GitHub connection → **Open project…**,
without an external Terminal command or Linear.

The earlier receipt-ordering defect is repaired in source and isolated behavioral
proof. This advances the accepted architecture. It does not complete copied-app
activation or establish clean-account acceptance. No new bounded production-code
defect was found in this review.

## Evidence matrix

| Claim | Planned behavior | Implemented behavior | Proof | Result |
| --- | --- | --- | --- | --- |
| Selected shutdown and window intent | Stop selected copies; helpers alone request no window; retain intent | Destination paths participate without a prior receipt; intent is monotonic | Seven-test lifecycle command below, native temporary processes | pass |
| Completion and startup | Commit target before reopen; preserve failed reopen for recovery | Promotion and recovery commit existing receipt before completion; helpers route to target | Source callers and first-install/published/development fixtures with authorization and failed open | pass, isolated boundary |
| Interrupted completion and retention | Retry after open/partial archival without losing releases or restarting app | One constructor preserves history; running verified executable satisfies reopen | Fixture retries before archival and after archive/active-record writes; process survives | pass, isolated boundary |
| Unadvanced cancellation | Preserve prior selection until prior app returns | Home resume precedes cancellation; reopen precedes clear | Update fixture: failed reopen, prior authorization, retry and receipt removal | pass for update; first-install configured recovery is a gap |
| Coordinator/recovery lifecycle | Retained helper survives GUI exit and completes Home/app startup | Retained exec handoff and receipt-directed recovery exist | Source only; tests call private completion helpers and stub GUI open | gap |
| Done when 1: clean-account DMG entry | Signed activation reaches setup without Terminal | Activation command and Swift integration absent | `InstallCommand`, app bootstrap and full diff; no fresh account/rendering | gap |
| Done when 2: accounts and chooser | Install/sign in to both tools and acquire gh without package manager; reach chooser | Guided setup, native status projection and chooser changes absent | Auth/CLI/UI sources unchanged; no sign-in attempted | gap |
| Done when 3: repeat open, shell, update, retry | No restart loop; fresh shell finds lf; retain destination; handle old/DMG bundles | Completion prerequisite repaired; PATH, destination and activation work absent | Process fixtures; both updaters still default to `/Applications`; no clean trial | gap |
| Done when 4: real conversation | Provider reply in existing project, GitHub connected, no Linear | No app conversation exercised | Real UI/account boundary unavailable | gap |
| Done when 5: behavioral and DTO coverage | Cover full installer/setup/navigation behavior | Seven lifecycle tests cover implemented subset; earlier staging tests remain evidence | Current output and design ledger; no new DTO/setup implementation | gap for full scope |
| Done when 6: affected-suite gate | Affected Rust/Python/Swift/UI/download suites and static checks | Focused review proof only; prior static and machine-install evidence retained | Current lifecycle test/diff check and design ledger | gap for gate |
| Done when 7: public path | Verify instructions against released artifacts | No public-channel trial; docs still begin with shell install and init | Docs/full diff; no publication | gap |

## Superseded finding and remaining boundary

P1 in [installer-slice-review.md](installer-slice-review.md) and finding 1 in
[concept-review.md](concept-review.md) describe the earlier implementation.
`commit_switch_selection` now writes `Settled` and `active_selection_committed`
to the pending receipt before reopening. Existing startup routing selects the
target from that receipt. `finish_switch` reopens before machine settlement
writes the active record, archives the receipt and removes the pending file.
Committed recovery resumes/verifies Home and finishes without quiescing again.
Cancellation also retains its receipt through failed reopen. The authorization
assertions exercise the previously identified first-install/prior-selection
hazard; this repair is more than moving `open` earlier.

P2, retained-helper execution and GUI-parent-exit survival, remains unproven.
The seven tests never invoke `promote`, `delegate_switch_recovery` or
`recover_switch`, start the Home keeper, or run Launch Services. They cannot
establish integration ordering, inherited descriptor behavior, registration,
or real command-entry recovery. A running sleep process proves duplicate-open
avoidance at the process boundary, not a usable window. First-install cancellation
still refuses ordinary helper startup; app activation retry must exercise the
existing install-command bypass rather than weakening startup authorization.

## Ownership and operational review

- Searched every production caller of `settle_switch`, `clear_switch`,
  `commit_switch_selection`, `finish_switch` and `active_install`. Successful
  promotion/recovery uses one reopen-before-archive helper. Remaining direct
  clears follow initial Home-pause failure, before app quiescence. The active
  record reader serves state reading and completion construction, with no new
  ordinary-startup route.
- Both promotion paths and recovery derive `ActiveInstall` in one place.
  Bootstrap adoption and startup's temporary projection are distinct uses,
  not competing completion decisions. Receipt schema, transition checks,
  binary store and machine registration remain authoritative.
- Searched Rust, Swift and release paths for app-ID shutdown and intent writers.
  No app-ID-wide quit remains. Selected-process observation is the sole
  production intent update. No Swift install receipt, account store, provider
  preference, repository list or onboarding-complete flag was added.
- The release README matches the implemented ordering. Customer instructions
  and UI were not rewritten from source. Signature validation still lives in
  the shell installer; Rust helper/tree checks do not replace it. Destination
  retention and PATH persistence remain necessary work.

## Proof and next action

On macOS, `cargo test -p loopflow --lib app_activation_tests`: **7 passed,
0 failed**, 1.65 seconds after 38.30 seconds compilation. Fixtures copy native
`/bin/sleep` into temporary bundles and use temporary machine roots. GUI open
is an explicitly failing test side effect. No host promotion, account connection,
keeper mutation or real GUI launch occurred. `git diff --check` passed.
Unchanged-source artifact, machine-install and static-check passes in the design
remain prior evidence; broader suites were not rerun for this review.

Next implement signed, repo-independent copied-bundle activation in the existing
installer, shared PATH persistence after isolating shell-test HOME/ZDOTDIR, and
recorded destination retention. Exercise the retained helper and real
promotion/recovery entry in a disposable Mac account: terminate its GUI parent;
interrupt before commit, after commit before reopen, and after reopen before
archival; include failed open, first installation/cancellation, update and
helper-only paths. Observe selected CLI/daemon/store, eventual receipt removal,
and no repeated restart. A HOME override does not isolate that machine boundary.

Obtain the running-app baseline before Swift UI changes. Provider/GitHub setup,
same-window opening, both clean-provider trials, docs, affected suites and public
verification remain required. No product decision blocks installer work. Broader
local planning ownership remains coordination with LOO-316; this review does not
claim local Tasks work.

Acceptance gaps remain, so this review does not publish, land, complete the Task
or choose Flow navigation. Production code is unchanged.
