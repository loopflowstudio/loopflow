# Newcomer setup concept review — standing guidance

Condensed 2026-10-22 from the 2026-09-26 review (full text in git at
`8278b4a210`, `scratch/concept-review.md`). Its installer gap (copied-bundle
entry, signed activation, PATH persistence, destination retention) is now
implemented; see the design's "This slice".

Judgment: keep the accepted interaction and installation model. Setup is a
derived interaction, not a durable entity — installation, account connection
and repository selection have different lifetimes, so no stored setup stage.

Hold these distinctions when touching the installer:

- **Commit vs completion.** A pending `SwitchReceipt` with
  `active_selection_committed` keeps startup on the committed target while
  reopen/archival can still fail and retry. Opening before commit restores the
  first-install startup hazard; clearing the receipt at commit loses recovery.
  `SwitchPhase::Settled` (committed, still pending) and
  `MachineInstallState::Settled` (no receipt) are internal; never show either
  as setup success.
- **One retention constructor.** `active_install_from_switch` serves both
  promotion paths and recovery. Keep retained bytes separate from replaceable
  destination paths; bootstrap adoption stays a separate operation.
- **First-install cancellation** has no prior installation to authorize
  startup. The copied app invokes its bundled helper directly; do not weaken
  ordinary startup authorization to make cancellation look successful.

Setup view (not yet built): one observed next action from the auth owner's
native reading, and one current-window project path (`setRepoPath` on the
current `PodiumModel`, not the older repo scene). No account store,
setup-complete flag, synthetic Session, alternate repository list or Swift
provider preference. Installing a tool never changes the default agent; a
saved token alone never means a usable account. Cancelled sign-in rechecks and
offers Sign in again; a timeout stays unknown with Retry. Once the view works,
its actions replace the CLI setup block in `docs/getting-started.md`.

Smallest missing lifecycle proof: in a disposable Mac account, run the retained
helper and real recovery entry, terminating the GUI parent and interrupting
before commit, after commit before reopen, and after reopen before archival;
include first-install cancellation, failed open, updates and helper-only
installation. Observe CLI/daemon/store selection, eventual receipt removal and
no repeated restart. A temporary HOME does not isolate this boundary.
