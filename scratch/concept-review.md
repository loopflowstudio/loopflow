# Newcomer setup concept review — 2026-09-26

Reviewed LOO-312, Jack's accepted direction in the [design](a-newcomer-gets-from-download.md),
the [slice review](installer-slice-review.md), growth memory, and the complete
branch diff from `102522bc2` through `f8260ba70`. The worktree started clean.

Judgment: keep the accepted interaction and existing owners. The useful
simplification is one recoverable installation operation behind opening the
app, followed by setup derived from current tool/account observations. No new
product decision is needed. The installer still loses its active reopen
obligation before reopening succeeds; the retained-helper handoff still lacks
boundary proof. Neither the copied-app slice nor LOO-312 is complete.

## Proposed usage first

This is a proposed experience sketch within Jack's accepted scope, not a
walkthrough of an observed app. Do not publish it as instructions until the
running-app baseline and clean-account trials exist.

> Download Loopflow, drag it into Applications, and open it. Loopflow prepares
> the installed app and may reopen it to finish. If preparation fails, the app
> explains the failure and offers Retry.
>
> Choose a coding tool. Install it and sign in from Loopflow, then connect
> GitHub. Existing verified sign-ins skip those steps. Loopflow is free; the
> coding tool has its own cost.
>
> Choose **Open project…**. A project is a folder of files; for now it must
> already use Git. Canceling the chooser leaves that action available. Choosing
> another folder stays in the same window.

Concrete recovery: Kim opens the copied app; preparation closes it, then the
coordinator exits before reopening. On the next open, the same installation
owner must recover the unfinished work and reach setup, without another
download or an external Terminal command. Successful reopen must discharge
that obligation so later opens do not repeat activation. This is required
behavior, not a claim that recovery currently does it.

Canceling provider sign-in is different: returning to setup rechecks actual
status and offers Sign in again. A timeout offers retry; it must not appear as
signed out or connected. Installing a tool and authenticating it remain
separate actions. No repository, Session, Wave, Task, or Linear account owns
either operation.

Affected documentation: eventually replace the Mac opening of
`docs/getting-started.md`, retain the DMG link in `website/content.yaml`, and
describe only proven completion/recovery in `release/README.md`. Keep the
separate CLI route and its current-shell PATH instruction. Keep init's skill
unchanged: its repository setup is a separate use, and this Task does not
redesign it. No customer docs or skills are rewritten in this review.

## Core model in one screen

| Action or fact | Existing owner and types | Change under the accepted design |
| --- | --- | --- |
| Open the copied app | `ArtifactSet` owns retained bytes; `InstallSelection` pairs an installation with its store; `ActivationTargets` names installed destinations | Add the repo-independent signed-bundle entry to the installer; Swift presents its progress |
| Recover interrupted preparation | `SwitchReceipt` owns switch identity, prior/target selections, advancement, and `app_was_running`; `ActiveInstall` records the selected installation | Preserve the reopen obligation through completion; do not create a Swift installation record |
| Install and connect a coding tool | Installer owns acquisition; `provider_auth` owns native login; `ProviderAuthSnapshot` currently describes account status | Add bounded native availability/auth observations; persist an explicit verified choice in the existing Home `agent` setting |
| Connect GitHub | Existing auth owner and `gh` | Acquire `gh` without a package manager; verify sign-in before offering Open project |
| Open a project | `PortfolioService.addRepo`, `RepoScanner`, `PodiumModel.setRepoPath` | Use the same current-window selection path for setup and Go → Open |

Setup is the overall interaction, not a new durable entity. The installation
switch is one recoverable operation; account connection is an explicit action;
the displayed next step is derived from observations. These lifetimes differ
for useful reasons. Do not combine them into one persisted setup stage.

## Findings and challenged simplifications

### 1. “Settled” does not yet mean the opening operation finished

Observed source: published promotion settles before `resume_switch_app`
(`install.rs:3468–3483`). Machine settlement writes the active installation,
archives the receipt, and removes the active switch (`machine_install.rs:979–1027`).
Recovery returns for an already settled machine, finishes a partially settled
receipt without reopening, and clears an unadvanced switch before reopening
(`install.rs:3155–3210`). Exact published recovery also settles first
(`install.rs:3259–3264`). This preserves the slice review's P1 finding. The
failure after interruption is inferred from this ordering, not reproduced live.

Current → proposed experience: an installation can finish with the window
missing → opening/retrying Loopflow completes that same operation. Proposed
engineering direction: keep the outstanding reopen work in the existing
installation authority until it is fulfilled. Preserve selected bytes, store
advancement boundaries, and the original window intent. Helpers alone still
must not request a window. Do not infer pending work from historical
`app_was_running` alone: it remains true after a successful reopen, too.

