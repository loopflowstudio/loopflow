# v0.15.1

<!-- loopflow:release-notes=narrative;gate=safe -->

v0.15.1 changes how Loopflow reaches your Machine and how you get back to your work. Releases now soak for a week before new installs get them. Each Machine follows a stream you choose and updates itself daily, and on a Mac the app and the terminal run the same `lf`. Starting a Task is one step from a sentence to a running conversation, and `lf resume` gets you back to a conversation whatever is holding it. A stopped ship now stays stopped. Older versions keep working with this release's store: its two migrations only add columns.

## Choose how new your Loopflow is

Before this release, every published release became GitHub's Latest the day it shipped, so `install.sh`, `lf install`, the website and `Loopflow-latest.dmg` sent each one straight to new installs. A Machine updated only when someone ran `lf install`, and Desktop and the terminal's `lf` could be different builds. Jack Heart chose two pointers over one build line (LOO-669). A release now soaks as `preview`, and `stable` follows it a week later. You choose a stream and keep following it.

- **Three streams.** `dev` is the newest green `main` build. `preview` is the newest published release. `stable` is GitHub's Latest marker, which the daily release run moves to the newest release that has been published for at least 7 days. It only moves forward. (#1772)
- **`lf install --dev | --preview | --stable`** switches the Machine's stream and records it as `stream:` in `~/.lf/config.yaml`. Plain `lf install`, `lf schedule` and a rerun of `install.sh` follow the recorded stream, which is `stable` when none is set. `install.sh --channel stable|preview` picks one for a fresh install. (#1776)
- **Green `main` is installable.** Every commit that passes CI on `main` is packaged with its `reader-catalog.txt`, and `lf install --dev` installs the newest one. It needs `gh` authentication, and the packages expire after 14 days. This first shipped as `lf self install --main` (#1755) and is now `--dev`. (#1755, #1776)
- **Daily updates by default.** Every install schedules `lf install` once a day on its stream: launchd on macOS, crontab on Linux. A cadence someone already chose is kept. `lf schedule` with no frequency keeps the installed schedule, or uses daily if there is none. It used to default to weekly. (#1776)
- **One `lf` per Mac.** With Loopflow.app installed, `~/.local/bin/lf` links to the app's bundled `lf`. A running app that updates itself is asked to install the update, so `lf install` doesn't replace its bundle under it. With the app closed, `lf install` replaces the bundle and migrates the store. Loopflow.app updates through Sparkle from an EdDSA-signed `appcast-preview.xml` or `appcast-stable.xml`. (#1776)
- **Your store is never left unreadable.** Every release ships `reader-catalog.txt` next to `SHA256SUMS`. Before downloading, `lf install` checks that catalog against the store and skips any build that can't open it. The app's updater does the same. If you move to a slower stream after a newer `lf` has migrated the store, you keep the current `lf` until that stream catches up. No command deletes or downgrades the store. (#1772, #1776)
- **Regressions hold `stable`.** Mark a bug Task with the release it regressed in: `lf task create --regressed-in v0.15.3` or `lf task edit --regressed-in …`. While the Task is open, the daily run won't move `stable` to that release or any later one. Once the Task completes, the first release published after that becomes eligible when its week is up. Task status and Desktop show the mark. (#1776)
- **`lf release status`** shows `Preview:` and `Stable:`, the release `stable` moves to on the next run, and any regression holding it back. These replace the single `Latest tag:` line. (#1776)

## From a request to a running Task in one step

Starting work used to take three commands: create the Task, run it, then resume its conversation. Desktop had no way to start a Task at all.

- **`lf task create "fix the flaky retry test" --run`** files the Task with your words as its notes and their first line as its title. It starts the workflow the same way `lf task run` does and opens the conversation with your request as the first turn. `--graph NAME` picks another workflow, and the name is checked before anything is filed. `-b` runs headless to `end`. `--run --json` prints a receipt even when the run fails after filing, so nobody files the same request twice. (#1761)
- **Desktop New Task (⌘N)** asks "What do you want?" and lets you pick a Wave or the repository. It names the default workflow and where that default came from, shows the workflow's steps with the ones where you take part, and switches straight to the new conversation. (#1761)
- **`lf wave graph show [WAVE | --no-wave]`** reports which workflow a scope's new Tasks take up and which setting chose it. The JSON field `graph` is now `default: {name, source}`. (#1761)

## Get back to any conversation

Getting back to a conversation used to mean choosing between `resume`, `connect`, `--takeover`, `--try`, `--fresh` and `session attach`, and guessing what was holding it. Now `lf resume` works out what is holding the conversation and gets you back to it.

- **`lf resume`** reattaches a conversation no terminal is showing. If another `lf` terminal shows it, `lf resume` takes it over, and the old terminal says where it went. It stops a provider client that `lf` started, and it continues an ended or crashed conversation from its history. If the scope has no conversation, it starts one and says so. (#1767)
- **`--fork`** continues in a new Session forked from the conversation's log and leaves the other client running. **`lf session new`** restarts the scope's conversation on a fresh provider thread, seeded with what the last thread knew. (#1767)
- **Tasks are assigned or unassigned.** The `deferred` state is gone. `lf task unassign` clears the owner and the Linear assignee, and nothing moves an unassigned Task: not ship, not CI repair, not Wave operators. Desktop's Defer/Resume buttons are now Claim/Unassign. (#1767)
- **Desktop's Open conversation always does something.** It puts the Task's conversation in the pane, shows the spinner or shows the real error. Before, a pane whose saved layout named a headless Run could show "No Task conversation open" and ignore clicks. (#1769)
- **Sessions end when their build goes.** A Session owner whose `lf` executable is removed or replaced ends within seconds, once its turns have settled. It tells an attached viewer to run `lf resume <session>` and logs the path. Headless Runs are never ended this way. Leftover owners from rebuilt Dev apps and removed worktrees coincided with `syspolicyd` at 600–800% CPU. (#1765)

## Shipping you can stop

A stopped ship didn't stay stopped. The next run into `end` approved Auto-Ship again, including a Task Session relaunching itself, and could merge the PR.

- **`lf ship --stop` holds** until someone runs `lf ship`. While it holds, a run that reaches a merging step (`lf land`, `lf pr arm`, `pr land` or `pr-land`) waits there with `not approved to merge (<who> stopped shipping)`. Steps that merge any other way, such as a raw `gh pr merge`, don't check the stop. (#1775)
- **Repeated identical failures stop the loop.** When all three attempts of a ship's run fail with the same cause, Auto-Ship is withdrawn, and the Task's history records "Stopped shipping: Loopflow: <cause>". This is not a stop: a fixed run can approve shipping again. Before this, one refusal looped 38 times. (#1770, #1775)
- **Stacked Tasks land after their parent squash-merges.** A child rebased onto `main` now syncs from its parent's merge commit instead of refusing with `UnsafeSyncBase`. (#1770)
- **Who did what.** `lf task status` and Desktop show who approved Auto-Ship, who stopped it and who started the latest Graph. A comment an agent posts with `lf task comment` is attributed to its Session and the person who asked for it. (#1775)

## Operational notes

- **Store compatibility.** This release adds the `ship_stop` and `task_regressed_in` migrations. Both only add columns, so older `lf` keeps opening the store. Migrations now take their order from `origin/main`'s first-parent history when they merge, not when a release is cut. Once a migration is on `main` it can't change, so fix it forward. Release builds refuse to compile while a migration is unmerged. (#1755, #1775, #1776)
- **When new installs get this release.** Publishing no longer marks a release Latest. v0.15.1 reaches new installs and the website's download once it has been published for 7 days and no open regression Task covers it; `lf install --preview` gets it now. The website itself still deploys on every release, so its docs describe `preview` while its install command serves `stable`. (#1772, #1776)
- **Sparkle needs setup.** The first real app update needs `SPARKLE_ED_PRIVATE_KEY` in Doppler, matching `SUPublicEDKey` in the app's `Info.plist`. A Mac on `dev`, and an install with `LF_INSTALL_CLI_ONLY=1`, links the CLI alone, because dev packages don't include a signed app. (#1776)
- **Minors cut from `main`.** `lf release run minor` now takes the same path as a patch: it runs `prepare` hooks and ships whatever `main` holds. It publishes no closing patch first. A minor's notes still cover the cycle since the preceding `.0`. The `.lf/releases/minor-<target>.json` receipt and the snapshot check are gone, and stale receipt files are ignored. (#1763)
- **Cron host on macOS.** Rerun `scripts/bootstrap-cron-host.sh <wave>` from a logged-in Terminal. Release and telemetry now run as LaunchAgents in the login session, where they can reach the Keychain. The script removes the Wave's crontab lines before it loads the agents, so a bootstrap stuck at a permission prompt can't leave two schedulers for release-run. The agents run only while someone is logged in. Linux still uses crontab. (#1764, #1768)
- **Linear rate limit.** `lf` spaces out non-urgent Linear requests below 25% of an allowance, and below 10% it saves what is left for creating Tasks and linking PRs. Turn on `logs: {linear_requests: true}` in `~/.lf/config.yaml` to see where requests go with `lf history linear`. (#1766)
- **Update scripts.** These were removed: `lf connect`/`lf session connect`, `lf session attach`, `lf resume --takeover/--try/--fresh`, `lf task defer`, `lf task resume`, `lf task edit --unassign`, `lf install --main`/`lf self install --main` (use `lf install --dev`) and `lf clone`. `lf task unclaim` is now `lf task unassign`. Scripts that read `lf wave graph show --json` should read `default.name` in place of `graph`. Old spellings fail with a usage error. (#1755, #1761, #1767, #1776)

## Small changes

- `lf wt prune` prints `keep` or `remove` with a reason for every checkout. It removes checkouts whose Task finished, and those with no Task or PR whose head hasn't moved in 7 days. `--all` clears everything that isn't live work. Unpushed work is saved under `refs/lf/pruned/` first. `--json` works without `--builds`. (#1774)
- `lf doctor` has a `desktop-exit` check that says how each Desktop bundle's recent launches ended. Desktop now writes to `~/.lf/logs/lf.log`, including a vitals line every 5 minutes, and the `lf install` refresh agent logs there too. (#1771)
- Desktop's sidebar counts only open Tasks and moves a dragged row as soon as you drop it. Under Working Tasks, it lists shipping Tasks in an Autonomous section. The spurious "Layout not saved: CancellationError" banner is gone. (#1762)
- Linear's TeamChanges delta selects fewer fields, and a PR link already recorded as Open is no longer rewritten. (#1766)
- Ending a Session owner no longer launches `/usr/bin/kill`. (#1765)