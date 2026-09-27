# Newcomer setup concept review — 2026-09-26

Reviewed LOO-312, Jack's accepted [design](a-newcomer-gets-from-download.md),
growth memory, the full production diff from `102522bc2` through `c64bfbc62`,
and the [completion review](installer-completion-review.md). Preserved that
review's pending notes in checkpoint `1650f2d97` before editing. This document
supersedes the earlier concept review's outstanding receipt-ordering finding.

Judgment: keep the accepted interaction and current installation model. The
completion repair now preserves the distinction between selecting an installation
and finishing its recovery. The shared selection constructor removes duplicate
retention decisions. No further product concept or speculative lifecycle is
needed. Copied-app activation and the newcomer journey remain incomplete.

## Proposed usage first

This sketch expresses Jack's accepted scope, not an observed walkthrough. It
must not become public instructions before the running-app and clean-account
proofs exist.

> Download Loopflow, drag it into Applications, and open it. Loopflow prepares
> the installed app and may reopen it to finish. If preparation fails, the app
> explains what happened and offers Retry.
>
> Choose a coding tool, install it, and sign in from Loopflow. Then connect
> GitHub. Existing verified sign-ins skip those steps. Loopflow is free; the
> coding tool has its own cost.
>
> Choose **Open project…** and select an existing Git project. A project is a
> folder of files. Canceling leaves the action available; choosing a folder
> opens it in the same window. An invalid folder explains the problem and lets
> another folder be chosen.

Concrete recovery: Kim opens the copied app; preparation closes it, then the
coordinator exits before reopening. On the next open, the installer completes
that pending operation and the app reaches setup. If the app already reopened
before interruption, retry observes it without requesting another open. The
receipt now supports this behavior; the actual app entry and helper lifetime
still need implementation and proof. Kim should not have to distinguish
selection commit, store advancement, and receipt archival.

Canceling sign-in has a different recovery: recheck native account state and
show Sign in again if needed. A timeout stays unknown with Retry. Neither
installing a tool nor finding a saved token establishes a usable account.

Affected docs remain `docs/getting-started.md`, the Mac download instructions in
`website/content.yaml`, and installer behavior in `release/README.md`. The first
two wait for working behavior; the release README matches the implemented
ordering. Keep the separate CLI route and immediate-shell PATH guidance. The
init skill remains separate repository setup; no skill rewrite is warranted.

## Core model in one screen

| Action or fact | Owner and representation | Transition or API |
| --- | --- | --- |
| Prepare the copied app | Installer; `ArtifactSet` retains bytes, `ActivationTargets` names destinations | Signed repo-independent entry still needed; reuse `promote` and existing staging/preflight |
| Select installation and store | `SwitchReceipt.target`, `active_selection_committed` | `commit_switch_selection`; ordinary startup then resolves the target |
| Finish or recover preparation | Pending `SwitchReceipt`; `app_was_running` retains window intent | `recover_switch`, `finish_switch`; resume before archival/removal |
| Retain completed selection/history | `ActiveInstall` | `active_install_from_switch` shared by both promotion paths and recovery |
| Install/connect a coding tool and GitHub | Installer acquisition, native CLI/auth owner, existing Home default | Guided install and native-status projection remain to implement; verified choice writes existing config |
| Open a project | `PortfolioService`, `RepoScanner`, current `PodiumModel` | Register/resolve with existing owners, then `setRepoPath` in the current window |

Setup is a derived interaction, not a durable entity. Installation, account
connection, and repository selection have different lifetimes. Combining them
into one stored setup stage would require repairing that stage whenever a tool
is removed, credentials expire, or a project moves.

## Findings and challenged simplifications

### 1. Keep commit and completion separate; the earlier defect is repaired

Observed source: `install.rs:3144` commits `Settled` plus
`active_selection_committed` to the pending receipt. `finish_switch` at `:3157`
reopens before machine settlement. `machine_install.rs:896` gives the pending
receipt precedence over the last active record; startup selection at `:926`
uses the committed target. Machine settlement at `:990` writes the active
record and immutable archive before removing the pending receipt.

Normal published promotion (`install.rs:3474`) derives one active selection,
commits it, resumes Home, and finishes. Committed recovery (`:3212`) resumes
and verifies Home before the same finish helper; it does not quiesce again.
Failed reopen leaves the receipt pending. Unadvanced recovery (`:3223`)
restores the prior Home and reopens before cancellation clears its receipt.

Earlier behavior → current behavior: a failed reopen lost its actionable
obligation → the same receipt remains recoverable. Keep this repair. Moving
open before selection commit would restore the first-install/prior-store
startup hazard. Clearing the receipt at commit would restore lost recovery.

Vocabulary caution, not a new defect: `SwitchPhase::Settled` means selection is
committed inside a still-pending switch; `MachineInstallState::Settled` means
there is no pending receipt. Do not expose either internal label as setup
success. No persisted rename or extra completion flag is justified in this
review. The pending record already answers whether completion work remains.

