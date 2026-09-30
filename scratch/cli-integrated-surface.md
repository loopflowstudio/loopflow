# Integrated CLI surface · 2026-09-30

Compiled at the Monitor/launch cut after integrating LOO-298 `25548d567`.
127 commands below root, 424 flags, 84 positionals, 16 hidden commands, zero registered aliases.

This is the exact implemented inventory, including model additions. Baseline verdicts and remaining disagreements stay in [the catalog](cli-command-catalog.md); presence here is not completion of a proposed deletion. Each argument is owned by its command, with its distinct input below. Metadata comes from `cli_catalog`; no manually copied command tree drives runtime.

| Command | Concept / operation | Hidden |
|---|---|---|
| lf | Open Loopflow or run its CLI |  |
| lf monitor | Show waiting, blocked, active, and finished work with next actions |  |
| lf monitor list | List a bounded page of recorded commands, newest first |  |
| lf monitor show | Inspect an Exec or Session by identity |  |
| lf monitor active | Observe active conversations and missing process evidence |  |
| lf monitor usage | Show direct provider-authored usage from recorded Session inputs |  |
| lf monitor ps | Print one parseable snapshot of live Loopflow call trees |  |
| lf monitor top | Refresh live Loopflow call trees on a terminal; print once when redirected |  |
| lf monitor prune | Reap registered orphan providers and remove dead process receipts |  |
| lf monitor activity | Show one ordered record of durable Work, Session, PR, and Steer facts |  |
| lf : | Run an inline prompt |  |
| lf __screenshot-supervisor | Internal owner-loss supervisor for one browser capture | True |
| lf __provider-session | Internal provider callback that records one native interactive session | True |
| lf session | Inspect and continue Sessions |  |
| lf session ask | Open a durable session and wait for the user to complete it |  |
| lf session history | Read this conversation's native start, usage and completion receipts |  |
| lf session list | List Sessions |  |
| lf session connect | Connect to the live conversation, or resume its saved history |  |
| lf session complete | Complete a review, blocked Ask, or interactive session |  |
| lf session rename | Rename a Session; a human name is never replaced by a suggestion |  |
| lf session bind | Assign a Task to a Session that has none; the Task never changes after |  |
| lf session ready | Mark the active session ready for your review |  |
| lf session serve-flow | Run the exact review skill in its durable terminal | True |
| lf session serve-ask | Run one ad-hoc request in its durable terminal | True |
| lf session stop-client | Stop one exact native provider Run after its review completes | True |
| lf account | Refresh account access and capacity, or manage logins and routing |  |
| lf account disconnect | Disconnect local credentials or one managed login |  |
| lf account connect | Connect local credentials or a managed login using a remembered browser |  |
| lf account set | Edit account configuration or remembered browser choices |  |
| lf account route | Explain configured and automatic account selection, or replace a route |  |
| lf account route set | Replace a provider's ordered route |  |
| lf repo | Repository releases, source measurement, CI evidence, and provider administration |  |
| lf repo new-chapter | Advance every Wave to the named Project plan |  |
| lf repo release | Release operations (run, check, notes, bump, tag, status) |  |
| lf repo release run | Run the full release workflow end-to-end |  |
| lf repo release check | Check if PRs have merged since the last tag |  |
| lf repo release notes | Generate release notes for a version |  |
| lf repo release bump | Bump version in manifest files |  |
| lf repo release tag | Create a git tag and push it |  |
| lf repo release publish | Stage or publish a GitHub Release |  |
| lf repo release status | Check release workflow status |  |
| lf repo tokens | Measure this codebase: lines and tokens per directory (tracked files only) |  |
| lf repo ci | Show how failed CI is detected, repaired, and landed across this Home |  |
| lf repo reteam | Reconcile linked Waves to the repository's Linear Team |  |
| lf home | Inspect this Home and observe routes to other Homes |  |
| lf home desktop | Open or focus Loopflow.app |  |
| lf home screenshot | Capture a URL or local HTML file without claiming the user's browser |  |
| lf home install | Install the latest published Loopflow release from any directory |  |
| lf home install schedule | Install the latest Loopflow at login and weekly by default (macOS launchd) |  |
| lf home install recover-switch | Continue one interrupted machine install switch from its pinned candidate | True |
| lf home install preflight | Preview whether this build may replace the global lf (read-only). Reads the shared store's migration frontier and validates executable planning references against this binary without changing that frontier. Exits non-zero on refusal so a caller can gate on it | True |
| lf home install local-preflight | Validate this exact local candidate against one receipt-selected store | True |
| lf home install advance-switch | Advance the receipt-selected store with this exact candidate's registry | True |
| lf home install promote | Promote this build to the global CLI: content-address it into ~/.lf/bin and atomically repoint the target symlink, under the exclusive promotion lock. Refuses — leaving every target unchanged — on incompatible schema or persisted executable evidence | True |
| lf home install rollback | Repoint the global CLI at retained prior bytes only after that binary's own preflight proves it recognizes the current store frontier | True |
| lf home sync-skills | Compile loopflow skills into your home vendor Skills directories | True |
| lf home doctor | Audit the local run ledger: continuity, vocabulary, attribution, identity, lineage, coverage |  |
| lf home ssh | Run lf on a Home or SSH host carrying your local credentials |  |
| lf home user | Print the configured participant display name |  |
| lf home id | Print this machine's stable local Home identity |  |
| lf home observe | Record the current route for a known Home identity |  |
| lf discord | Bridge new Discord messages to finite Wave Runs |  |
| lf discord serve | Poll a configured channel and post each Run's final answer |  |
| lf wave | Manage Wave identity, placement and planning |  |
| lf wave cron | Local launchd jobs that run lf commands on a schedule |  |
| lf wave cron add | Install or replace a scheduled lf invocation |  |
| lf wave cron list | List installed loopflow cron jobs |  |
| lf wave cron preflight | Validate Home authority and declared jobs without changing launchd |  |
| lf wave cron sync | Reconcile installed launchd jobs to match a wave's declared `crons:` |  |
| lf wave cron run | Execute one installed cron job and persist its terminal receipt | True |
| lf wave cron history | Show durable cron receipts |  |
| lf wave cron trigger | Ask launchd to fire an installed job |  |
| lf wave cron remove | Uninstall a scheduled lf invocation |  |
| lf wave list | List every wave in the registry (running and stopped), marking which have a live server. Local-only query over the shared ledger |  |
| lf wave status | Show one Wave's chapter, Tasks, Runs, and live loop state from the registry. Defaults to the ambient wave (`LF_WAVE_ID`) |  |
| lf wave connect | Connect a Wave to its Initiative and the repository's Team (Task prefix) |  |
| lf wave sync | Refresh shared planning from Linear |  |
| lf wave rename | Rename the provider Initiative |  |
| lf wave forget | Forget an empty Wave registration, preserving authored files |  |
| lf wave place | Place a Wave on a Home |  |
| lf wave relocate | Rename or rehome a stopped Wave |  |
| lf wave retire | Retire the Wave, retaining history |  |
| lf wave update-plan | Replace the current chapter's KRs, targets, and Flow recommendation |  |
| lf task | Concrete work, worktrees, commits, and pull requests |  |
| lf task pr | Pull request lifecycle |  |
| lf task pr checks | Show CI status for current branch |  |
| lf task pr next | After an out-of-band merge, rotate this Task to its next serial PR, carrying committed and uncommitted follow-up onto the new branch |  |
| lf task pr publish | Publish a ready PR headlessly: push, create or refresh, print state + URL. Opens no review surface |  |
| lf task pr open | Push and create or update a draft PR, then open its GitHub page. Existing ready PRs stay ready; opening a draft does not publish it |  |
| lf task pr submit | Prepare a PR to land: rebase, clear scratch, mark ready, and assign it to you. Nothing merges until you click merge on GitHub |  |
| lf task pr arm | Prepare a PR, request exact-head auto-merge, and return without watching |  |
| lf task pr land | Arm and watch a PR through CI repair and authoritative merge |  |
| lf task pr abandon | Abandon branch: close PR, remove worktree, delete branch |  |
| lf task wt | Worktree operations |  |
| lf task wt create | Create a low-level sibling worktree |  |
| lf task wt switch | Switch to a worktree by name, identity leaf, or full branch |  |
| lf task wt list | List worktrees (read-only; reflects the last-synced main) |  |
| lf task wt prune | Remove clean terminal or inactive worktrees |  |
| lf task wt remove | Remove a worktree |  |
| lf task rebase | Rebase current branch onto target (default: main) |  |
| lf task commit | Commit changes |  |
| lf task __worker | Internal: drive a Task Flow from its claimed boundary | True |
| lf task checkout | Ensure tracked Task Work and its worktree without starting a worker |  |
| lf task run | Start or continue a Task through its saved Flow |  |
| lf task create | File a Task in the current chapter; optionally prepare and run it |  |
| lf task status | Show durable Task facts and current worker evidence |  |
| lf task diff | Show this Task's patch or list its changed files |  |
| lf task file | Read one file from this Task's worktree |  |
| lf task save | Save UTF-8 stdin with an expected revision and retained recovery files |  |
| lf task complete | Complete planning work, or a placed Task whose pull requests are settled |  |
| lf task delete | Delete a Task from Linear and reconcile its local record |  |
| lf task edit | Edit a Task's title or notes, before or after placement |  |
| lf task comment | Read the comment thread, or append direction without starting execution |  |
| lf task interrupt | Interrupt the active provider turn |  |
| lf task wait | Wait without polling an LM |  |
| lf task restart | Stop the pinned Flow and begin a new one in a fresh Task worker; defaults to the chapter's currently recommended Flow |  |
| lf __telemetry-scorecard | Internal: render the repository maintainer scorecard for telemetry-daily | True |
| lf list | Discover commands, skills, and flows |  |
| lf help | Explain a command, skill, or flow without launching it |  |
| lf roadmap | Show the current repository's roadmap: every open Task across the repo's Waves, joined to live evidence and bucketed into Now / Waiting / Available / Later. `--wave` scopes it; `--all` spans every repository on this machine. Local-only, deterministic |  |
| lf replay | Launch the immutable provider request retained for a captured input |  |
| lf __flow-step | Execute one captured Flow boundary in its own process | True |
| lf run | Run a definition, preferring a flow over a same-named skill |  |
| lf flow | Run or inspect authored flows |  |
| lf flow list | List authored flows or saved FlowSessions |  |
| lf flow show | Inspect an authored flow or a saved FlowSession |  |
| lf flow resume | Continue a saved Flow invocation |  |
| lf skill | Run a skill explicitly |  |

