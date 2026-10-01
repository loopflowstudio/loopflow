# Integrated CLI surface · 2026-09-30

127 commands below root, 428 flags, 84 positionals, 16 hidden commands, 0 registered aliases.

Compiled after merging main `de074a2eb` and completing the owner/cull changes.
This inventory includes every new landed-model input. Each current row is kept
for the distinct operation/input described below; baseline deletions and merges
remain individually recorded in [the catalog](cli-command-catalog.md).
Clap metadata is [retained verbatim](cli-catalog-current.json). No aliases exist.

## Current command decisions

All paths are canonical. Root shorthand derives from this tree. The common
caller is `src/bin/lf.rs::execute_command`; concrete readers/consumers below
name the implementation owning that operation. A namespace organizes its real
children; it creates no additional stored object. Automatic help is parser discovery.

| Command | Concept / operation | Verdict | Caller / implementation |
|---|---|---|---|
| lf | Open Loopflow or run its CLI | keep | rust/loopflow/src/bin/lf.rs; rust/loopflow/src/lf/navigation.rs |
| lf monitor | Show waiting, blocked, active, and finished work with next actions | keep | rust/loopflow/src/lf/commands/monitor.rs; swift/Loopflow/Services/RegistryQuery.swift |
| lf monitor list | List a bounded page of recorded commands, newest first | keep | rust/loopflow/src/lf/commands/monitor.rs; swift/Loopflow/Services/RegistryQuery.swift |
| lf monitor show | Inspect an Exec or Session by identity | keep | rust/loopflow/src/lf/commands/monitor.rs; swift/Loopflow/Services/RegistryQuery.swift |
| lf monitor active | Observe active conversations and missing process evidence | keep | rust/loopflow/src/lf/commands/monitor.rs; swift/Loopflow/Services/RegistryQuery.swift |
| lf monitor usage | Show direct provider-authored usage from recorded Session inputs | keep | rust/loopflow/src/lf/commands/monitor.rs; swift/Loopflow/Services/RegistryQuery.swift |
| lf monitor ps | Print one parseable snapshot of live Loopflow call trees | keep | rust/loopflow/src/lf/commands/monitor.rs; swift/Loopflow/Services/RegistryQuery.swift |
| lf monitor top | Refresh live Loopflow call trees on a terminal; print once when redirected | keep | rust/loopflow/src/lf/commands/monitor.rs; swift/Loopflow/Services/RegistryQuery.swift |
| lf monitor prune | Reap registered orphan providers and remove dead process receipts | keep | rust/loopflow/src/lf/commands/monitor.rs; swift/Loopflow/Services/RegistryQuery.swift |
| lf monitor activity | Show one ordered record of durable Work, Session, PR, and Steer facts | keep | rust/loopflow/src/lf/commands/monitor.rs; swift/Loopflow/Services/RegistryQuery.swift |
| lf : | Run an inline prompt | keep | rust/loopflow/src/bin/lf.rs; rust/loopflow/src/lf/navigation.rs |
| lf __screenshot-supervisor | Internal owner-loss supervisor for one browser capture | keep (internal) | rust/loopflow/src/bin/lf.rs; rust/loopflow/src/lf/navigation.rs |
| lf __provider-session | Internal provider callback that records one native interactive session | keep (internal) | rust/loopflow/src/bin/lf.rs; rust/loopflow/src/lf/navigation.rs |
| lf session | Inspect and continue Sessions | keep | rust/loopflow/src/lf/commands/session.rs; rust/loopflow/src/ops/human_session.rs |
| lf session ask | Open a durable session and wait for the user to complete it | keep | rust/loopflow/src/lf/commands/session.rs; rust/loopflow/src/ops/human_session.rs |
| lf session history | Read this conversation's native start, usage and completion receipts | keep | rust/loopflow/src/lf/commands/session.rs; rust/loopflow/src/ops/human_session.rs |
| lf session list | List Sessions | keep | rust/loopflow/src/lf/commands/session.rs; rust/loopflow/src/ops/human_session.rs |
| lf session connect | Connect to the live conversation, or resume its saved history | keep | rust/loopflow/src/lf/commands/session.rs; rust/loopflow/src/ops/human_session.rs |
| lf session complete | Complete a review, blocked Ask, or interactive session | keep | rust/loopflow/src/lf/commands/session.rs; rust/loopflow/src/ops/human_session.rs |
| lf session rename | Rename a Session; a human name is never replaced by a suggestion | keep | rust/loopflow/src/lf/commands/session.rs; rust/loopflow/src/ops/human_session.rs |
| lf session bind | Assign a Task to a Session that has none; the Task never changes after | keep | rust/loopflow/src/lf/commands/session.rs; rust/loopflow/src/ops/human_session.rs |
| lf session ready | Mark the active session ready for your review | keep | rust/loopflow/src/lf/commands/session.rs; rust/loopflow/src/ops/human_session.rs |
| lf session serve-flow | Run the exact review skill in its durable terminal | keep (internal) | rust/loopflow/src/lf/commands/session.rs; rust/loopflow/src/ops/human_session.rs |
| lf session serve-ask | Run one ad-hoc request in its durable terminal | keep (internal) | rust/loopflow/src/lf/commands/session.rs; rust/loopflow/src/ops/human_session.rs |
| lf session stop-client | Stop one exact native provider client after its review completes | keep (internal) | rust/loopflow/src/lf/commands/session.rs; rust/loopflow/src/ops/human_session.rs |
| lf account | Refresh account access and capacity, or manage logins and routing | keep | rust/loopflow/src/lf/commands/account.rs; rust/loopflow/src/provider_account |
| lf account disconnect | Disconnect local credentials or one managed login | keep | rust/loopflow/src/lf/commands/account.rs; rust/loopflow/src/provider_account |
| lf account connect | Connect local credentials or a managed login using a remembered browser | keep | rust/loopflow/src/lf/commands/account.rs; rust/loopflow/src/provider_account |
| lf account set | Edit account configuration or remembered browser choices | keep | rust/loopflow/src/lf/commands/account.rs; rust/loopflow/src/provider_account |
| lf account redeem-reset | Spend one banked Codex reset for this named login | keep | rust/loopflow/src/lf/commands/account.rs; rust/loopflow/src/provider_account |
| lf account route | Explain configured and automatic account selection, or replace a route | keep | rust/loopflow/src/lf/commands/account.rs; rust/loopflow/src/provider_account |
| lf account route set | Replace a provider's ordered route | keep | rust/loopflow/src/lf/commands/account.rs; rust/loopflow/src/provider_account |
| lf repo | Repository releases, source measurement, CI evidence, and provider administration | keep | rust/loopflow/src/bin/lf.rs::execute_command; rust/loopflow/src/ops |
| lf repo connect | Connect a Wave to its Initiative and the repository's Team (Task prefix) | keep | rust/loopflow/src/bin/lf.rs::execute_command; rust/loopflow/src/ops |
| lf repo refresh | Refresh shared planning from Linear | keep | rust/loopflow/src/bin/lf.rs::execute_command; rust/loopflow/src/ops |
| lf repo new-chapter | Advance every Wave to the named Project plan | keep | rust/loopflow/src/bin/lf.rs::execute_command; rust/loopflow/src/ops |
| lf repo release | Release operations (run, check, notes, bump, tag, status) | keep | rust/loopflow/src/bin/lf.rs::execute_command; rust/loopflow/src/ops |
| lf repo release run | Run the full release workflow end-to-end | keep | rust/loopflow/src/bin/lf.rs::execute_command; rust/loopflow/src/ops |
| lf repo release check | Check if PRs have merged since the last tag | keep | rust/loopflow/src/bin/lf.rs::execute_command; rust/loopflow/src/ops |
| lf repo release notes | Generate release notes for a version | keep | rust/loopflow/src/bin/lf.rs::execute_command; rust/loopflow/src/ops |
| lf repo release bump | Bump version in manifest files | keep | rust/loopflow/src/bin/lf.rs::execute_command; rust/loopflow/src/ops |
| lf repo release tag | Create a git tag and push it | keep | rust/loopflow/src/bin/lf.rs::execute_command; rust/loopflow/src/ops |
| lf repo release publish | Stage or publish a GitHub Release | keep | rust/loopflow/src/bin/lf.rs::execute_command; rust/loopflow/src/ops |
| lf repo release status | Check release workflow status | keep | rust/loopflow/src/bin/lf.rs::execute_command; rust/loopflow/src/ops |
| lf repo tokens | Measure this codebase: lines and tokens per directory (tracked files only) | keep | rust/loopflow/src/bin/lf.rs::execute_command; rust/loopflow/src/ops |
| lf repo ci | Show how failed CI is detected, repaired, and landed across this Home | keep | rust/loopflow/src/bin/lf.rs::execute_command; rust/loopflow/src/ops |
| lf repo reteam | Reconcile linked Waves to the repository's Linear Team | keep | rust/loopflow/src/bin/lf.rs::execute_command; rust/loopflow/src/ops |
| lf home | Inspect this Home and observe routes to other Homes | keep | rust/loopflow/src/bin/lf.rs::execute_command; rust/loopflow/src/machine_install.rs |
| lf home desktop | Open or focus Loopflow.app | keep | rust/loopflow/src/bin/lf.rs::execute_command; rust/loopflow/src/machine_install.rs |
| lf home screenshot | Capture a URL or local HTML file without claiming the user's browser | keep | rust/loopflow/src/bin/lf.rs::execute_command; rust/loopflow/src/machine_install.rs |
| lf home install | Install the latest published Loopflow release from any directory | keep | rust/loopflow/src/bin/lf.rs::execute_command; rust/loopflow/src/machine_install.rs |
| lf home install schedule | Install the latest Loopflow at login and weekly by default (macOS launchd) | keep | rust/loopflow/src/bin/lf.rs::execute_command; rust/loopflow/src/machine_install.rs |
| lf home install recover-switch | Continue one interrupted machine install switch from its pinned candidate | keep (internal) | rust/loopflow/src/bin/lf.rs::execute_command; rust/loopflow/src/machine_install.rs |
| lf home install preflight | Preview whether this build may replace the global lf (read-only). Reads the shared store's migration frontier and validates executable planning references against this binary without changing that frontier. Exits non-zero on refusal so a caller can gate on it | keep (internal) | rust/loopflow/src/bin/lf.rs::execute_command; rust/loopflow/src/machine_install.rs |
| lf home install local-preflight | Validate this exact local candidate against one receipt-selected store | keep (internal) | rust/loopflow/src/bin/lf.rs::execute_command; rust/loopflow/src/machine_install.rs |
| lf home install advance-switch | Advance the receipt-selected store with this exact candidate's registry | keep (internal) | rust/loopflow/src/bin/lf.rs::execute_command; rust/loopflow/src/machine_install.rs |
| lf home install promote | Promote this build to the global CLI: content-address it into ~/.lf/bin and atomically repoint the target symlink, under the exclusive promotion lock. Refuses — leaving every target unchanged — on incompatible schema or persisted executable evidence | keep (internal) | rust/loopflow/src/bin/lf.rs::execute_command; rust/loopflow/src/machine_install.rs |
| lf home install rollback | Repoint the global CLI at retained prior bytes only after that binary's own preflight proves it recognizes the current store frontier | keep (internal) | rust/loopflow/src/bin/lf.rs::execute_command; rust/loopflow/src/machine_install.rs |
| lf home sync-skills | Compile loopflow skills into your home vendor Skills directories | keep (internal) | rust/loopflow/src/bin/lf.rs::execute_command; rust/loopflow/src/machine_install.rs |
| lf home doctor | Audit recorded Session inputs: continuity, vocabulary, attribution, identity, lineage, coverage | keep | rust/loopflow/src/bin/lf.rs::execute_command; rust/loopflow/src/machine_install.rs |
| lf home ssh | Run lf on a Home or SSH host carrying your local credentials | keep | rust/loopflow/src/bin/lf.rs::execute_command; rust/loopflow/src/machine_install.rs |
| lf home user | Print the configured participant display name | keep | rust/loopflow/src/bin/lf.rs::execute_command; rust/loopflow/src/machine_install.rs |
| lf home id | Print this machine's stable local Home identity | keep | rust/loopflow/src/bin/lf.rs::execute_command; rust/loopflow/src/machine_install.rs |
| lf home observe | Record the current route for a known Home identity | keep | rust/loopflow/src/bin/lf.rs::execute_command; rust/loopflow/src/machine_install.rs |
| lf discord | Bridge new Discord messages to finite Wave Sessions | keep | rust/loopflow/src/bin/lf.rs; rust/loopflow/src/lf/navigation.rs |
| lf discord serve | Poll a configured channel and post each Session's final answer | keep | rust/loopflow/src/bin/lf.rs; rust/loopflow/src/lf/navigation.rs |
| lf wave | Manage Wave identity, placement and planning | keep | rust/loopflow/src/bin/lf.rs::run_wave_command; rust/loopflow/src/lf/commands/placement.rs |
| lf wave cron | Local launchd jobs that run lf commands on a schedule | keep | rust/loopflow/src/bin/lf.rs::run_wave_command; rust/loopflow/src/lf/commands/placement.rs |
| lf wave cron add | Install or replace a scheduled lf invocation | keep | rust/loopflow/src/bin/lf.rs::run_wave_command; rust/loopflow/src/lf/commands/placement.rs |
| lf wave cron list | List installed loopflow cron jobs | keep | rust/loopflow/src/bin/lf.rs::run_wave_command; rust/loopflow/src/lf/commands/placement.rs |
| lf wave cron preflight | Validate Home authority and declared jobs without changing launchd | keep | rust/loopflow/src/bin/lf.rs::run_wave_command; rust/loopflow/src/lf/commands/placement.rs |
| lf wave cron sync | Reconcile installed launchd jobs to match a wave's declared `crons:` | keep | rust/loopflow/src/bin/lf.rs::run_wave_command; rust/loopflow/src/lf/commands/placement.rs |
| lf wave cron run | Execute one installed cron job and persist its terminal receipt | keep (internal) | rust/loopflow/src/bin/lf.rs::run_wave_command; rust/loopflow/src/lf/commands/placement.rs |
| lf wave cron history | Show durable cron receipts | keep | rust/loopflow/src/bin/lf.rs::run_wave_command; rust/loopflow/src/lf/commands/placement.rs |
| lf wave cron trigger | Ask launchd to fire an installed job | keep | rust/loopflow/src/bin/lf.rs::run_wave_command; rust/loopflow/src/lf/commands/placement.rs |
| lf wave cron remove | Uninstall a scheduled lf invocation | keep | rust/loopflow/src/bin/lf.rs::run_wave_command; rust/loopflow/src/lf/commands/placement.rs |
| lf wave list | List authored Waves and retained planning identities without starting work | keep | rust/loopflow/src/bin/lf.rs::run_wave_command; rust/loopflow/src/lf/commands/placement.rs |
| lf wave status | Show one Wave's current plan, Task details, and execution evidence | keep | rust/loopflow/src/lf/commands/waves.rs::status |
| lf wave place | Set the Home for Wave schedules and newly created work | keep | rust/loopflow/src/bin/lf.rs::run_wave_command; rust/loopflow/src/lf/commands/placement.rs |
| lf wave rename | Rename or relocate an authored Wave and its provider mapping | keep | rust/loopflow/src/bin/lf.rs::run_wave_command; rust/loopflow/src/lf/commands/placement.rs |
| lf wave update-plan | Replace the current chapter's KRs, targets, and Flow recommendation | keep | rust/loopflow/src/bin/lf.rs::run_wave_command; rust/loopflow/src/lf/commands/placement.rs |
| lf task | Concrete work, worktrees, commits, and pull requests | keep | rust/loopflow/src/bin/lf.rs::run_task_command; swift/Loopflow/Services/RegistryQuery.swift |
| lf task pr | Pull request lifecycle | keep | rust/loopflow/src/bin/lf.rs::run_task_command; swift/Loopflow/Services/RegistryQuery.swift |
| lf task pr checks | Show CI status for current branch | keep | rust/loopflow/src/bin/lf.rs::run_task_command; swift/Loopflow/Services/RegistryQuery.swift |
| lf task pr next | After an out-of-band merge, rotate this Task to its next serial PR, carrying committed and uncommitted follow-up onto the new branch | keep | rust/loopflow/src/bin/lf.rs::run_task_command; swift/Loopflow/Services/RegistryQuery.swift |
| lf task pr publish | Publish a ready PR headlessly: push, create or refresh, print state + URL. Opens no review surface | keep | rust/loopflow/src/bin/lf.rs::run_task_command; swift/Loopflow/Services/RegistryQuery.swift |
| lf task pr open | Push and create or update a draft PR, then open its GitHub page. Existing ready PRs stay ready; opening a draft does not publish it | keep | rust/loopflow/src/bin/lf.rs::run_task_command; swift/Loopflow/Services/RegistryQuery.swift |
| lf task pr submit | Prepare a PR to land: sync, clear scratch, mark ready, and assign it to you. Nothing merges until you click merge on GitHub | keep | rust/loopflow/src/bin/lf.rs::run_task_command; swift/Loopflow/Services/RegistryQuery.swift |
| lf task pr arm | Prepare a PR, request exact-head auto-merge, and return without watching | keep | rust/loopflow/src/bin/lf.rs::run_task_command; swift/Loopflow/Services/RegistryQuery.swift |
| lf task pr land | Arm and watch a PR through CI repair and authoritative merge | keep | rust/loopflow/src/bin/lf.rs::run_task_command; swift/Loopflow/Services/RegistryQuery.swift |
| lf task pr abandon | Abandon branch: close PR, remove worktree, delete branch | keep | rust/loopflow/src/bin/lf.rs::run_task_command; swift/Loopflow/Services/RegistryQuery.swift |
| lf task wt | Worktree operations | keep | rust/loopflow/src/bin/lf.rs::run_task_command; swift/Loopflow/Services/RegistryQuery.swift |
| lf task wt create | Create a low-level sibling worktree | keep | rust/loopflow/src/bin/lf.rs::run_task_command; swift/Loopflow/Services/RegistryQuery.swift |
| lf task wt switch | Switch to a worktree by name, identity leaf, or full branch | keep | rust/loopflow/src/bin/lf.rs::run_task_command; swift/Loopflow/Services/RegistryQuery.swift |
| lf task wt list | List worktrees (read-only; reflects the last-synced main) | keep | rust/loopflow/src/bin/lf.rs::run_task_command; swift/Loopflow/Services/RegistryQuery.swift |
| lf task wt prune | Remove clean terminal or inactive worktrees | keep | rust/loopflow/src/bin/lf.rs::run_task_command; swift/Loopflow/Services/RegistryQuery.swift |
| lf task wt delete | Delete a worktree and its local and remote branch; retain PR and Task outcomes | keep | rust/loopflow/src/bin/lf.rs::run_task_command; swift/Loopflow/Services/RegistryQuery.swift |
| lf task sync | Merge upstream into the current branch (default: main or stack parent) | keep | rust/loopflow/src/bin/lf.rs::run_task_command; swift/Loopflow/Services/RegistryQuery.swift |
| lf task commit | Commit changes | keep | rust/loopflow/src/bin/lf.rs::run_task_command; swift/Loopflow/Services/RegistryQuery.swift |
| lf task __worker | Internal: drive a Task Flow from its claimed boundary | keep (internal) | rust/loopflow/src/bin/lf.rs::run_task_command; swift/Loopflow/Services/RegistryQuery.swift |
| lf task checkout | Ensure tracked Task Work and its worktree without starting a worker | keep | rust/loopflow/src/bin/lf.rs::run_task_command; swift/Loopflow/Services/RegistryQuery.swift |
| lf task create | File a Task in the current chapter; optionally prepare and run it | keep | rust/loopflow/src/bin/lf.rs::run_task_command; swift/Loopflow/Services/RegistryQuery.swift |
| lf task status | Show durable Task facts and current worker evidence | keep | rust/loopflow/src/bin/lf.rs::run_task_command; swift/Loopflow/Services/RegistryQuery.swift |
| lf task diff | Show this Task's patch or list its changed files | keep | rust/loopflow/src/bin/lf.rs::run_task_command; swift/Loopflow/Services/RegistryQuery.swift |
| lf task file | Read one file from this Task's worktree | keep | rust/loopflow/src/bin/lf.rs::run_task_command; swift/Loopflow/Services/RegistryQuery.swift |
| lf task save | Save UTF-8 stdin with an expected revision and retained recovery files | keep | rust/loopflow/src/bin/lf.rs::run_task_command; swift/Loopflow/Services/RegistryQuery.swift |
| lf task complete | Complete planning work, or a placed Task whose pull requests are settled | keep | rust/loopflow/src/bin/lf.rs::run_task_command; swift/Loopflow/Services/RegistryQuery.swift |
| lf task abandon | Cancel the Task in Linear and locally, close its PRs and delete its branches | keep | rust/loopflow/src/bin/lf.rs::run_task_command; swift/Loopflow/Services/RegistryQuery.swift |
| lf task sweep | Preview open issues outside current chapters; apply safe cancellations explicitly | keep | rust/loopflow/src/bin/lf.rs::run_task_command; swift/Loopflow/Services/RegistryQuery.swift |
| lf task delete | Cancel unfinished placed work, clean up delivery, then trash the Linear issue | keep | rust/loopflow/src/bin/lf.rs::run_task_command; swift/Loopflow/Services/RegistryQuery.swift |
| lf task edit | Edit a Task's title or notes, before or after placement | keep | rust/loopflow/src/bin/lf.rs::run_task_command; swift/Loopflow/Services/RegistryQuery.swift |
| lf task comment | Read the thread or publish a comment; agent comments default to progress | keep | rust/loopflow/src/bin/lf.rs::run_task_command; swift/Loopflow/Services/RegistryQuery.swift |
| lf task interrupt | Interrupt the active provider turn | keep | rust/loopflow/src/bin/lf.rs::run_task_command; swift/Loopflow/Services/RegistryQuery.swift |
| lf task wait | Wait without polling an LM | keep | rust/loopflow/src/bin/lf.rs::run_task_command; swift/Loopflow/Services/RegistryQuery.swift |
| lf task restart | Stop the pinned Flow and begin a new one in a fresh Task worker; defaults to the chapter's currently recommended Flow | keep | rust/loopflow/src/bin/lf.rs::run_task_command; swift/Loopflow/Services/RegistryQuery.swift |
| lf __telemetry-scorecard | Internal: render the repository maintainer scorecard for telemetry-daily | keep (internal) | rust/loopflow/src/bin/lf.rs; rust/loopflow/src/lf/navigation.rs |
| lf list | Discover commands, skills, and flows | keep | rust/loopflow/src/bin/lf.rs; rust/loopflow/src/lf/navigation.rs |
| lf help | Explain a command, skill, or flow without launching it | keep | rust/loopflow/src/bin/lf.rs; rust/loopflow/src/lf/navigation.rs |
| lf roadmap | Show the current repository's roadmap: every open Task across the repo's Waves, joined to live evidence and bucketed into Now / Waiting / Available / Later. `--wave` scopes it; `--all` spans every repository on this machine. Local-only, deterministic | keep | rust/loopflow/src/bin/lf.rs; rust/loopflow/src/lf/navigation.rs |
| lf replay | Launch the immutable provider request retained for a captured input | keep | rust/loopflow/src/bin/lf.rs; rust/loopflow/src/lf/navigation.rs |
| lf __flow-step | Execute one captured Flow boundary in its own process | keep (internal) | rust/loopflow/src/bin/lf.rs; rust/loopflow/src/lf/navigation.rs |
| lf run | Run a definition, preferring a flow over a same-named skill | keep | rust/loopflow/src/bin/lf.rs; rust/loopflow/src/lf/navigation.rs |
| lf flow | Run or inspect authored flows | keep | rust/loopflow/src/lf/commands/flow.rs; rust/loopflow/src/controller/task/mod.rs |
| lf flow start | Start or continue a Task through its saved Flow | keep | rust/loopflow/src/lf/commands/flow.rs; rust/loopflow/src/controller/task/mod.rs |
| lf flow list | List authored flows or saved FlowSessions | keep | rust/loopflow/src/lf/commands/flow.rs; rust/loopflow/src/controller/task/mod.rs |
| lf flow show | Inspect an authored flow or a saved FlowSession | keep | rust/loopflow/src/lf/commands/flow.rs; rust/loopflow/src/controller/task/mod.rs |
| lf flow resume | Continue a saved Flow invocation | keep | rust/loopflow/src/lf/commands/flow.rs; rust/loopflow/src/controller/task/mod.rs |
| lf skill | Run a skill explicitly | keep | rust/loopflow/src/bin/lf.rs; rust/loopflow/src/lf/navigation.rs |