### 2. Retained history has one constructor without flattening useful distinctions

Observed source: `active_install_from_switch` (`install.rs:3104`) preserves the
recorded active selection when it already matches the target; otherwise it
carries recorded retained releases and the receipt's fallbacks forward. Both
promotion paths and recovery call it. `active_install` has only the state-reader
and this constructor as production consumers.

Earlier model → current model: three retention decisions → one constructor.
Keep retained bytes separate from replaceable destination paths, and keep
startup's temporary projection separate from completed retained history.
Bootstrap adoption precedes a switch and remains a distinct operation.
Deleting these distinctions would lose either safe replacement, recovery, or
first-install handling. No additional interface reduction is required.

### 3. The remaining installer gap is the executable boundary

P2 from the slice review remains open. `promote` (`install.rs:3529`) delegates
an existing receipt to recovery; for a new published promotion, `:3538` stages
and execs a helper outside the selected bundle. Recovery delegation (`:2920`)
runs the receipt-pinned owner. The seven fixtures do not execute these entry
paths, Launch Services, or the Home keeper. Their sleep processes cannot prove
a usable window or inherited-descriptor/GUI-parent-exit behavior.

First-install cancellation remains a required counterexample: no prior
installation exists to authorize ordinary startup. `bin/lf.rs:1151` bypasses
that authorization for install commands, while entry-gate dispatch happens
earlier at `:1127`. The copied app must invoke its bundled installation helper
directly and prove recovery through this ordering. Do not weaken ordinary
startup authorization to make cancellation appear successful.

`InstallCommand` (`lf/mod.rs:1115`) still has no copied-bundle activation entry.
Signature validation remains in `release/install.sh:185`; Rust tree/helper
identity checks alone do not establish signed activation. `install/published.rs:26`
and `release/install.sh:164` still default to `/Applications`. Shared PATH
persistence and update destination retention remain unimplemented. These are
missing accepted capabilities, not reasons to introduce a Swift install record
or a second updater.

### 4. Keep the setup projection and same-window action already designed

Source confirms the earlier mismatches remain: `provider_auth/mod.rs:949`
can report active from a stored token without checking native executable
availability. `PodiumView.swift:41` renders `WorkspaceNavigator` with no selected
repository. `LoopflowApp.swift:179` registers a chosen repository but opens the
older repo scene; the navigator selects the current model at
`WorkspaceNavigator.swift:412`.

Current → proposed experience: separate installation/account prerequisites and
two project-opening paths → one observed next action and one current-window
selection path. Preserve loading/error distinction, returning sessions, canceled
choosers, invalid-folder feedback, existing OpenCode use, repository config
precedence, and native authentication failures. The typed native-status view
must stay with the auth owner; explicit verified provider choice uses the
existing Home default. Installation alone never changes that default.

No account store, setup-complete boolean, synthetic Session, alternate repository
list, or provider preference in Swift earns a place. GitHub stays before
**Open project…**; acquiring `gh` without a package manager remains engineering
work. This review does not change Jack's optional-Linear direction or claim
local Tasks already support it.

## Evidence, decisions, and next proof

Reused the completion review's **7 passing lifecycle tests** (1.65 seconds after
compilation) on unchanged production source. Inspected their bodies and all
completion/clear/selection callers. The fixtures cover selected processes,
startup authorization, failed reopen, retry, retained history after partial
archival, helper-only completion, and unadvanced update cancellation. They do
not invoke real promotion/recovery. No behavioral tests were rerun for this
documentation-only review; `git diff --check` validates the edited Markdown.

No new bounded production defect or product decision was identified. The old
P1 finding is superseded; P2 and the completion review's acceptance gaps carry
forward. This review changes no implementation and invalidates no prior proof.

The next implementation is the signed, repo-independent copied-bundle entry
through the existing installer. Alongside it, isolate every shell-test HOME
and ZDOTDIR before adding shared PATH persistence, and retain the copied app's
actual destination for updates. No live baseline is needed for those Rust and
shell changes; the baseline remains required before Swift UI edits.

The smallest missing lifecycle proof executes the retained helper and actual
recovery entry in a disposable Mac account, terminating the GUI parent and
interrupting before commit, after commit before reopen, and after reopen before
archival. Include first-install cancellation, failed open, updates, and
helper-only installation. Observe correct CLI/daemon/store selection, eventual
receipt removal, and no repeated restart. A temporary HOME does not isolate
this machine boundary. Rendering and a fresh account are unavailable here;
no host installation or login was attempted to substitute for that proof.

Provider/GitHub setup, same-window opening, the real provider response, both
clean-provider trials, narrow docs, affected-suite gate, and public-channel
verification remain required by the complete design. Broader local planning
ownership remains coordination with LOO-316. This review chooses no Flow
navigation and does not publish, merge, or complete the Task.