## Inputs

| Command | Input | Distinct input / purpose |
|---|---|---|
| lf | --docs | Docs paths, globs, or directories to include in context |
| lf | --clipboard / -c | Include clipboard content in prompt |
| lf | --model / -m | Model to use (harness or harness:model) |
| lf | --account | Prefer this managed provider login before the normal route. Repeat to select provider-qualified preferences such as `claude=jack@`. Logins spend; a profile is only the Chrome venue accounts log in through, so it is never a run-time selector |
| lf | --only-account | Restrict this invocation and its children to exactly these managed provider logins. Providers without a selection are unavailable |
| lf | --__account-lease-probe | Internal SSH compatibility and broker-connectivity probe |
| lf | --yolo | Skip permission prompts |
| lf | --mode | Choose the provider surface; omission inherits configuration and terminal context |
| lf | --chrome | Override Chrome integration; omission inherits configuration |
| lf | --diff | Select changed-code context; omission inherits configuration |
| lf | --max-turns | Maximum agent turns for this invocation |
| lf | --as | Select one Work for a direct skill, flow or inline prompt |
| lf | --__cwd | Keep a Work-bound internal launch in this exact checkout |
| lf | --no-loopflow | Exclude loopflow operating guidance |
| lf | --__flow-step | Execute a skill from this saved Flow boundary, without resolving its definition again |
| lf | --help / -h | Print help |
| lf | --version / -V | Print version |
| lf monitor | --json | json |
| lf monitor | --all | all |
| lf monitor | --help / -h | Print help |
| lf monitor list | --json | json |
| lf monitor list | --all | Include all repositories |
| lf monitor list | --limit | limit |
| lf monitor list | --after | Continue with the previous page's next object, encoded as JSON |
| lf monitor list | --parent | Direct children of an exact or unambiguous parent Exec |
| lf monitor list | --caller | Commands issued by this AgentSession |
| lf monitor list | --search | Literal command text, ignoring ASCII case |
| lf monitor list | --outcome | outcome |
| lf monitor list | --task | Recorded work for a Task, including completed Tasks |
| lf monitor list | --wave | Recorded work for a Wave |
| lf monitor list | --help / -h | Print help |
| lf monitor show | <id> | id |
| lf monitor show | --json | json |
| lf monitor show | --events | Print a Session's retained raw input events |
| lf monitor show | --final | Print a Session's last recorded provider conclusion |
| lf monitor show | --input | Inspect an exact retained input belonging to this Session |
| lf monitor show | --help / -h | Print help |
| lf monitor active | --json | json |
| lf monitor active | --watch | Stream NDJSON until stdin closes |
| lf monitor active | --task | task |
| lf monitor active | --help / -h | Print help |
| lf monitor usage | --json | Emit Session usage evidence as JSON |
| lf monitor usage | --days | Observation window, in days (zero means all time) |
| lf monitor usage | --parent | Inputs issued by this Session or retained capture |
| lf monitor usage | --wave | Limit to Session inputs attributed to one Wave |
| lf monitor usage | --project | Limit to Session inputs attributed to one Project |
| lf monitor usage | --task | Limit to Session inputs attributed to one Task |
| lf monitor usage | --help / -h | Print help |
| lf monitor ps | --json | Emit the versioned activity snapshot as JSON |
| lf monitor ps | --help / -h | Print help |
| lf monitor top | --json | Emit one versioned activity snapshot as JSON |
| lf monitor top | --help / -h | Print help |
| lf monitor prune | --dry-run | Show exact targets without changing process or receipt state |
| lf monitor prune | --json | Emit the versioned prune report as JSON |
| lf monitor prune | --help / -h | Print help |
| lf monitor activity | --since | Relative window (7d, 24h, 30m) or RFC3339 start |
| lf monitor activity | --limit | Maximum rows after Work filters (1-200) |
| lf monitor activity | --wave | Scope to one Wave by name |
| lf monitor activity | --project | Scope to one Project by slug |
| lf monitor activity | --task | Scope to one Task by Linear identifier |
| lf monitor activity | --json | Emit the typed activity snapshot as JSON |
| lf monitor activity | --help / -h | Print help |
| lf : | <prompt> | prompt |
| lf : | --help / -h | Print help |
| lf __screenshot-supervisor | <source> | URL or local HTML file to capture |
| lf __screenshot-supervisor | --output / -o | PNG destination |
| lf __screenshot-supervisor | --width | Viewport width in pixels |
| lf __screenshot-supervisor | --height | Viewport height in pixels |
| lf __screenshot-supervisor | --help / -h | Print help |
| lf __provider-session | --help / -h | Print help |
| lf session | --help / -h | Print help |
| lf session ask | --skill | Named skill for the session |
| lf session ask | <question> | What the session should work through |
| lf session ask | --help / -h | Print help |
| lf session history | <id> | id |
| lf session history | --json | json |
| lf session history | --after | Continue after an observed event sequence |
| lf session history | --limit | limit |
| lf session history | --help / -h | Print help |
| lf session list | --json | json |
| lf session list | --all | Include waiting steps from every repository on this machine |
| lf session list | --interactive | Select interactive (true), headless (false), or both (all) |
| lf session list | --history | Include completed conversations and historical reviews |
| lf session list | --limit | Maximum conversations; 0 reads the complete matching inventory |
| lf session list | --offset | offset |
| lf session list | --page | Return a bounded stable-ID page with a continuation cursor |
| lf session list | --after | Previous page's next identity; keep the same filters |
| lf session list | --task | task |
| lf session list | --search | search |
| lf session list | --help / -h | Print help |
| lf session connect | <id> | id |
| lf session connect | --json | json |
| lf session connect | --replace | Stop Loopflow-owned clients before resuming here |
| lf session connect | --try | Ask the provider to resume even when another client is active |
| lf session connect | --help / -h | Print help |
| lf session complete | <id> | id |
| lf session complete | --help / -h | Print help |
| lf session rename | <id> | id |
| lf session rename | <name> | name |
| lf session rename | --suggest | Propose an agent-generated name; keeps a human-assigned name |
| lf session rename | --json | json |
| lf session rename | --help / -h | Print help |
| lf session bind | <id> | id |
| lf session bind | --task | The Task, by its issue identifier (e.g. INF-123) or stable Task ID |
| lf session bind | --dry-run | Resolve the exact target without assigning the Session |
| lf session bind | --json | json |
| lf session bind | --help / -h | Print help |
| lf session ready | <summary> | summary |
| lf session ready | --help / -h | Print help |
| lf session serve-flow | <task_id> | task_id |
| lf session serve-flow | <invocation_id> | invocation_id |
| lf session serve-flow | <flow> | flow |
| lf session serve-flow | <node_id> | node_id |
| lf session serve-flow | <skill> | skill |
| lf session serve-flow | <iteration> | iteration |
| lf session serve-flow | --help / -h | Print help |
| lf session serve-ask | <run_id> | run_id |
| lf session serve-ask | --help / -h | Print help |
| lf session stop-client | <input> | input |
| lf session stop-client | --help / -h | Print help |
| lf account | <provider> | Limit observations to one provider |
| lf account | --cached | Inspect cached evidence without contacting providers or the origin broker |
| lf account | --details | Include credential sources, browser choices, and timestamps |
| lf account | --json | Emit the account overview as one JSON document |
| lf account | --help / -h | Print help |
| lf account disconnect | <provider> | provider |
| lf account disconnect | <email> | email |
| lf account disconnect | --help / -h | Print help |
| lf account connect | <provider> | provider |
| lf account connect | <email> | email |
| lf account connect | --chrome-profile | chrome_profile |
| lf account connect | --import | Adopt an existing Claude login |
| lf account connect | --api-key | Read the provider's API key environment variable |
| lf account connect | --help / -h | Print help |
| lf account set | <provider> | provider |
| lf account set | <email> | email |
| lf account set | --login-email | login_email |
| lf account set | --routing | routing |
| lf account set | --plan | plan |
| lf account set | --clear-plan | clear_plan |
| lf account set | --paid-through | paid_through |
| lf account set | --clear-paid-through | clear_paid_through |
| lf account set | --clear-cooldown | clear_cooldown |
| lf account set | --chrome-profile | Replace the ordered browser choices (repeat for fallback profiles) |
| lf account set | --clear-chrome-profiles | clear_chrome_profiles |
| lf account set | --help / -h | Print help |
| lf account route | --repo | repo |
| lf account route | --default | default |
| lf account route | --json | json |
| lf account route | --help / -h | Print help |
| lf account route set | <provider> | provider |
| lf account route set | <accounts> | accounts |
| lf account route set | --repo | repo |
| lf account route set | --default | default |
| lf account route set | --help / -h | Print help |
| lf repo | --help / -h | Print help |
| lf repo new-chapter | <name> | name |
| lf repo new-chapter | --dry-run | dry_run |
| lf repo new-chapter | --json | json |
| lf repo new-chapter | --help / -h | Print help |
| lf repo release | --help / -h | Print help |
| lf repo release run | <version> | Version to release: patch\|minor\|major\|X.Y.Z (default: patch) |
| lf repo release run | --target / -t | target |
| lf repo release run | --help / -h | Print help |
| lf repo release check | --target / -t | target |
| lf repo release check | --help / -h | Print help |
| lf repo release notes | <version> | Version (e.g. 0.9.6) |
| lf repo release notes | --prev-tag | prev_tag |
| lf repo release notes | --preview | Print notes without updating manifests or release archives |
| lf repo release notes | --target / -t | target |
| lf repo release notes | --help / -h | Print help |
| lf repo release bump | <version> | Version to bump to (e.g. 0.9.6) |
| lf repo release bump | --target / -t | target |
| lf repo release bump | --help / -h | Print help |
| lf repo release tag | <version> | Version to tag (e.g. 0.9.6) |
| lf repo release tag | --target / -t | target |
| lf repo release tag | --help / -h | Print help |
| lf repo release publish | <tag> | Release tag (for example v0.12.4) |
| lf repo release publish | --notes | Release notes used while creating or updating the draft |
| lf repo release publish | --asset | Asset to upload; repeat for multiple files |
| lf repo release publish | --finalize | Publish the existing draft and mark it latest |
| lf repo release publish | --help / -h | Print help |
| lf repo release status | --target / -t | target |
| lf repo release status | --help / -h | Print help |
| lf repo tokens | --json | Emit as JSON |
| lf repo tokens | --days | Walk git history instead: the codebase's size on each day it changed |
| lf repo tokens | --help / -h | Print help |
| lf repo ci | --since | Relative window (7d, 24h, 30m) or RFC3339 start |
| lf repo ci | --wave | Scope to one Wave |
| lf repo ci | --repo | Scope to one GitHub owner/repo |
| lf repo ci | --json | Emit the complete incident report as JSON |
| lf repo ci | --help / -h | Print help |
| lf repo reteam | --apply | apply |
| lf repo reteam | --help / -h | Print help |
| lf home | --help / -h | Print help |
| lf home desktop | --help / -h | Print help |
| lf home screenshot | <source> | URL or local HTML file to capture |
| lf home screenshot | --output / -o | PNG destination |
| lf home screenshot | --width | Viewport width in pixels |
| lf home screenshot | --height | Viewport height in pixels |
| lf home screenshot | --help / -h | Print help |
| lf home install | --help / -h | Print help |
| lf home install schedule | <frequency> | Weekly: Monday 09:00; daily: 09:00; otherwise on clock boundaries (local time) |
| lf home install schedule | --help / -h | Print help |
| lf home install recover-switch | --switch | The fixed machine switch receipt to continue |
| lf home install recover-switch | --help / -h | Print help |
| lf home install preflight | --json | Emit the structured PromotionPreview as JSON |
| lf home install preflight | --help / -h | Print help |
| lf home install local-preflight | --store | store |
| lf home install local-preflight | --json | json |
| lf home install local-preflight | --help / -h | Print help |
| lf home install advance-switch | --switch | switch |
| lf home install advance-switch | --help / -h | Print help |
| lf home install promote | --from-build | Promote this exact unpublished local lf into a disposable installed Home |
| lf home install promote | --coordinated-build | Candidate delegated to the receipt-pinned active coordinator |
| lf home install promote | --fresh | Abandon an incompatible disposable Home and fork published data again |
| lf home install promote | --reuse-home | Reuse a retained development installation and its existing Home data |
| lf home install promote | --cli-target | The global CLI symlink to replace (e.g. ~/.local/bin/lf) |
| lf home install promote | --app-source | A staged Loopflow.app bundle to install alongside the CLI |
| lf home install promote | --app-target | The global Loopflow.app path to replace atomically |
| lf home install promote | --legacy-app-target | A retired app bundle to remove after the new app commits |
| lf home install promote | --sync-skills | Regenerate global skills after the promotion commits |
| lf home install promote | --preview | Validate and print the preview but change nothing |
| lf home install promote | --help / -h | Print help |
| lf home install rollback | --cli-target | The global CLI symlink to replace (e.g. ~/.local/bin/lf) |
| lf home install rollback | --candidate | The immutable content-addressed prior executable to activate |
| lf home install rollback | --help / -h | Print help |
| lf home sync-skills | --yes / -y | Confirm writes under ~/ without prompting |
| lf home sync-skills | --no-prune | Keep stale loopflow-generated skills |
| lf home sync-skills | --help / -h | Print help |
| lf home doctor | --planning | Diagnose repository planning without changing it |
| lf home doctor | --json | Emit the audit as JSON |
| lf home doctor | --help / -h | Print help |
| lf home ssh | --account | Prefer this origin account when the remote lf chooses a provider |
| lf home ssh | --only-account | Restrict remote provider launches to these origin accounts |
| lf home ssh | <target> | HomeId (preferred), SSH alias, or user@host |
| lf home ssh | --repo | Repository path on the remote, relative to $HOME |
| lf home ssh | --secret | Doppler secret to resolve locally and forward as an env var (repeatable). The Doppler token itself is never forwarded |
| lf home ssh | --forward-agent | Forward the ssh-agent (`ssh -A`). Off by default: git pushes use the forwarded GH_TOKEN over HTTPS, so agent forwarding is unneeded risk |
| lf home ssh | <lf_args> | Arguments for the remote lf. The target is the boundary: every argument after it belongs to the remote invocation |
| lf home ssh | --help / -h | Print help (see more with '--help') |
| lf home user | --json | json |
| lf home user | --help / -h | Print help |
| lf home id | --json | json |
| lf home id | --help / -h | Print help |
| lf home observe | <home_id> | home_id |
| lf home observe | <route> | route |
| lf home observe | --json | json |
| lf home observe | --help / -h | Print help |
| lf discord | --help / -h | Print help |
| lf discord serve | <wave> | wave |
| lf discord serve | --help / -h | Print help |
| lf wave | --help / -h | Print help |
| lf wave cron | --help / -h | Print help |
| lf wave cron add | --wave / -w | Wave name passed to `lf <flow> --wave <wave>` (ambient if omitted) |
| lf wave cron add | --flow | Flow or skill name to run |
| lf wave cron add | --schedule | Fixed-daily cron expression, or the `daily` alias |
| lf wave cron add | --help / -h | Print help |
| lf wave cron list | --wave / -w | Only jobs for this Wave |
| lf wave cron list | --json | Emit machine-readable job state |
| lf wave cron list | --help / -h | Print help |
| lf wave cron preflight | --wave / -w | Wave whose GOAL.md `crons:` are validated |
| lf wave cron preflight | --help / -h | Print help |
| lf wave cron sync | --wave / -w | Wave whose GOAL.md `crons:` drive the installed jobs |
| lf wave cron sync | --help / -h | Print help |
| lf wave cron run | --wave / -w | Wave whose installed declaration is executed |
| lf wave cron run | --flow | Flow or skill name to run |
| lf wave cron run | --scheduled | Mark a launchd-owned invocation |
| lf wave cron run | --help / -h | Print help |
| lf wave cron history | --wave / -w | Wave whose receipts are shown |
| lf wave cron history | --flow | Only receipts for this flow or skill |
| lf wave cron history | --days | Receipt window in days |
| lf wave cron history | --json | Emit machine-readable receipts |
| lf wave cron history | --help / -h | Print help |
| lf wave cron trigger | --wave / -w | Wave whose installed job is fired |
| lf wave cron trigger | --flow | Flow or skill name to run |
| lf wave cron trigger | --wait | Wait for and return the scheduled receipt |
| lf wave cron trigger | --timeout | Maximum wait for a receipt |
| lf wave cron trigger | --help / -h | Print help |
| lf wave cron remove | --wave / -w | Wave name passed to `lf <flow> --wave <wave>` |
| lf wave cron remove | --flow | Flow or skill name to remove |
| lf wave cron remove | --help / -h | Print help |
| lf wave list | --json | Emit the wave snapshot as JSON (Loopflow's dashboard snapshot) |
| lf wave list | --all | List Waves from every repository on this machine, not just the current repository (worktrees collapse to their main checkout) |
| lf wave list | --current | Exclude abandoned and retired registrations from current navigation |
| lf wave list | --help / -h | Print help |
| lf wave status | <wave> | Wave name (default: the ambient wave) |
| lf wave status | --json | Emit the status snapshot as JSON |
| lf wave status | --sync | Refresh planning from Linear before reading |
| lf wave status | --help / -h | Print help |
| lf wave connect | <wave> | Wave name (auto-detected if omitted) |
| lf wave connect | --wave / -w | Wave name (flag form; same as positional wave) |
| lf wave connect | --all | Recursively initialize every Wave under wave/ |
| lf wave connect | --team-key | Repository Team key = Task prefix (e.g. LOO). Defaults from the repository name |
| lf wave connect | --team-name | Repository Team display name. Defaults to the repository name |
| lf wave connect | --help / -h | Print help |
| lf wave sync | <wave> | wave |
| lf wave sync | --wave / -w | wave_flag |
| lf wave sync | --all | all |
| lf wave sync | --help / -h | Print help |
| lf wave rename | <wave> | wave |
| lf wave rename | --title | title |
| lf wave rename | --help / -h | Print help |
| lf wave forget | <name> | name |
| lf wave forget | --dry-run | dry_run |
| lf wave forget | --json | json |
| lf wave forget | --help / -h | Print help |
| lf wave place | <name> | name |
| lf wave place | <home_id> | home_id |
| lf wave place | --json | json |
| lf wave place | --help / -h | Print help |
| lf wave relocate | <wave> | wave |
| lf wave relocate | --repo | repo |
| lf wave relocate | --name | name |
| lf wave relocate | --json | json |
| lf wave relocate | --help / -h | Print help |
| lf wave retire | <name> | name |
| lf wave retire | --reason | reason |
| lf wave retire | --json | json |
| lf wave retire | --help / -h | Print help |
| lf wave update-plan | --wave / -w | wave |
| lf wave update-plan | --plan | plan |
| lf wave update-plan | --help / -h | Print help |
| lf task | --help / -h | Print help |
| lf task pr | --help / -h | Print help |
| lf task pr checks | --watch / -w | watch |
| lf task pr checks | --logs / -l | logs |
| lf task pr checks | --help / -h | Print help |
| lf task pr next | <slug> | Name the next serial branch (defaults to the settled PR's next slug, then the sequence number) |
| lf task pr next | --help / -h | Print help |
| lf task pr publish | --model / -m | model |
| lf task pr publish | --title | title |
| lf task pr publish | --body | body |
| lf task pr publish | --help / -h | Print help |
| lf task pr open | --model / -m | model |
| lf task pr open | --title | title |
| lf task pr open | --body | body |
| lf task pr open | --help / -h | Print help |
| lf task pr submit | --strict | strict |
| lf task pr submit | --create-pr / -p | create_pr |
| lf task pr submit | --complete / -c | complete |
| lf task pr submit | --next | next |
| lf task pr submit | --worktree / -w | worktree |
| lf task pr submit | --message / -m | message |
| lf task pr submit | --title | title |
| lf task pr submit | --body | body |
| lf task pr submit | --help / -h | Print help |
| lf task pr arm | --strict | strict |
| lf task pr arm | --local | local |
| lf task pr arm | --complete / -c | complete |
| lf task pr arm | --next | next |
| lf task pr arm | --worktree / -w | worktree |
| lf task pr arm | --message / -m | message |
| lf task pr arm | --title | title |
| lf task pr arm | --body | body |
| lf task pr arm | --help / -h | Print help |
| lf task pr land | --strict | strict |
| lf task pr land | --local | local |
| lf task pr land | --complete / -c | complete |
| lf task pr land | --next | next |
| lf task pr land | --worktree / -w | worktree |
| lf task pr land | --message / -m | message |
| lf task pr land | --title | title |
| lf task pr land | --body | body |
| lf task pr land | --help / -h | Print help |
| lf task pr abandon | <branch> | Branch to abandon (default: current) |
| lf task pr abandon | --force / -f | force |
| lf task pr abandon | --help / -h | Print help |
| lf task wt | --help / -h | Print help |
| lf task wt create | <name> | Worktree name |
| lf task wt create | --plan | Print the placement plan without creating a worktree |
| lf task wt create | --help / -h | Print help |
| lf task wt switch | <name> | Worktree name or full branch name to switch to |
| lf task wt switch | --help / -h | Print help |
| lf task wt list | --json | json |
| lf task wt list | --sync | Fetch origin and fast-forward main before listing (mutates the canonical checkout). Off by default so a list never touches it |
| lf task wt list | --help / -h | Print help |
| lf task wt prune | --dry-run | Show what would be pruned without removing anything |
| lf task wt prune | --help / -h | Print help |
| lf task wt remove | <name> | Worktree name to remove |
| lf task wt remove | --force / -f | force |
| lf task wt remove | --help / -h | Print help |
| lf task rebase | --plan | Print the planned rebase strategy without mutating git |
| lf task rebase | --manual | Keep the rebase local and leave conflicts for this process to resolve |
| lf task rebase | --continue | Stage resolved conflict paths and continue the local rebase |
| lf task rebase | --abort | Abort the local rebase in progress |
| lf task rebase | --adopt | Explicitly claim a raw rebase that has no Loopflow owner |
| lf task rebase | <onto> | Branch to rebase onto |
| lf task rebase | --help / -h | Print help |
| lf task commit | --message / -m | message |
| lf task commit | --no-add | no_add |
| lf task commit | --help / -h | Print help |
| lf task __worker | <task_id> | task_id |
| lf task __worker | --help / -h | Print help |
| lf task checkout | <issue> | issue |
| lf task checkout | --name | name |
| lf task checkout | --stack-on | Fork this Task's worktree from another Task's active PR |
| lf task checkout | --directive | directive |
| lf task checkout | --json | json |
| lf task checkout | --help / -h | Print help |
| lf task run | <issue> | issue |
| lf task run | --name | name |
| lf task run | --flow | Select a Flow for this Task worker; defaults to the chapter recommendation |
| lf task run | --stack-on | Fork this Task's worktree from another Task's active PR |
| lf task run | --directive | directive |
| lf task run | --reason | Explain what changed after an execution blocker |
| lf task run | --retry | Retry uncertain native work after confirmed engine exit |
| lf task run | --json | json |
| lf task run | --help / -h | Print help |
| lf task create | --wave | Wave name; defaults to the bound Wave |
| lf task create | --title | Task title; omitted when stdin supplies the report and first line |
| lf task create | --notes | Description; defaults to a report read from stdin |
| lf task create | --run | Validate placement and execution before filing, then run the Task |
| lf task create | --name | name |
| lf task create | --flow | Select a Flow for this Task worker; defaults to the chapter recommendation |
| lf task create | --stack-on | Fork this Task's worktree from another Task's active PR |
| lf task create | --json | json |
| lf task create | --help / -h | Print help |
| lf task status | <issue> | Task issue; defaults to the Task in this checkout |
| lf task status | --json | json |
| lf task status | --help / -h | Print help |
| lf task diff | <issue> | issue |
| lf task diff | <path> | path |
| lf task diff | --files | List changed paths and comparison revisions instead of a patch |
| lf task diff | --base | base |
| lf task diff | --draft | Compare a UTF-8 draft read from stdin without writing the worktree |
| lf task diff | --json | json |
| lf task diff | --help / -h | Print help |
| lf task file | <issue> | issue |
| lf task file | <path> | path |
| lf task file | --recoveries | Inspect retained versions, including late writes; omitted for fast content reads |
| lf task file | --json | json |
| lf task file | --help / -h | Print help |
| lf task save | <issue> | issue |
| lf task save | <path> | path |
| lf task save | --revision | revision |
| lf task save | --json | json |
| lf task save | --help / -h | Print help |
| lf task complete | <issue> | issue |
| lf task complete | --summary | summary |
| lf task complete | --json | json |
| lf task complete | --help / -h | Print help |
| lf task delete | <issue> | issue |
| lf task delete | --help / -h | Print help |
| lf task edit | <issue> | issue |
| lf task edit | --title | title |
| lf task edit | --notes | notes |
| lf task edit | --wave / -w | wave |
| lf task edit | --help / -h | Print help |
| lf task comment | <issue> | issue |
| lf task comment | <message> | message |
| lf task comment | --wave / -w | wave |
| lf task comment | --json | json |
| lf task comment | --help / -h | Print help |
| lf task interrupt | <issue> | issue |
| lf task interrupt | --json | json |
| lf task interrupt | --help / -h | Print help |
| lf task wait | <issue> | issue |
| lf task wait | --until | until |
| lf task wait | --timeout | timeout |
| lf task wait | --json | json |
| lf task wait | --help / -h | Print help |
| lf task restart | <issue> | issue |
| lf task restart | <advice> | advice |
| lf task restart | --flow | Replacement Flow; validated before any checkpoint or stop |
| lf task restart | --json | json |
| lf task restart | --help / -h | Print help |
| lf __telemetry-scorecard | --json | Emit structured JSON for operator automation |
| lf __telemetry-scorecard | --help / -h | Print help |
| lf list | <path> | path |
| lf list | --json | json |
| lf list | --help / -h | Print help |
| lf help | <path> | path |
| lf help | --all | all |
| lf help | --help / -h | Print help |
| lf roadmap | --wave | Scope to one Wave (default: every Wave in the current repository) |
| lf roadmap | --json | Emit the roadmap snapshot as JSON |
| lf roadmap | --all | Span every repository on this machine, not just the current one |
| lf roadmap | --help / -h | Print help |
| lf replay | <run> | Captured input identity or an unambiguous displayed prefix |
| lf replay | --help / -h | Print help |
| lf __flow-step | <id> | id |
| lf __flow-step | <version> | version |
| lf __flow-step | --help / -h | Print help |
| lf run | <name> | name |
| lf run | <args> | args |
| lf run | --help / -h | Print help |
| lf flow | --help / -h | Print help |
| lf flow list | --json | json |
| lf flow list | --sessions | List or show saved FlowSessions instead of reusable templates |
| lf flow list | --all | Include every repository and flows with unknown repository evidence |
| lf flow list | --limit | FlowSession page size (default 100) |
| lf flow list | --after | Previous page's next identity; retain the same filters |
| lf flow list | --search | Literal name or identity containment |
| lf flow list | --state | state |
| lf flow list | --for-task | Retained Task ID or issue identifier, including completed Tasks |
| lf flow list | --for-wave | Retained Wave ID or name |
| lf flow list | --taskless | taskless |
| lf flow list | --managed | Whether the Task currently selects this FlowSession |
| lf flow list | --help / -h | Print help |
| lf flow show | <name> | name |
| lf flow show | --json | json |
| lf flow show | --sessions | sessions |
| lf flow show | --help / -h | Print help |
| lf flow resume | <invocation> | invocation |
| lf flow resume | --retry | retry |
| lf flow resume | --help / -h | Print help |
| lf skill | --help / -h | Print help |
