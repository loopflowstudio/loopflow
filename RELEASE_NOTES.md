# v0.12.22

v0.12.22 keeps a Task's plan, conversation, review feedback, and running work together. The Mac app brings them into one workspace, while saved Flows carry work through revision and blocked PR landings can recover on the same commit. Getting started also requires less configuration: unconfigured runs choose an installed coding agent, and display names can come from Git.

## Follow a Task without losing your place

The Mac app now uses one repository → Wave → Task → Session outline. Selecting a Task exposes its Flow, comments, and running work; entering a Session retains the surrounding workspace.

- Choose Compact, Full hierarchy, or Sessions presentations. Started Tasks appear before they have a Session, unattributed conversations remain under **Orphan sessions**, and navigation state survives repository switches.
- Preview a Task's Flow before **Start**. Once started, inspect its captured definition, current step, and loop iteration, then **Resume** or use the confirmed **Stop & restart…** action.
- Read recent Runs and Linear comments from the Task overview. Failed comment reads retain the earlier thread with **May be out of date** and **Retry**.
- Open **Monitor** beside Sessions and shells to follow active Task Runs. Incomplete observations remain visible rather than being reported as confirmed emptiness.
- Switching to a Session in another checkout restores its conversation, companion terminals, split layout, and focus. **New conversation** opens a scoped prompt without creating a Task or starting its Flow.
- Rename Sessions in place or with `lf session rename <id> "Name"`. Agent suggestions preserve names chosen by a person.

## Continue from review to the next useful step

`lf task advance <issue>` drives a Task's saved Flow until input, a blocker, interruption, or completion. Recovery retains captured definitions, selected branches, and completed review feedback, so revision does not require reconstructing the work.

- The `feature` Flow connects design review to implementation, compression, behavioral and concept reviews, explicit loop decisions, and an interactive demo. Demo feedback can lead to another implementation pass.
- **Complete** replaces Approve/Iterate in CLI and desktop Sessions. Completing the Session returns feedback; the following decision step chooses where the Flow goes next.
- Blocked decisions reuse one Ask and reassess its answer. Decisions take effect only after their originating Run succeeds, and stale workers cannot settle newer work.
- Ordinary Flows are resumable too. Finishing a Flow does not implicitly merge its PR or complete its Task.
- Launching inside a Task's worktree without an explicit Work selector now associates the launch with that Task. `lf --task ID` accepts direct Flows and inline prompts as well as skills.

## Recover delivery without an empty commit

After resolving an external blocker, rerun `lf pr land` on the same head. Landing reconciles fresh GitHub evidence after repairs and reports the repair agent's concrete conclusion.

- Blocked landings can resume without a lifetime repair ceiling or a required SHA change. GitHub's confirmed merge takes precedence over provider errors.
- Repair publication preserves active checkout placement and the intended Task disposition, including `-c` or `--next <slug>`.
- Rebase recreates deleted remote branches despite stale tracking refs, while leases continue protecting unseen remote work.
- Preparing a replacement PR head explicitly removes the PR from GitHub's merge queue before publication.

## See what happened during Wave operation

Each Wave wake runs one `wave/operate` attempt, with failures, input claims, and provider sessions recorded in the turn journal. This simplifies Wave operation while Tasks and ordinary Flows retain their execution engines.

- Wave stack traversal, queue/skip controls, and playhead state are removed. A later unrelated successful wake cannot erase an earlier recorded failure.
- `lf wave recover <name>` displays the original historical continuation. Canceling it requires its exact source sequence and a reason; unknown provider liveness blocks replacement.
- `lf runs --active [--task ID] [--json]` reports current provider-backed Runs and observation gaps. macOS also supports streaming snapshots with `--watch --json`; Linux supports one-shot reads.
- Desktop Task terminals use Ghostty with a pool owned by each workspace. The tmux plugin and desktop tmux shells are removed; timer rendering handles unavailable display links.

## Start with the agent and name already configured

The first-run guidance now explains the plan–build–review workflow and leads with the Mac app. Agent selection and naming use existing local configuration, while request authors remain attached to their original requests.

- Unconfigured runs select the first installed agent in order: Codex, Claude Code, OpenCode. Explicit configuration and skill defaults retain precedence; missing-agent errors include installation guidance.
- The Mac app explains how to recover when opening a non-Git folder. Documentation adds a glossary, prerequisites, and provider-cost guidance; Task delivery still requires Git, Linear, and GitHub setup.
- Inspect the resolved display name with `lf user name --json`, replacing `lf name`. A personal Loopflow override takes precedence over Git's `user.name`, including repository overrides.
- Reopening or moving a Session resumes its conversation without injecting a participant-update prompt. A missing Loopflow name preference no longer tells the agent to disregard a familiar name.
- Request authors survive CLI and Mac chat, journal replay, Discord, and Linear without being reassigned to publishers or editors. Unknown authors remain unknown, and display names do not establish authorization.

## Operational notes

- Existing Flow invocations retain their captured definitions. Start a fresh invocation to replace obsolete review navigation policies. Inspect interrupted external operations before retrying them.
- Wave listeners and residents must use matching binaries. Historical queues are not automatically transferred into ordinary Flows. Unclosed attempts still require authoritative termination reconciliation; this release does not add that command.
- Minor releases share a product snapshot with their closing patch: publish a new patch when changes remain, otherwise reuse the latest completed patch. Patch notes cover the preceding release; minor notes cover the cycle since the preceding `.0` tag. Persisted pair selection supports retries, but recovery receipts remain local to the controller.
- `lf release notes VERSION --preview` generates notes without changing release archives or manifests. Existing versions use their historical tag and context. Minor publication stops before tagging if the merged candidate differs from the prepared snapshot.
- The Mac workspace currently supports light mode only. The `design` and `ship-5whys` single-step Flow wrappers are removed; invoke them as skills. Bare names prefer skills, with `lf skill <name>` and `lf flow <name>` available for explicit selection.
- Recorded recovery and publication checks include simulated provider/GitHub boundaries; live branch recreation and configured release-note previews were observed. Live provider continuation, hosted recovery, and hosted UI acceptance remain unverified in the supplied evidence. The recorded default local gate had a Flow Session fixture conflict between `LF_HOME` and `LF_CONTROL_HOME`; its broad Rust pass used an isolated Home without that pin.

## Small changes

- Specialized operating instructions live in the skills that use them, reducing universal prompt guidance. Durable lessons live in Wave memory; retired directions and orphaned attribution notes are removed, while chapter archives remain in `.lf/chapters/`.
- `lf ls --current` excludes abandoned and retired registrations. `lf work forget wave <id>` removes an empty abandoned registration and supports `--dry-run`.
- Development Session reopen commands retain the development executable. Stored Codex OAuth no longer overrides native login.
- Terminal test polling reads fresh state before evaluating its deadline.
- Seven Rust dependencies are updated, and the repository includes the MIT license.