## Every current input

Each option belongs to the command shown, with no parallel alias. Query-local
Task/Wave/Project filters select historical observations; root selectors choose
execution location/context. Preview, mutation, provider permission and account
restriction remain distinct inputs. Callers inherit the command evidence above.

| Command | Input | Distinct behavior | Verdict |
|---|---|---|---|
| lf | --docs | Docs paths, globs, or directories to include in context | keep |
| lf | --clipboard / -c | Include clipboard content in prompt | keep |
| lf | --model / -m | Model to use (harness or harness:model) | keep |
| lf | --account | Prefer this managed provider login before the normal route. Repeat to select provider-qualified preferences such as `claude=jack@`. Logins spend; a profile is only the Chrome venue accounts log in through, so it is never a run-time selector | keep |
| lf | --only-account | Restrict this invocation and its children to exactly these managed provider logins. Providers without a selection are unavailable | keep |
| lf | --__account-lease-probe | Internal SSH compatibility and broker-connectivity probe | keep (internal) |
| lf | --yolo | Skip permission prompts | keep |
| lf | --mode | Choose the provider surface; omission inherits configuration and terminal context | keep |
| lf | --chrome | Override Chrome integration; omission inherits configuration | keep |
| lf | --diff | Select changed-code context; omission inherits configuration | keep |
| lf | --max-turns | Maximum agent turns for this invocation | keep |
| lf | --wave | Add Wave context and identity without changing the working directory | keep |
| lf | --task | Execute in this Task's checkout | keep |
| lf | --wt | Execute in an existing worktree by name or branch | keep |
| lf | --__cwd | Keep a Work-bound internal launch in this exact checkout | keep (internal) |
| lf | --no-loopflow | Exclude loopflow operating guidance | keep |
| lf | --__flow-step | Execute a skill from this saved Flow boundary, without resolving its definition again | keep (internal) |
| lf | --help / -h | Print help | keep |
| lf | --version / -V | Print version | keep |
| lf monitor | --json | json | keep |
| lf monitor | --all | all | keep |
| lf monitor | --help / -h | Print help | keep |
| lf monitor list | --json | json | keep |
| lf monitor list | --all | Include all repositories | keep |
| lf monitor list | --limit | limit | keep |
| lf monitor list | --after | Continue with the previous page's next object, encoded as JSON | keep |
| lf monitor list | --parent | Direct children of an exact or unambiguous parent Exec | keep |
| lf monitor list | --caller | Commands issued by this AgentSession | keep |
| lf monitor list | --search | Literal command text, ignoring ASCII case | keep |
| lf monitor list | --outcome | outcome | keep |
| lf monitor list | --task | Recorded work for a Task, including completed Tasks | keep |
| lf monitor list | --wave | Recorded work for a Wave | keep |
| lf monitor list | --help / -h | Print help | keep |
| lf monitor show | <id> | id | keep |
| lf monitor show | --json | json | keep |
| lf monitor show | --events | Print a Session's retained raw input events | keep |
| lf monitor show | --final | Print a Session's last recorded provider conclusion | keep |
| lf monitor show | --input | Inspect an exact retained input belonging to this Session | keep |
| lf monitor show | --help / -h | Print help | keep |
| lf monitor active | --json | json | keep |
| lf monitor active | --watch | Stream NDJSON until stdin closes | keep |
| lf monitor active | --task | task | keep |
| lf monitor active | --help / -h | Print help | keep |
| lf monitor usage | --json | Emit Session usage evidence as JSON | keep |
| lf monitor usage | --days | Observation window, in days (zero means all time) | keep |
| lf monitor usage | --parent | Inputs issued by this Session or retained capture | keep |
| lf monitor usage | --wave | Limit to Session inputs attributed to one Wave | keep |
| lf monitor usage | --project | Limit to Session inputs attributed to one Project | keep |
| lf monitor usage | --task | Limit to Session inputs attributed to one Task | keep |
| lf monitor usage | --help / -h | Print help | keep |
| lf monitor ps | --json | Emit the versioned activity snapshot as JSON | keep |
| lf monitor ps | --help / -h | Print help | keep |
| lf monitor top | --json | Emit one versioned activity snapshot as JSON | keep |
| lf monitor top | --help / -h | Print help | keep |
| lf monitor prune | --dry-run | Show exact targets without changing process or receipt state | keep |
| lf monitor prune | --json | Emit the versioned prune report as JSON | keep |
| lf monitor prune | --help / -h | Print help | keep |
| lf monitor activity | --since | Relative window (7d, 24h, 30m) or RFC3339 start | keep |
| lf monitor activity | --limit | Maximum rows after Work filters (1-200) | keep |
| lf monitor activity | --wave | Scope to one Wave by name | keep |
| lf monitor activity | --project | Scope to one Project by slug | keep |
| lf monitor activity | --task | Scope to one Task by Linear identifier | keep |
| lf monitor activity | --json | Emit the typed activity snapshot as JSON | keep |
| lf monitor activity | --help / -h | Print help | keep |
| lf : | <prompt> | prompt | keep |
| lf : | --help / -h | Print help | keep |
| lf __screenshot-supervisor | <source> | URL or local HTML file to capture | keep |
| lf __screenshot-supervisor | --output / -o | PNG destination | keep |
| lf __screenshot-supervisor | --width | Viewport width in pixels | keep |
| lf __screenshot-supervisor | --height | Viewport height in pixels | keep |
| lf __screenshot-supervisor | --help / -h | Print help | keep |
| lf __provider-session | --help / -h | Print help | keep |
| lf session | --help / -h | Print help | keep |
| lf session ask | --skill | Named skill for the session | keep |
| lf session ask | <question> | What the session should work through | keep |
| lf session ask | --help / -h | Print help | keep |
| lf session history | <id> | id | keep |
| lf session history | --json | json | keep |
| lf session history | --after | Continue after an observed event sequence | keep |
| lf session history | --limit | limit | keep |
| lf session history | --help / -h | Print help | keep |
| lf session list | --json | json | keep |
| lf session list | --all | Include waiting steps from every repository on this machine | keep |
| lf session list | --interactive | Select interactive (true), headless (false), or both (all) | keep |
| lf session list | --history | Include completed conversations and historical reviews | keep |
| lf session list | --limit | Maximum conversations; 0 reads the complete matching inventory | keep |
| lf session list | --offset | offset | keep |
| lf session list | --page | Return a bounded stable-ID page with a continuation cursor | keep |
| lf session list | --after | Previous page's next identity; keep the same filters | keep |
| lf session list | --task | task | keep |
| lf session list | --search | search | keep |
| lf session list | --help / -h | Print help | keep |
| lf session connect | <id> | id | keep |
| lf session connect | --json | json | keep |
| lf session connect | --replace | Stop Loopflow-owned clients before resuming here | keep |
| lf session connect | --try | Ask the provider to resume even when another client is active | keep |
| lf session connect | --help / -h | Print help | keep |
| lf session complete | <id> | id | keep |
| lf session complete | --help / -h | Print help | keep |
| lf session rename | <id> | id | keep |
| lf session rename | <name> | name | keep |
| lf session rename | --suggest | Propose an agent-generated name; keeps a human-assigned name | keep |
| lf session rename | --json | json | keep |
| lf session rename | --help / -h | Print help | keep |
| lf session bind | <id> | id | keep |
| lf session bind | --task | The Task, by its issue identifier (e.g. INF-123) or stable Task ID | keep |
| lf session bind | --dry-run | Resolve the exact target without assigning the Session | keep |
| lf session bind | --json | json | keep |
| lf session bind | --help / -h | Print help | keep |
| lf session ready | <summary> | summary | keep |
| lf session ready | --help / -h | Print help | keep |
| lf session serve-flow | <task_id> | task id | keep |
| lf session serve-flow | <invocation_id> | invocation id | keep |
| lf session serve-flow | <flow> | flow | keep |
| lf session serve-flow | <node_id> | node id | keep |
| lf session serve-flow | <skill> | skill | keep |
| lf session serve-flow | <iteration> | iteration | keep |
| lf session serve-flow | --help / -h | Print help | keep |
| lf session serve-ask | <input> | input | keep |
| lf session serve-ask | --help / -h | Print help | keep |
| lf session stop-client | <input> | input | keep |
| lf session stop-client | --help / -h | Print help | keep |
| lf account | <provider> | Limit observations to one provider | keep |
| lf account | --cached | Inspect cached evidence without contacting providers or the origin broker | keep |
| lf account | --details | Include credential sources, browser choices, and timestamps | keep |
| lf account | --json | Emit the account overview as one JSON document | keep |
| lf account | --help / -h | Print help | keep |
| lf account disconnect | <provider> | provider | keep |
| lf account disconnect | <email> | email | keep |
| lf account disconnect | --help / -h | Print help | keep |
| lf account connect | <provider> | provider | keep |
| lf account connect | <email> | email | keep |
| lf account connect | --chrome-profile | chrome profile | keep |
| lf account connect | --import | Adopt an existing Claude login | keep |
| lf account connect | --api-key | Read the provider's API key environment variable | keep |
| lf account connect | --help / -h | Print help | keep |
| lf account set | <provider> | provider | keep |
| lf account set | <email> | email | keep |
| lf account set | --login-email | login email | keep |
| lf account set | --routing | routing | keep |
| lf account set | --plan | plan | keep |
| lf account set | --clear-plan | clear plan | keep |
| lf account set | --paid-through | paid through | keep |
| lf account set | --clear-paid-through | clear paid through | keep |
| lf account set | --clear-cooldown | clear cooldown | keep |
| lf account set | --chrome-profile | Replace the ordered browser choices (repeat for fallback profiles) | keep |
| lf account set | --clear-chrome-profiles | clear chrome profiles | keep |
| lf account set | --help / -h | Print help | keep |
| lf account redeem-reset | <provider> | provider | keep |
| lf account redeem-reset | <email> | email | keep |
| lf account redeem-reset | --idempotency-key | Reuse this key when retrying the same redemption | keep |
| lf account redeem-reset | --credit-id | Opaque credit ID returned by live status (otherwise the service chooses) | keep |
| lf account redeem-reset | --json | json | keep |
| lf account redeem-reset | --help / -h | Print help | keep |
| lf account route | --repo | repo | keep |
| lf account route | --default | default | keep |
| lf account route | --json | json | keep |
| lf account route | --help / -h | Print help | keep |
| lf account route set | <provider> | provider | keep |
| lf account route set | <accounts> | accounts | keep |
| lf account route set | --repo | repo | keep |
| lf account route set | --default | default | keep |
| lf account route set | --help / -h | Print help | keep |
| lf repo | --help / -h | Print help | keep |
| lf repo connect | <wave> | Wave name (auto-detected if omitted) | keep |
| lf repo connect | --all | Recursively initialize every Wave under wave/ | keep |
| lf repo connect | --team-key | Repository Team key = Task prefix (e.g. LOO). Defaults from the repository name | keep |
| lf repo connect | --team-name | Repository Team display name. Defaults to the repository name | keep |
| lf repo connect | --help / -h | Print help | keep |
| lf repo refresh | <wave> | wave | keep |
| lf repo refresh | --all | all | keep |
| lf repo refresh | --help / -h | Print help | keep |
| lf repo new-chapter | <name> | name | keep |
| lf repo new-chapter | --dry-run | dry run | keep |
| lf repo new-chapter | --json | json | keep |
| lf repo new-chapter | --help / -h | Print help | keep |
| lf repo release | --help / -h | Print help | keep |
| lf repo release run | <version> | Version to release: patch\|minor\|major\|X.Y.Z (default: patch) | keep |
| lf repo release run | --target / -t | target | keep |
| lf repo release run | --help / -h | Print help | keep |
| lf repo release check | --target / -t | target | keep |
| lf repo release check | --help / -h | Print help | keep |
| lf repo release notes | <version> | Version (e.g. 0.9.6) | keep |
| lf repo release notes | --prev-tag | prev tag | keep |
| lf repo release notes | --preview | Print notes without updating manifests or release archives | keep |
| lf repo release notes | --target / -t | target | keep |
| lf repo release notes | --help / -h | Print help | keep |
| lf repo release bump | <version> | Version to bump to (e.g. 0.9.6) | keep |
| lf repo release bump | --target / -t | target | keep |
| lf repo release bump | --help / -h | Print help | keep |
| lf repo release tag | <version> | Version to tag (e.g. 0.9.6) | keep |
| lf repo release tag | --target / -t | target | keep |
| lf repo release tag | --help / -h | Print help | keep |
| lf repo release publish | <tag> | Release tag (for example v0.12.4) | keep |
| lf repo release publish | --notes | Release notes used while creating or updating the draft | keep |
| lf repo release publish | --asset | Asset to upload; repeat for multiple files | keep |
| lf repo release publish | --finalize | Publish the existing draft and mark it latest | keep |
| lf repo release publish | --help / -h | Print help | keep |
| lf repo release status | --target / -t | target | keep |
| lf repo release status | --help / -h | Print help | keep |
| lf repo tokens | --json | Emit as JSON | keep |
| lf repo tokens | --days | Walk git history instead: the codebase's size on each day it changed | keep |
| lf repo tokens | --help / -h | Print help | keep |
| lf repo ci | --since | Relative window (7d, 24h, 30m) or RFC3339 start | keep |
| lf repo ci | --wave | Scope to one Wave | keep |
| lf repo ci | --repo | Scope to one GitHub owner/repo | keep |
| lf repo ci | --json | Emit the complete incident report as JSON | keep |
| lf repo ci | --help / -h | Print help | keep |
| lf repo reteam | --apply | apply | keep |
| lf repo reteam | --help / -h | Print help | keep |
| lf home | --help / -h | Print help | keep |
| lf home desktop | --help / -h | Print help | keep |
| lf home screenshot | <source> | URL or local HTML file to capture | keep |
| lf home screenshot | --output / -o | PNG destination | keep |
| lf home screenshot | --width | Viewport width in pixels | keep |
| lf home screenshot | --height | Viewport height in pixels | keep |
| lf home screenshot | --help / -h | Print help | keep |
| lf home install | --help / -h | Print help | keep |
| lf home install schedule | <frequency> | Weekly: Monday 09:00; daily: 09:00; otherwise on clock boundaries (local time) | keep |
| lf home install schedule | --help / -h | Print help | keep |
| lf home install recover-switch | --switch | The fixed machine switch receipt to continue | keep |
| lf home install recover-switch | --help / -h | Print help | keep |
| lf home install preflight | --json | Emit the structured PromotionPreview as JSON | keep |
| lf home install preflight | --help / -h | Print help | keep |
| lf home install local-preflight | --store | store | keep |
| lf home install local-preflight | --json | json | keep |
| lf home install local-preflight | --help / -h | Print help | keep |
| lf home install advance-switch | --switch | switch | keep |
| lf home install advance-switch | --help / -h | Print help | keep |
| lf home install promote | --from-build | Promote this exact unpublished local lf into a disposable installed Home | keep |
| lf home install promote | --coordinated-build | Candidate delegated to the receipt-pinned active coordinator | keep (internal) |
| lf home install promote | --fresh | Abandon an incompatible disposable Home and fork published data again | keep |
| lf home install promote | --reuse-home | Reuse a retained development installation and its existing Home data | keep |
| lf home install promote | --cli-target | The global CLI symlink to replace (e.g. ~/.local/bin/lf) | keep |
| lf home install promote | --app-source | A staged Loopflow.app bundle to install alongside the CLI | keep |
| lf home install promote | --app-target | The global Loopflow.app path to replace atomically | keep |
| lf home install promote | --legacy-app-target | A retired app bundle to remove after the new app commits | keep |
| lf home install promote | --sync-skills | Regenerate global skills after the promotion commits | keep |
| lf home install promote | --preview | Validate and print the preview but change nothing | keep |
| lf home install promote | --help / -h | Print help | keep |
| lf home install rollback | --cli-target | The global CLI symlink to replace (e.g. ~/.local/bin/lf) | keep |
| lf home install rollback | --candidate | The immutable content-addressed prior executable to activate | keep |
| lf home install rollback | --help / -h | Print help | keep |
| lf home sync-skills | --yes / -y | Confirm writes under ~/ without prompting | keep |
| lf home sync-skills | --no-prune | Keep stale loopflow-generated skills | keep |
| lf home sync-skills | --help / -h | Print help | keep |
| lf home doctor | --planning | Diagnose repository planning without changing it | keep |
| lf home doctor | --json | Emit the audit as JSON | keep |
| lf home doctor | --help / -h | Print help | keep |
| lf home ssh | --account | Prefer this origin account when the remote lf chooses a provider | keep |
| lf home ssh | --only-account | Restrict remote provider launches to these origin accounts | keep |
| lf home ssh | <target> | HomeId (preferred), SSH alias, or user@host | keep |
| lf home ssh | --repo | Repository path on the remote, relative to $HOME | keep |
| lf home ssh | --secret | Doppler secret to resolve locally and forward as an env var (repeatable). The Doppler token itself is never forwarded | keep |
| lf home ssh | --forward-agent | Forward the ssh-agent (`ssh -A`). Off by default: git pushes use the forwarded GH_TOKEN over HTTPS, so agent forwarding is unneeded risk | keep |
| lf home ssh | <lf_args> | Arguments for the remote lf. The target is the boundary: every argument after it belongs to the remote invocation | keep |
| lf home ssh | --help / -h | Print help (see more with '--help') | keep |
| lf home user | --json | json | keep |
| lf home user | --help / -h | Print help | keep |
| lf home id | --json | json | keep |
| lf home id | --help / -h | Print help | keep |
| lf home observe | <home_id> | home id | keep |
| lf home observe | <route> | route | keep |
| lf home observe | --json | json | keep |
| lf home observe | --help / -h | Print help | keep |
| lf discord | --help / -h | Print help | keep |
| lf discord serve | <wave> | wave | keep |
| lf discord serve | --help / -h | Print help | keep |
| lf wave | --help / -h | Print help | keep |
| lf wave cron | --help / -h | Print help | keep |
| lf wave cron add | --wave / -w | Wave name passed to `lf <flow> --wave <wave>` (ambient if omitted) | keep |
| lf wave cron add | --flow | Flow or skill name to run | keep |
| lf wave cron add | --schedule | Fixed-daily cron expression, or the `daily` alias | keep |
| lf wave cron add | --help / -h | Print help | keep |
| lf wave cron list | --wave / -w | Only jobs for this Wave | keep |
| lf wave cron list | --json | Emit machine-readable job state | keep |
| lf wave cron list | --help / -h | Print help | keep |
| lf wave cron preflight | --wave / -w | Wave whose GOAL.md `crons:` are validated | keep |
| lf wave cron preflight | --help / -h | Print help | keep |
| lf wave cron sync | --wave / -w | Wave whose GOAL.md `crons:` drive the installed jobs | keep |
| lf wave cron sync | --help / -h | Print help | keep |
| lf wave cron run | --wave / -w | Wave whose installed declaration is executed | keep |
| lf wave cron run | --flow | Flow or skill name to run | keep |
| lf wave cron run | --scheduled | Mark a launchd-owned invocation | keep (internal) |
| lf wave cron run | --help / -h | Print help | keep |
| lf wave cron history | --wave / -w | Wave whose receipts are shown | keep |
| lf wave cron history | --flow | Only receipts for this flow or skill | keep |
| lf wave cron history | --days | Receipt window in days | keep |
| lf wave cron history | --json | Emit machine-readable receipts | keep |
| lf wave cron history | --help / -h | Print help | keep |
| lf wave cron trigger | --wave / -w | Wave whose installed job is fired | keep |
| lf wave cron trigger | --flow | Flow or skill name to run | keep |
| lf wave cron trigger | --wait | Wait for and return the scheduled receipt | keep |
| lf wave cron trigger | --timeout | Maximum wait for a receipt | keep |
| lf wave cron trigger | --help / -h | Print help | keep |
| lf wave cron remove | --wave / -w | Wave name passed to `lf <flow> --wave <wave>` | keep |
| lf wave cron remove | --flow | Flow or skill name to remove | keep |
| lf wave cron remove | --help / -h | Print help | keep |
| lf wave list | --json | Emit the wave snapshot as JSON (Loopflow's dashboard snapshot) | keep |
| lf wave list | --all | List Waves from every repository on this machine, not just the current repository (worktrees collapse to their main checkout) | keep |
| lf wave list | --current | Exclude abandoned and retired registrations from current navigation | keep |
| lf wave list | --help / -h | Print help | keep |
| lf wave status | <wave> | Wave name (default: the ambient wave) | keep |
| lf wave status | --json | Emit the status snapshot as JSON | keep |
| lf wave status | --sync | Refresh planning from Linear before reading | keep |
| lf wave status | --help / -h | Print help | keep |
| lf wave place | <name> | name | keep |
| lf wave place | <home_id> | home id | keep |
| lf wave place | --json | json | keep |
| lf wave place | --help / -h | Print help | keep |
| lf wave rename | <wave> | wave | keep |
| lf wave rename | --repo | repo | keep |
| lf wave rename | --name | name | keep |
| lf wave rename | --title | Change the linked Initiative display title | keep |
| lf wave rename | --json | json | keep |
| lf wave rename | --help / -h | Print help | keep |
| lf wave update-plan | --wave / -w | wave | keep |
| lf wave update-plan | --plan | plan | keep |
| lf wave update-plan | --help / -h | Print help | keep |
| lf task | --help / -h | Print help | keep |
| lf task pr | --help / -h | Print help | keep |
| lf task pr checks | --watch / -w | watch | keep |
| lf task pr checks | --logs / -l | logs | keep |
| lf task pr checks | --help / -h | Print help | keep |
| lf task pr next | <slug> | Name the next serial branch (defaults to the settled PR's next slug, then the sequence number) | keep |
| lf task pr next | --help / -h | Print help | keep |
| lf task pr publish | --model / -m | model | keep |
| lf task pr publish | --title | title | keep |
| lf task pr publish | --body | body | keep |
| lf task pr publish | --help / -h | Print help | keep |
| lf task pr open | --model / -m | model | keep |
| lf task pr open | --title | title | keep |
| lf task pr open | --body | body | keep |
| lf task pr open | --help / -h | Print help | keep |
| lf task pr submit | --strict | strict | keep |
| lf task pr submit | --create-pr / -p | create pr | keep |
| lf task pr submit | --complete / -c | complete | keep |
| lf task pr submit | --next | next | keep |
| lf task pr submit | --worktree / -w | worktree | keep |
| lf task pr submit | --message / -m | message | keep |
| lf task pr submit | --title | title | keep |
| lf task pr submit | --body | body | keep |
| lf task pr submit | --help / -h | Print help | keep |
| lf task pr arm | --strict | strict | keep |
| lf task pr arm | --local | local | keep |
| lf task pr arm | --complete / -c | complete | keep |
| lf task pr arm | --next | next | keep |
| lf task pr arm | --worktree / -w | worktree | keep |
| lf task pr arm | --message / -m | message | keep |
| lf task pr arm | --title | title | keep |
| lf task pr arm | --body | body | keep |
| lf task pr arm | --help / -h | Print help | keep |
| lf task pr land | --strict | strict | keep |
| lf task pr land | --local | local | keep |
| lf task pr land | --complete / -c | complete | keep |
| lf task pr land | --next | next | keep |
| lf task pr land | --worktree / -w | worktree | keep |
| lf task pr land | --message / -m | message | keep |
| lf task pr land | --title | title | keep |
| lf task pr land | --body | body | keep |
| lf task pr land | --help / -h | Print help | keep |
| lf task pr abandon | <branch> | Branch to abandon (default: current) | keep |
| lf task pr abandon | --force / -f | force | keep |
| lf task pr abandon | --help / -h | Print help | keep |
| lf task wt | --help / -h | Print help | keep |
| lf task wt create | <name> | Worktree name | keep |
| lf task wt create | --plan | Print the placement plan without creating a worktree | keep |
| lf task wt create | --help / -h | Print help | keep |
| lf task wt switch | <name> | Worktree name or full branch name to switch to | keep |
| lf task wt switch | --help / -h | Print help | keep |
| lf task wt list | --json | json | keep |
| lf task wt list | --sync | Fetch origin and fast-forward main before listing (mutates the canonical checkout). Off by default so a list never touches it | keep |
| lf task wt list | --help / -h | Print help | keep |
| lf task wt prune | --dry-run | Show what would be pruned without removing anything | keep |
| lf task wt prune | --help / -h | Print help | keep |
| lf task wt delete | <name> | Worktree name to remove | keep |
| lf task wt delete | --force / -f | force | keep |
| lf task wt delete | --help / -h | Print help | keep |
| lf task sync | --plan | Print the planned sync strategy without mutating git | keep |
| lf task sync | --manual | Keep the sync local and leave conflicts for this process to resolve | keep |
| lf task sync | --continue | Stage resolved conflict paths and continue the local sync | keep |
| lf task sync | --abort | Abort the local sync in progress | keep |
| lf task sync | --adopt | Explicitly claim a raw sync that has no Loopflow owner | keep |
| lf task sync | <onto> | Branch to sync onto | keep |
| lf task sync | --help / -h | Print help | keep |
| lf task commit | --message / -m | message | keep |
| lf task commit | --no-add | no add | keep |
| lf task commit | --help / -h | Print help | keep |
| lf task __worker | <task_id> | task id | keep |
| lf task __worker | --help / -h | Print help | keep |
| lf task checkout | <issue> | issue | keep |
| lf task checkout | --name | name | keep |
| lf task checkout | --stack-on | Fork this Task's worktree from another Task's active PR | keep |
| lf task checkout | --directive | directive | keep |
| lf task checkout | --json | json | keep |
| lf task checkout | --help / -h | Print help | keep |
| lf task create | --wave | Wave name; defaults to the bound Wave | keep |
| lf task create | --title | Task title; omitted when stdin supplies the report and first line | keep |
| lf task create | --notes | Description; defaults to a report read from stdin | keep |
| lf task create | --run | Validate placement and execution before filing, then run the Task | keep |
| lf task create | --name | name | keep |
| lf task create | --flow | Select a Flow for this Task worker; defaults to the chapter recommendation | keep |
| lf task create | --stack-on | Fork this Task's worktree from another Task's active PR | keep |
| lf task create | --json | json | keep |
| lf task create | --help / -h | Print help | keep |
| lf task status | <issue> | Task issue; defaults to the Task in this checkout | keep |
| lf task status | --json | json | keep |
| lf task status | --help / -h | Print help | keep |
| lf task diff | <issue> | issue | keep |
| lf task diff | <path> | path | keep |
| lf task diff | --files | List changed paths and comparison revisions instead of a patch | keep |
| lf task diff | --base | base | keep |
| lf task diff | --draft | Compare a UTF-8 draft read from stdin without writing the worktree | keep |
| lf task diff | --json | json | keep |
| lf task diff | --help / -h | Print help | keep |
| lf task file | <issue> | issue | keep |
| lf task file | <path> | path | keep |
| lf task file | --recoveries | Inspect retained versions, including late writes; omitted for fast content reads | keep |
| lf task file | --json | json | keep |
| lf task file | --help / -h | Print help | keep |
| lf task save | <issue> | issue | keep |
| lf task save | <path> | path | keep |
| lf task save | --revision | revision | keep |
| lf task save | --json | json | keep |
| lf task save | --help / -h | Print help | keep |
| lf task complete | <issue> | issue | keep |
| lf task complete | --summary | summary | keep |
| lf task complete | --json | json | keep |
| lf task complete | --help / -h | Print help | keep |
| lf task abandon | <issue> | Issue ID or branch; defaults to the Task in this checkout | keep |
| lf task abandon | --force / -f | force | keep |
| lf task abandon | --json | json | keep |
| lf task abandon | --help / -h | Print help | keep |
| lf task sweep | --apply | apply | keep |
| lf task sweep | --json | json | keep |
| lf task sweep | --help / -h | Print help | keep |
| lf task delete | <issue> | issue | keep |
| lf task delete | --help / -h | Print help | keep |
| lf task edit | <issue> | issue | keep |
| lf task edit | --title | title | keep |
| lf task edit | --notes | notes | keep |
| lf task edit | --wave / -w | wave | keep |
| lf task edit | --help / -h | Print help | keep |
| lf task comment | <issue> | issue | keep |
| lf task comment | <message> | message | keep |
| lf task comment | --steer | Deliver new direction even when publishing from an agent Session | keep |
| lf task comment | --wave / -w | wave | keep |
| lf task comment | --json | json | keep |
| lf task comment | --help / -h | Print help | keep |
| lf task interrupt | <issue> | issue | keep |
| lf task interrupt | --json | json | keep |
| lf task interrupt | --help / -h | Print help | keep |
| lf task wait | <issue> | issue | keep |
| lf task wait | --until | until | keep |
| lf task wait | --timeout | timeout | keep |
| lf task wait | --json | json | keep |
| lf task wait | --help / -h | Print help | keep |
| lf task restart | <issue> | issue | keep |
| lf task restart | <advice> | advice | keep |
| lf task restart | --flow | Replacement Flow; validated before any checkpoint or stop | keep |
| lf task restart | --json | json | keep |
| lf task restart | --help / -h | Print help | keep |
| lf __telemetry-scorecard | --json | Emit structured JSON for operator automation | keep |
| lf __telemetry-scorecard | --help / -h | Print help | keep |
| lf list | <path> | path | keep |
| lf list | --json | json | keep |
| lf list | --help / -h | Print help | keep |
| lf help | <path> | path | keep |
| lf help | --all | all | keep |
| lf help | --help / -h | Print help | keep |
| lf roadmap | --wave | Scope to one Wave (default: every Wave in the current repository) | keep |
| lf roadmap | --task | Find an exact issue identifier, including retained historical Tasks | keep |
| lf roadmap | --json | Emit the roadmap snapshot as JSON | keep |
| lf roadmap | --all | Span every repository on this machine, not just the current one | keep |
| lf roadmap | --help / -h | Print help | keep |
| lf replay | <run> | Captured input identity or an unambiguous displayed prefix | keep |
| lf replay | --help / -h | Print help | keep |
| lf __flow-step | <id> | id | keep |
| lf __flow-step | <version> | version | keep |
| lf __flow-step | --help / -h | Print help | keep |
| lf run | <name> | name | keep |
| lf run | <args> | args | keep |
| lf run | --help / -h | Print help | keep |
| lf flow | --help / -h | Print help | keep |
| lf flow start | <template> | Template for a new Task Flow; existing saved progress remains authoritative | keep |
| lf flow start | --name | name | keep |
| lf flow start | --stack-on | Fork this Task's worktree from another Task's active PR | keep |
| lf flow start | --directive | directive | keep |
| lf flow start | --reason | Explain what changed after an execution blocker | keep |
| lf flow start | --retry | Retry uncertain native work after confirmed engine exit | keep |
| lf flow start | --json | json | keep |
| lf flow start | --help / -h | Print help | keep |
| lf flow list | --json | json | keep |
| lf flow list | --sessions | List or show saved FlowSessions instead of reusable templates | keep |
| lf flow list | --all | Include every repository and flows with unknown repository evidence | keep |
| lf flow list | --limit | FlowSession page size (default 100) | keep |
| lf flow list | --after | Previous page's next identity; retain the same filters | keep |
| lf flow list | --search | Literal name or identity containment | keep |
| lf flow list | --state | state | keep |
| lf flow list | --for-task | Retained Task ID or issue identifier, including completed Tasks | keep |
| lf flow list | --for-wave | Retained Wave ID or name | keep |
| lf flow list | --taskless | taskless | keep |
| lf flow list | --managed | Whether the Task currently selects this FlowSession | keep |
| lf flow list | --help / -h | Print help | keep |
| lf flow show | <name> | name | keep |
| lf flow show | --json | json | keep |
| lf flow show | --sessions | sessions | keep |
| lf flow show | --help / -h | Print help | keep |
| lf flow resume | <invocation> | invocation | keep |
| lf flow resume | --retry | retry | keep |
| lf flow resume | --help / -h | Print help | keep |
| lf skill | --help / -h | Print help | keep |