The smallest source check already rejects a mechanical “move open earlier”
fix. Local promotion does reopen before settlement (`install.rs:2714–2716`),
but entry routing and authorization during a switch use
`startup_selection_during_switch` / `startup_active_during_switch`
(`machine_install.rs:762–800, 891–930, 1038–1080`). Before selection is committed,
ordinary startup selects the prior installation, or refuses first-install startup
because no prior selection exists. A switch-authorized process can instead use
the target. A completion fix must prove
the reopened app and its helpers use the appropriate installation/store;
copying one branch's ordering is not enough.

No new queue, coordinator service, or UI receipt is justified. The exact
receipt transition needed remains implementation work, not an approved schema
change. Keeping retained and destination paths distinct remains necessary for
replacement and recovery; combining them would discard required information.

### 2. One setup screen should expose observations, not remembered success

Observed source: `ProviderAuthService::resolve_snapshot` can report active
from a stored token before checking a native broker
(`provider_auth/mod.rs:949–986`). `ProviderAuthSnapshot` has no executable
availability field (`:255–261`). It cannot alone answer whether Loopflow can
launch the selected coding tool.

Current → proposed experience: separately acquired tools and account state →
the next useful action in the primary window. Keep the design's typed native
status projection inside the auth owner, with unavailable/unknown distinct
from signed out. Keep availability distinct from authentication. A selected
provider is UI intent until verified; only then update the existing Home
default. Repository overrides retain precedence. Connected does not promise
quota: the actual conversation remains the integration proof.

This avoids an onboarding-complete flag, a second credential store, a Swift
provider default, and a synthetic Session. These are avoided additions, not
claimed code deletions. The auth projection and default-selection behavior
are still unimplemented in this branch.

### 3. Open project should have one meaning in the primary window

Observed source: no selected repository renders `WorkspaceNavigator`
(`swift/LoopflowMac/Views/PodiumView.swift:41–49`). The app-level chooser
registers a folder and opens the `repo` scene
(`swift/LoopflowMac/LoopflowApp.swift:179–195`); selection inside the navigator
sets the current model's repository (`Views/WorkspaceNavigator.swift:410–413`).

The accepted same-window action removes a navigation distinction newcomers
should not need to learn. Reuse registration and selection rather than adding
a setup repository list. Preserve failed/loading discovery, returning sessions,
cancel, invalid-folder feedback, and the other windows' existing uses. No UI
claim here is live evidence, and no UI edit precedes the required baseline.

### 4. Acquisition paths can share ownership without becoming one action

Keep both DMG and CLI acquisition. Jack selected DMG for newcomers; the CLI
remains supported. Both should use the same validation, PATH writer, recorded
destination, and recovery owner. `install/published.rs:26–28` and
`release/install.sh:164` still choose `/Applications` absent an override. A
second copy after an update would break the apparent simplicity of drag/open.
The design's destination retention and explicit `LF_APPLICATIONS_DIR`
precedence remain required. No new updater configuration is proposed.

## Evidence, decisions, and next proof

Reused the slice review's four passing process/selection fixtures and the
design's four artifact fixtures. Source comparison against `fb6ce42dc` confirms
the reviewed installer and machine owner are unchanged. No behavioral tests
were rerun for this documentation-only review. Those passes establish neither
GUI reopen nor the retained-helper `exec` boundary; P2 remains open. This
review's source checks do not upgrade either claim to acceptance.

The smallest next implementation/proof is the completion boundary: interrupt
before settlement, after selection is committed but before reopening, and
after reopening but before completion is recorded. Retry through the real
installer entry and verify the correct app/helpers/store, eventual completion,
and no repeated restart on a later open. Include unadvanced rollback and
helper-only installation. Use owner-level isolated fixtures first; execute the
real retained helper and terminate its GUI parent in a disposable macOS account
for the boundary proof. A HOME override does not isolate machine state.

Any changed completion ordering requires renewed behavioral review of reopen,
startup authorization, and retry. Prior path-selection tests remain useful but
cannot stand in for that proof. Signed-bundle activation, PATH persistence,
update destination retention, provider/GitHub setup, live primary-window
baseline, clean-account trials, and public-channel verification remain in the
full design; this bounded recommendation does not remove them.

No human choice blocks this installer work. How to acquire `gh` is an
engineering choice under the accepted no-package-manager requirement. The
broader local planning/Linear boundary still needs coordination with LOO-316;
this review neither resolves its ownership nor claims local Tasks work.

Review only: no implementation, account, installation, planning, publication,
merge, Task completion, or Flow navigation changes.
