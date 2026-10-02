# lf command reference

```bash
lf help --all
lf help task pr land
lf monitor --json
```

Generated from the compiled Clap tree. [The workflow guide](lf.md) uses
short forms; this reference names each canonical owner. Hidden commands
are internal process boundaries and are marked below.

## Selection and output

`--task` selects a Task checkout; `--wt` selects an existing worktree.
`--wave` adds context without moving directories and must match a Task's
owning Wave. Named direct Flows are independent contributions; `flow start`
selects the Task's saved managed Flow. Saved state, identity and feedback
survive continuation. `task restart` explicitly replaces that workflow.

A preference (`--account`) permits fallback. A restriction (`--only-account`)
limits this launch and its children. Saved Flows retain provider selections;
children check their destination's access before starting a provider.
Account observations distinguish unavailable, expired and measured capacity.

JSON readers emit one document; `monitor active --watch --json` emits NDJSON
until stdin closes. Diagnostics go to stderr. Exit 0 means the requested
operation succeeded; 1 denotes an operational failure, 2 a syntax or lookup
failure, and 130 interruption. A successful auto-merge request is not a merge.

## Flow decisions and recovery

```bash
lf --task EXP-12 flow start
lf flow resume FLOW_ID --retry
lf task restart EXP-12 --flow feature
```

Resume retains the captured graph. Retry retains its position. Restart
deliberately captures a replacement. Completing a review returns feedback;
the authored Flow decides what follows.

## lf

Open Loopflow or run its CLI

| Argument | What it does |
|---|---|
| `--docs` | Docs paths, globs, or directories to include in context |
| `--clipboard / -c` | Include clipboard content in prompt Default: false. |
| `--model / -m` | Model to use (harness or harness:model) |
| `--account` | Prefer this managed provider login before the normal route. Repeat to select provider-qualified preferences such as `claude=jack@`. Logins spend; a profile is only the Chrome venue accounts log in through, so it is never a run-time selector |
| `--only-account` | Restrict this invocation and its children to exactly these managed provider logins. Providers without a selection are unavailable |
| `--isolate` | Run in the selected account's own provider home, unmoved by account switches. Applies to this invocation and its children Default: false. |
| `--shared` | Run in the provider's ordinary home despite an `isolate: true` default Default: false. |
| `--__account-lease-probe` | Internal SSH compatibility and broker-connectivity probe Default: false. Internal. |
| `--yolo` | Skip permission prompts Default: false. |
| `--mode` | Choose the provider surface; omission inherits configuration and terminal context |
| `--chrome` | Override Chrome integration; omission inherits configuration |
| `--diff` | Select changed-code context; omission inherits configuration |
| `--max-turns` | Maximum agent turns for this invocation |
| `--wave` | Add Wave context and identity without changing the working directory |
| `--task` | Execute in this Task's checkout |
| `--wt` | Execute in an existing worktree by name or branch |
| `--__cwd` | Keep a Work-bound internal launch in this exact checkout Internal. |
| `--no-loopflow` | Exclude loopflow operating guidance Default: false. |
| `--__flow-step` | Execute a skill from this saved Flow boundary, without resolving its definition again Internal. |
| `--help / -h` | Print help |
| `--version / -V` | Print version |

## lf monitor

Show waiting, blocked, active, and finished work with next actions

| Argument | What it does |
|---|---|
| `--json` | json Default: false. |
| `--all` | all Default: false. |
| `--help / -h` | Print help |

## lf monitor list

List a bounded page of recorded commands, newest first

| Argument | What it does |
|---|---|
| `--json` | json Default: false. |
| `--all` | Include all repositories Default: false. |
| `--limit` | limit Default: 100. |
| `--after` | Continue with the previous page's next object, encoded as JSON |
| `--parent` | Direct children of an exact or unambiguous parent Exec |
| `--caller` | Commands issued by this AgentSession |
| `--search` | Literal command text, ignoring ASCII case |
| `--outcome` | outcome |
| `--task` | Recorded work for a Task, including completed Tasks |
| `--wave` | Recorded work for a Wave |
| `--help / -h` | Print help |

## lf monitor show

Inspect an Exec or Session by identity

| Argument | What it does |
|---|---|
| `<id>` | id |
| `--json` | json Default: false. |
| `--events` | Print a Session's retained raw input events Default: false. |
| `--final` | Print a Session's last recorded provider conclusion Default: false. |
| `--input` | Inspect an exact retained input belonging to this Session |
| `--context` | Print only what the step's submitted input was made of, by source Default: false. |
| `--help / -h` | Print help |

## lf monitor active

Observe active conversations and missing process evidence

| Argument | What it does |
|---|---|
| `--json` | json Default: false. |
| `--watch` | Stream NDJSON until stdin closes Default: false. |
| `--task` | task |
| `--help / -h` | Print help |

## lf monitor usage

Show direct provider-authored usage from recorded Session inputs

| Argument | What it does |
|---|---|
| `--json` | Emit Session usage evidence as JSON Default: false. |
| `--days` | Observation window, in days (zero means all time) Default: 30. |
| `--weekly` | Report context cost and turn time by week since 2026-09-30 Default: false. |
| `--binds` | Compare Task and Wave usage under prospective and post-hoc bind attribution Default: false. |
| `--parent` | Inputs issued by this Session or retained capture |
| `--wave` | Limit to Session inputs attributed to one Wave |
| `--project` | Limit to Session inputs attributed to one Project |
| `--task` | Limit to Session inputs attributed to one Task |
| `--context` | Break each step's submitted input down by source, flagged against budgets Default: false. |
| `--help / -h` | Print help |

## lf monitor ps

Print one parseable snapshot of live Loopflow call trees

| Argument | What it does |
|---|---|
| `--json` | Emit the versioned activity snapshot as JSON Default: false. |
| `--help / -h` | Print help |

## lf monitor top

Refresh live Loopflow call trees on a terminal; print once when redirected

| Argument | What it does |
|---|---|
| `--json` | Emit one versioned activity snapshot as JSON Default: false. |
| `--help / -h` | Print help |

## lf monitor prune

Reap registered orphan providers and remove dead process receipts

| Argument | What it does |
|---|---|
| `--dry-run` | Show exact targets without changing process or receipt state Default: false. |
| `--json` | Emit the versioned prune report as JSON Default: false. |
| `--help / -h` | Print help |

## lf monitor activity

Show one ordered record of durable Work, Session, PR, and Steer facts

| Argument | What it does |
|---|---|
| `--since` | Relative window (7d, 24h, 30m) or RFC3339 start Default: 7d. |
| `--limit` | Maximum rows after Work filters (1-200) Default: 50. |
| `--wave` | Scope to one Wave by name |
| `--project` | Scope to one Project by slug |
| `--task` | Scope to one Task by Linear identifier |
| `--json` | Emit the typed activity snapshot as JSON Default: false. |
| `--help / -h` | Print help |

## lf :

Run an inline prompt

| Argument | What it does |
|---|---|
| `<prompt>` | prompt |
| `--help / -h` | Print help |

## lf __screenshot-supervisor

Internal owner-loss supervisor for one browser capture

Internal command; invoked by the owning operation.

| Argument | What it does |
|---|---|
| `<source>` | URL or local HTML file to capture |
| `--output / -o` | PNG destination |
| `--width` | Viewport width in pixels Default: 1440. |
| `--height` | Viewport height in pixels Default: 900. |
| `--help / -h` | Print help |

## lf __provider-session

Internal provider callback that records one native interactive session

Internal command; invoked by the owning operation.

| Argument | What it does |
|---|---|
| `--help / -h` | Print help |

## lf session

Inspect and continue Sessions

| Argument | What it does |
|---|---|
| `--help / -h` | Print help |

## lf session history

Read this conversation's native start, usage and completion receipts

| Argument | What it does |
|---|---|
| `<id>` | id |
| `--json` | json Default: false. |
| `--after` | Continue after an observed event sequence Default: 0. |
| `--limit` | limit Default: 100. |
| `--help / -h` | Print help |

## lf session list

List Sessions

| Argument | What it does |
|---|---|
| `--json` | json Default: false. |
| `--all` | Include waiting steps from every repository on this machine Default: false. |
| `--interactive` | Select interactive (true), headless (false), or both (all) Default: true. |
| `--history` | Include completed conversations and historical reviews Default: false. |
| `--limit` | Maximum conversations; 0 reads the complete matching inventory Default: 100. |
| `--offset` | offset Default: 0. |
| `--page` | Return a bounded stable-ID page with a continuation cursor Default: false. |
| `--after` | Previous page's next identity; keep the same filters |
| `--task` | task |
| `--orphan` | Only Sessions without a Task association Default: false. |
| `--search` | search |
| `--help / -h` | Print help |

## lf session connect

Connect to the live conversation, or resume its saved history

| Argument | What it does |
|---|---|
| `<id>` | id |
| `--json` | json Default: false. |
| `--replace` | Stop Loopflow-owned clients before resuming here Default: false. |
| `--try` | Ask the provider to resume even when another client is active Default: false. |
| `--help / -h` | Print help |

## lf session ensure

Find or start the one ongoing conversation of this repository or a Wave

| Argument | What it does |
|---|---|
| `--wave / -w` | The Wave's conversation instead of the repository's |
| `--json` | json Default: false. |
| `--help / -h` | Print help |

## lf session replace

Give a primary Session's scope a fresh conversation

| Argument | What it does |
|---|---|
| `<id>` | id |
| `--json` | json Default: false. |
| `--help / -h` | Print help |

## lf session complete

Complete a review or interactive session

| Argument | What it does |
|---|---|
| `<id>` | id |
| `--help / -h` | Print help |

## lf session rename

Rename a Session; a human name is never replaced by a suggestion

| Argument | What it does |
|---|---|
| `<id>` | id |
| `<name>` | name |
| `--suggest` | Propose an agent-generated name; keeps a human-assigned name Default: false. |
| `--json` | json Default: false. |
| `--help / -h` | Print help |

## lf session bind

Assign a Task to a Session that has none; the Task never changes after

| Argument | What it does |
|---|---|
| `<id>` | id |
| `--task` | The Task, by its issue identifier (e.g. INF-123) or stable Task ID |
| `--dry-run` | Resolve the exact target without assigning the Session Default: false. |
| `--json` | json Default: false. |
| `--help / -h` | Print help |

## lf session ready

Mark the active session ready for your review

| Argument | What it does |
|---|---|
| `<summary>` | summary |
| `--help / -h` | Print help |

## lf session serve-flow

Run the exact review skill in its durable terminal

Internal command; invoked by the owning operation.

| Argument | What it does |
|---|---|
| `<task_id>` | task id |
| `<invocation_id>` | invocation id |
| `<flow>` | flow |
| `<node_id>` | node id |
| `<skill>` | skill |
| `<iteration>` | iteration |
| `--help / -h` | Print help |

## lf session serve-conversation

Run one prepared conversation in its durable terminal

Internal command; invoked by the owning operation.

| Argument | What it does |
|---|---|
| `<input>` | input |
| `--help / -h` | Print help |

## lf session stop-client

Stop one exact native provider client after its review completes

Internal command; invoked by the owning operation.

| Argument | What it does |
|---|---|
| `<input>` | input |
| `--help / -h` | Print help |

## lf account

Refresh account access and capacity, or manage logins and routing

| Argument | What it does |
|---|---|
| `<provider>` | Limit observations to one provider |
| `--cached` | Inspect cached evidence without contacting providers or the origin broker Default: false. |
| `--details` | Include credential sources, browser choices, and timestamps Default: false. |
| `--json` | Emit the account overview as one JSON document Default: false. |
| `--help / -h` | Print help |

## lf account disconnect

Disconnect local credentials or one managed login

| Argument | What it does |
|---|---|
| `<provider>` | provider |
| `<email>` | email |
| `--help / -h` | Print help |

## lf account connect

Connect local credentials or a managed login using a remembered browser

| Argument | What it does |
|---|---|
| `<provider>` | provider |
| `<email>` | email |
| `--chrome-profile` | chrome profile |
| `--import` | Adopt an existing Claude login Default: false. |
| `--api-key` | Read the provider's API key environment variable Default: false. |
| `--help / -h` | Print help |

## lf account set

Edit account configuration or remembered browser choices

| Argument | What it does |
|---|---|
| `<provider>` | provider |
| `<email>` | email |
| `--login-email` | login email |
| `--routing` | routing |
| `--plan` | plan |
| `--clear-plan` | clear plan Default: false. |
| `--paid-through` | paid through |
| `--clear-paid-through` | clear paid through Default: false. |
| `--clear-cooldown` | clear cooldown Default: false. |
| `--chrome-profile` | Replace the ordered browser choices (repeat for fallback profiles) |
| `--clear-chrome-profiles` | clear chrome profiles Default: false. |
| `--help / -h` | Print help |

## lf account redeem-reset

Spend one banked Codex reset for this named login

| Argument | What it does |
|---|---|
| `<provider>` | provider |
| `<email>` | email |
| `--idempotency-key` | Reuse this key when retrying the same redemption |
| `--credit-id` | Opaque credit ID returned by live status (otherwise the service chooses) |
| `--json` | json Default: false. |
| `--help / -h` | Print help |

## lf account use

Sign the provider's ordinary home in as a stored login, from now on

| Argument | What it does |
|---|---|
| `<provider>` | provider |
| `<email>` | email |
| `--help / -h` | Print help |

## lf account route

Explain configured and automatic account selection, or replace a route

| Argument | What it does |
|---|---|
| `--repo` | repo |
| `--default` | default Default: false. |
| `--json` | json Default: false. |
| `--help / -h` | Print help |

## lf account route set

Replace a provider's ordered route

| Argument | What it does |
|---|---|
| `<provider>` | provider |
| `<accounts>` | accounts |
| `--repo` | repo |
| `--default` | default Default: false. |
| `--help / -h` | Print help |

## lf repo

Repository releases, source measurement, CI evidence, and provider administration

| Argument | What it does |
|---|---|
| `--help / -h` | Print help |

## lf repo connect

Connect a Wave to its Initiative and the repository's Team (Task prefix)

| Argument | What it does |
|---|---|
| `<wave>` | Wave name (auto-detected if omitted) |
| `--all` | Recursively initialize every Wave under wave/ Default: false. |
| `--team-key` | Repository Team key = Task prefix (e.g. LOO). Defaults from the repository name |
| `--team-name` | Repository Team display name. Defaults to the repository name |
| `--help / -h` | Print help |

## lf repo refresh

Refresh shared planning from Linear

| Argument | What it does |
|---|---|
| `<wave>` | wave |
| `--all` | all Default: false. |
| `--help / -h` | Print help |

## lf repo new-chapter

Advance every Wave to the named Project plan

| Argument | What it does |
|---|---|
| `<name>` | name |
| `--dry-run` | dry run Default: false. |
| `--json` | json Default: false. |
| `--help / -h` | Print help |

## lf repo release

Release operations (run, check, notes, bump, tag, status)

| Argument | What it does |
|---|---|
| `--help / -h` | Print help |

## lf repo release run

Run the full release workflow end-to-end

| Argument | What it does |
|---|---|
| `<version>` | Version to release: patch\|minor\|major\|X.Y.Z (default: patch) |
| `--target / -t` | target |
| `--help / -h` | Print help |

## lf repo release check

Check if PRs have merged since the last tag

| Argument | What it does |
|---|---|
| `--target / -t` | target |
| `--help / -h` | Print help |

## lf repo release notes

Generate release notes for a version

| Argument | What it does |
|---|---|
| `<version>` | Version (e.g. 0.9.6) |
| `--prev-tag` | prev tag |
| `--preview` | Print notes without updating manifests or release archives Default: false. |
| `--target / -t` | target |
| `--help / -h` | Print help |

## lf repo release bump

Bump version in manifest files

| Argument | What it does |
|---|---|
| `<version>` | Version to bump to (e.g. 0.9.6) |
| `--target / -t` | target |
| `--help / -h` | Print help |

## lf repo release tag

Create a git tag and push it

| Argument | What it does |
|---|---|
| `<version>` | Version to tag (e.g. 0.9.6) |
| `--target / -t` | target |
| `--help / -h` | Print help |

## lf repo release publish

Stage or publish a GitHub Release

| Argument | What it does |
|---|---|
| `<tag>` | Release tag (for example v0.12.4) |
| `--notes` | Release notes used while creating or updating the draft |
| `--asset` | Asset to upload; repeat for multiple files |
| `--finalize` | Publish the existing draft and mark it latest Default: false. |
| `--help / -h` | Print help |

## lf repo release status

Check release workflow status

| Argument | What it does |
|---|---|
| `--target / -t` | target |
| `--help / -h` | Print help |

## lf repo tokens

Measure this codebase: lines and tokens per directory (tracked files only)

| Argument | What it does |
|---|---|
| `--json` | Emit as JSON Default: false. |
| `--days` | Walk git history instead: the codebase's size on each day it changed |
| `--help / -h` | Print help |

## lf repo ci

Show how failed CI is detected, repaired, and landed across this Home

| Argument | What it does |
|---|---|
| `--since` | Relative window (7d, 24h, 30m) or RFC3339 start Default: 7d. |
| `--wave` | Scope to one Wave |
| `--repo` | Scope to one GitHub owner/repo |
| `--json` | Emit the complete incident report as JSON Default: false. |
| `--help / -h` | Print help |

## lf repo ci watch

Watch this repository's PR checks and start a ci-fix when a recorded landing fails

| Argument | What it does |
|---|---|
| `--once` | Check every open PR once and exit Default: false. |
| `--install` | Keep the watcher running in the background as a launchd service Default: false. |
| `--uninstall` | Remove the background service Default: false. |
| `--status` | Show whether a watcher is live, its last poll, and what it started Default: false. |
| `--json` | Emit the status as JSON Default: false. |
| `--parent-pid` | Stop when this process exits Internal. |
| `--help / -h` | Print help |

## lf repo reteam

Reconcile linked Waves to the repository's Linear Team

| Argument | What it does |
|---|---|
| `--apply` | apply Default: false. |
| `--help / -h` | Print help |

## lf home

Inspect this Home and observe routes to other Homes

| Argument | What it does |
|---|---|
| `--help / -h` | Print help |

## lf home desktop

Open or focus Loopflow.app

| Argument | What it does |
|---|---|
| `--help / -h` | Print help |

## lf home screenshot

Capture a URL or local HTML file without claiming the user's browser

| Argument | What it does |
|---|---|
| `<source>` | URL or local HTML file to capture |
| `--output / -o` | PNG destination |
| `--width` | Viewport width in pixels Default: 1440. |
| `--height` | Viewport height in pixels Default: 900. |
| `--help / -h` | Print help |

## lf home install

Install the latest published Loopflow release from any directory

| Argument | What it does |
|---|---|
| `--help / -h` | Print help |

## lf home install schedule

Install the latest Loopflow at login and weekly by default (macOS launchd)

| Argument | What it does |
|---|---|
| `<frequency>` | Weekly: Monday 09:00; daily: 09:00; otherwise on clock boundaries (local time) Default: weekly. |
| `--help / -h` | Print help |

## lf home install recover-switch

Continue one interrupted machine install switch from its pinned candidate

Internal command; invoked by the owning operation.

| Argument | What it does |
|---|---|
| `--switch` | The fixed machine switch receipt to continue |
| `--help / -h` | Print help |

## lf home install preflight

Preview whether this build may replace the global lf (read-only). Reads the shared store's migration frontier and validates executable planning references against this binary without changing that frontier. Exits non-zero on refusal so a caller can gate on it

Internal command; invoked by the owning operation.

| Argument | What it does |
|---|---|
| `--json` | Emit the structured PromotionPreview as JSON Default: false. |
| `--help / -h` | Print help |

## lf home install advance-switch

Advance the receipt-selected store with this exact candidate's registry

Internal command; invoked by the owning operation.

| Argument | What it does |
|---|---|
| `--switch` | switch |
| `--help / -h` | Print help |

## lf home install promote

Promote this build to the global CLI: content-address it into ~/.lf/bin and atomically repoint the target symlink, under the exclusive promotion lock. Refuses — leaving every target unchanged — on incompatible schema or persisted executable evidence

Internal command; invoked by the owning operation.

| Argument | What it does |
|---|---|
| `--cli-target` | The global CLI symlink to replace (e.g. ~/.local/bin/lf) |
| `--app-source` | A staged Loopflow.app bundle to install alongside the CLI |
| `--app-target` | The global Loopflow.app path to replace atomically |
| `--legacy-app-target` | A retired app bundle to remove after the new app commits |
| `--sync-skills` | Regenerate global skills after the promotion commits Default: false. |
| `--preview` | Validate and print the preview but change nothing Default: false. |
| `--help / -h` | Print help |

## lf home install rollback

Repoint the global CLI at retained prior bytes only after that binary's own preflight proves it recognizes the current store frontier

Internal command; invoked by the owning operation.

| Argument | What it does |
|---|---|
| `--cli-target` | The global CLI symlink to replace (e.g. ~/.local/bin/lf) |
| `--candidate` | The immutable content-addressed prior executable to activate |
| `--help / -h` | Print help |

## lf home sync-skills

Compile loopflow skills into your home vendor Skills directories

Internal command; invoked by the owning operation.

| Argument | What it does |
|---|---|
| `--yes / -y` | Confirm writes under ~/ without prompting Default: false. |
| `--no-prune` | Keep stale loopflow-generated skills Default: false. |
| `--help / -h` | Print help |

## lf home doctor

Diagnose installation, storage, Exec integrity and scheduled receipts

| Argument | What it does |
|---|---|
| `--planning` | Diagnose repository planning without changing it Default: false. |
| `--json` | Emit the audit as JSON Default: false. |
| `--help / -h` | Print help |

## lf home ssh

Run lf on a Home or SSH host carrying your local credentials

| Argument | What it does |
|---|---|
| `--account` | Prefer this origin account when the remote lf chooses a provider |
| `--only-account` | Restrict remote provider launches to these origin accounts |
| `<target>` | HomeId (preferred), SSH alias, or user@host |
| `--repo` | Repository path on the remote, relative to $HOME |
| `--secret` | Doppler secret to resolve locally and forward as an env var (repeatable). The Doppler token itself is never forwarded |
| `--forward-agent` | Forward the ssh-agent (`ssh -A`). Off by default: git pushes use the forwarded GH_TOKEN over HTTPS, so agent forwarding is unneeded risk Default: false. |
| `<lf_args>` | Arguments for the remote lf. The target is the boundary: every argument after it belongs to the remote invocation |
| `--help / -h` | Print help (see more with '--help') |

## lf home user

Print the configured participant display name

| Argument | What it does |
|---|---|
| `--json` | json Default: false. |
| `--help / -h` | Print help |

## lf home id

Print this machine's stable local Home identity

| Argument | What it does |
|---|---|
| `--json` | json Default: false. |
| `--help / -h` | Print help |

## lf home observe

Record the current route for a known Home identity

| Argument | What it does |
|---|---|
| `<home_id>` | home id |
| `<route>` | route |
| `--json` | json Default: false. |
| `--help / -h` | Print help |

## lf discord

Bridge new Discord messages to finite Wave Sessions

| Argument | What it does |
|---|---|
| `--help / -h` | Print help |

## lf discord serve

Poll a configured channel and post each Session's final answer

| Argument | What it does |
|---|---|
| `<wave>` | wave |
| `--help / -h` | Print help |

## lf wave

Manage Wave identity, placement and planning

| Argument | What it does |
|---|---|
| `--help / -h` | Print help |

## lf wave cron

Local launchd jobs that run lf commands on a schedule

| Argument | What it does |
|---|---|
| `--help / -h` | Print help |

## lf wave cron add

Install or replace a scheduled lf invocation

| Argument | What it does |
|---|---|
| `--wave / -w` | Wave name passed to `lf <flow> --wave <wave>` (ambient if omitted) |
| `--flow` | Flow or skill name to run |
| `--schedule` | Daily or every-minute cron expression, or a schedule alias Default: daily. |
| `--help / -h` | Print help |

## lf wave cron list

List installed loopflow cron jobs

| Argument | What it does |
|---|---|
| `--wave / -w` | Only jobs for this Wave |
| `--json` | Emit machine-readable job state Default: false. |
| `--help / -h` | Print help |

## lf wave cron preflight

Validate Home authority and declared jobs without changing launchd

| Argument | What it does |
|---|---|
| `--wave / -w` | Wave whose GOAL.md `crons:` are validated |
| `--help / -h` | Print help |

## lf wave cron sync

Reconcile installed launchd jobs to match a wave's declared `crons:`

| Argument | What it does |
|---|---|
| `--wave / -w` | wave |
| `--repo` | Install the finite repository Task check on this Home Default: false. |
| `--disable` | Remove the repository schedule; running work retains its authority Default: false. |
| `--help / -h` | Print help |

## lf wave cron run

Execute one installed cron job and persist its terminal receipt

Internal command; invoked by the owning operation.

| Argument | What it does |
|---|---|
| `--wave / -w` | Wave whose installed declaration is executed |
| `--flow` | Flow or skill name to run |
| `--scheduled` | Mark a launchd-owned invocation Default: false. Internal. |
| `--help / -h` | Print help |

## lf wave cron history

Show durable cron receipts

| Argument | What it does |
|---|---|
| `--wave / -w` | Wave whose receipts are shown |
| `--flow` | Only receipts for this flow or skill |
| `--days` | Receipt window in days Default: 35. |
| `--json` | Emit machine-readable receipts Default: false. |
| `--help / -h` | Print help |

## lf wave cron trigger

Ask launchd to fire an installed job

| Argument | What it does |
|---|---|
| `--wave / -w` | Wave whose installed job is fired |
| `--flow` | Flow or skill name to run |
| `--wait` | Wait for and return the scheduled receipt Default: false. |
| `--timeout` | Maximum wait for a receipt Default: 15m. |
| `--help / -h` | Print help |

## lf wave cron remove

Uninstall a scheduled lf invocation

| Argument | What it does |
|---|---|
| `--wave / -w` | Wave name passed to `lf <flow> --wave <wave>` |
| `--flow` | Flow or skill name to remove |
| `--help / -h` | Print help |

## lf wave list

List authored Waves and retained planning identities without starting work

| Argument | What it does |
|---|---|
| `--json` | Emit the wave snapshot as JSON (Loopflow's dashboard snapshot) Default: false. |
| `--all` | List Waves from every repository on this machine, not just the current repository (worktrees collapse to their main checkout) Default: false. |
| `--current` | Exclude abandoned and retired registrations from current navigation Default: false. |
| `--help / -h` | Print help |

## lf wave status

Show one Wave's current plan, Task details, and execution evidence

| Argument | What it does |
|---|---|
| `<wave>` | Wave name (default: the ambient wave) |
| `--json` | Emit the status snapshot as JSON Default: false. |
| `--sync` | Refresh planning from Linear before reading Default: false. |
| `--help / -h` | Print help |

## lf wave place

Set the Home for Wave schedules and newly created work

| Argument | What it does |
|---|---|
| `<name>` | name |
| `<home_id>` | home id |
| `--json` | json Default: false. |
| `--help / -h` | Print help |

## lf wave rename

Rename or relocate an authored Wave and its provider mapping

| Argument | What it does |
|---|---|
| `<wave>` | wave |
| `--repo` | repo |
| `--name` | name |
| `--title` | Change the linked Initiative display title |
| `--json` | json Default: false. |
| `--help / -h` | Print help |

## lf wave update-plan

Replace the current chapter's KRs, targets, and Flow recommendation

| Argument | What it does |
|---|---|
| `--wave / -w` | wave |
| `--plan` | plan |
| `--help / -h` | Print help |

## lf task

Concrete work, worktrees, commits, and pull requests

| Argument | What it does |
|---|---|
| `--help / -h` | Print help |

## lf task automation

Inspect repository scheduling and Task enrollment

| Argument | What it does |
|---|---|
| `--json` | json Default: false. |
| `--help / -h` | Print help |

## lf task reconcile

Check enrolled Tasks and authorized deliveries once, then exit

| Argument | What it does |
|---|---|
| `--json` | json Default: false. |
| `--help / -h` | Print help |

## lf task automate

Enroll or hold a Task without interrupting running work

| Argument | What it does |
|---|---|
| `<issue>` | issue |
| `<state>` | state |
| `--help / -h` | Print help |

## lf task __repair

Run a reserved CI repair in its own process

Internal command; invoked by the owning operation.

| Argument | What it does |
|---|---|
| `<incident>` | incident |
| `<launcher>` | launcher |
| `--help / -h` | Print help |

## lf task pr

Pull request lifecycle

| Argument | What it does |
|---|---|
| `--help / -h` | Print help |

## lf task pr reconcile

Check recorded repository landings once, record CI failures, and settle verified merges

| Argument | What it does |
|---|---|
| `--help / -h` | Print help |

## lf task pr checks

Show CI status for current branch

| Argument | What it does |
|---|---|
| `--watch / -w` | watch Default: false. |
| `--logs / -l` | logs Default: false. |
| `--help / -h` | Print help |

## lf task pr next

After an out-of-band merge, rotate this Task to its next serial PR, carrying committed and uncommitted follow-up onto the new branch

| Argument | What it does |
|---|---|
| `<slug>` | Name the next serial branch (defaults to the settled PR's next slug, then the sequence number) |
| `--help / -h` | Print help |

## lf task pr publish

Publish a ready PR headlessly: push, create or refresh, print state + URL. Opens no review surface

| Argument | What it does |
|---|---|
| `--model / -m` | model |
| `--title` | title |
| `--body` | body |
| `--help / -h` | Print help |

## lf task pr open

Push and create or update a draft PR, then open its GitHub page. Existing ready PRs stay ready; opening a draft does not publish it

| Argument | What it does |
|---|---|
| `--model / -m` | model |
| `--title` | title |
| `--body` | body |
| `--help / -h` | Print help |

## lf task pr submit

Prepare a PR to land: sync, clear scratch, mark ready, and assign it to you. Nothing merges until you click merge on GitHub

| Argument | What it does |
|---|---|
| `--strict` | strict Default: false. |
| `--create-pr / -p` | create pr Default: false. |
| `--complete / -c` | complete Default: false. |
| `--next` | next |
| `--worktree / -w` | worktree |
| `--message / -m` | message |
| `--title` | title |
| `--body` | body |
| `--help / -h` | Print help |

## lf task pr arm

Prepare a PR, request exact-head auto-merge, and return without watching

| Argument | What it does |
|---|---|
| `--strict` | strict Default: false. |
| `--local` | local Default: false. |
| `--complete / -c` | complete Default: false. |
| `--next` | next |
| `--worktree / -w` | worktree |
| `--message / -m` | message |
| `--title` | title |
| `--body` | body |
| `--help / -h` | Print help |

## lf task pr land

Request auto-merge, retain settlement intent, and return

| Argument | What it does |
|---|---|
| `--strict` | strict Default: false. |
| `--local` | local Default: false. |
| `--complete / -c` | complete Default: false. |
| `--next` | next |
| `--worktree / -w` | worktree |
| `--message / -m` | message |
| `--title` | title |
| `--body` | body |
| `--help / -h` | Print help |

## lf task pr abandon

Abandon branch: close PR, remove worktree, delete branch

| Argument | What it does |
|---|---|
| `<branch>` | Branch to abandon (default: current) |
| `--force / -f` | force Default: false. |
| `--help / -h` | Print help |

## lf task wt

Worktree operations

| Argument | What it does |
|---|---|
| `--help / -h` | Print help |

## lf task wt create

Create a low-level sibling worktree

| Argument | What it does |
|---|---|
| `<name>` | Worktree name |
| `--plan` | Print the placement plan without creating a worktree Default: false. |
| `--help / -h` | Print help |

## lf task wt switch

Switch to a worktree by name, identity leaf, or full branch

| Argument | What it does |
|---|---|
| `<name>` | Worktree name or full branch name to switch to |
| `--help / -h` | Print help |

## lf task wt list

List worktrees (read-only; reflects the last-synced main)

| Argument | What it does |
|---|---|
| `--json` | json Default: false. |
| `--sync` | Fetch origin and fast-forward main before listing (mutates the canonical checkout). Off by default so a list never touches it Default: false. |
| `--help / -h` | Print help |

## lf task wt prune

Remove clean terminal or inactive worktrees

| Argument | What it does |
|---|---|
| `--dry-run` | Show what would be pruned without removing anything Default: false. |
| `--help / -h` | Print help |

## lf task wt delete

Delete a worktree and its local and remote branch; retain PR and Task outcomes

| Argument | What it does |
|---|---|
| `<name>` | Worktree name to remove |
| `--force / -f` | force Default: false. |
| `--help / -h` | Print help |

## lf task sync

Merge upstream into the current branch (default: main or stack parent)

| Argument | What it does |
|---|---|
| `--plan` | Print the planned sync strategy without mutating git Default: false. |
| `--manual` | Keep the sync local and leave conflicts for this process to resolve Default: false. |
| `--continue` | Stage resolved conflict paths and continue the local sync Default: false. |
| `--abort` | Abort the local sync in progress Default: false. |
| `--adopt` | Explicitly claim a raw sync that has no Loopflow owner Default: false. |
| `<onto>` | Branch to sync onto |
| `--help / -h` | Print help |

## lf task commit

Commit changes

| Argument | What it does |
|---|---|
| `--message / -m` | message |
| `--no-add` | no add Default: false. |
| `--help / -h` | Print help |

## lf task __worker

Internal: drive a Task Flow from its claimed boundary

Internal command; invoked by the owning operation.

| Argument | What it does |
|---|---|
| `<task_id>` | task id |
| `--help / -h` | Print help |

## lf task checkout

Ensure tracked Task Work and its worktree without starting a worker

| Argument | What it does |
|---|---|
| `<issue>` | issue |
| `--name` | name |
| `--stack-on` | Fork this Task's worktree from another Task's active PR |
| `--directive` | directive |
| `--json` | json Default: false. |
| `--help / -h` | Print help |

## lf task create

File a Task in the current chapter; optionally prepare and run it

| Argument | What it does |
|---|---|
| `--wave` | Wave name; defaults to the bound Wave |
| `--title` | Task title; omitted when stdin supplies the report and first line |
| `--notes` | Description; defaults to a report read from stdin |
| `--run` | Validate placement and execution before filing, then run the Task Default: false. |
| `--name` | name |
| `--flow` | Select a Flow for this Task worker; defaults to the chapter recommendation |
| `--stack-on` | Fork this Task's worktree from another Task's active PR |
| `--json` | json Default: false. |
| `--help / -h` | Print help |

## lf task status

Show durable Task facts and current worker evidence

| Argument | What it does |
|---|---|
| `<issue>` | Task issue; defaults to the Task in this checkout |
| `--json` | json Default: false. |
| `--help / -h` | Print help |

## lf task diff

Show this Task's patch or list its changed files

| Argument | What it does |
|---|---|
| `<issue>` | issue |
| `<path>` | path |
| `--files` | List changed paths and comparison revisions instead of a patch Default: false. |
| `--base` | base Default: parent. |
| `--draft` | Compare a UTF-8 draft read from stdin without writing the worktree Default: false. |
| `--json` | json Default: false. |
| `--help / -h` | Print help |

## lf task file

Read one file from this Task's worktree

| Argument | What it does |
|---|---|
| `<issue>` | issue |
| `<path>` | path |
| `--recoveries` | Inspect retained versions, including late writes; omitted for fast content reads Default: false. |
| `--json` | json Default: false. |
| `--help / -h` | Print help |

## lf task save

Save UTF-8 stdin with an expected revision and retained recovery files

| Argument | What it does |
|---|---|
| `<issue>` | issue |
| `<path>` | path |
| `--revision` | revision |
| `--json` | json Default: false. |
| `--help / -h` | Print help |

## lf task complete

Complete planning work, or a placed Task whose pull requests are settled

| Argument | What it does |
|---|---|
| `<issue>` | issue |
| `--summary` | summary |
| `--json` | json Default: false. |
| `--help / -h` | Print help |

## lf task abandon

Cancel the Task in Linear and locally, close its PRs and delete its branches

| Argument | What it does |
|---|---|
| `<issue>` | Issue ID or branch; defaults to the Task in this checkout |
| `--force / -f` | force Default: false. |
| `--json` | json Default: false. |
| `--help / -h` | Print help |

## lf task sweep

Preview open issues outside current chapters; apply safe cancellations explicitly

| Argument | What it does |
|---|---|
| `--apply` | apply Default: false. |
| `--json` | json Default: false. |
| `--help / -h` | Print help |

## lf task delete

Cancel unfinished placed work, clean up delivery, then trash the Linear issue

| Argument | What it does |
|---|---|
| `<issue>` | issue |
| `--help / -h` | Print help |

## lf task edit

Edit a Task's title or notes, before or after placement

| Argument | What it does |
|---|---|
| `<issue>` | issue |
| `--title` | title |
| `--notes` | notes |
| `--wave / -w` | wave |
| `--help / -h` | Print help |

## lf task comment

Read the thread or publish a comment; agent comments default to progress

| Argument | What it does |
|---|---|
| `<issue>` | issue |
| `<message>` | message |
| `--steer` | Deliver new direction even when publishing from an agent Session Default: false. |
| `--wave / -w` | wave |
| `--json` | json Default: false. |
| `--help / -h` | Print help |

## lf task interrupt

Interrupt the active provider turn

| Argument | What it does |
|---|---|
| `<issue>` | issue |
| `--json` | json Default: false. |
| `--help / -h` | Print help |

## lf task wait

Wait without polling an LM

| Argument | What it does |
|---|---|
| `<issue>` | issue |
| `--until` | until Default: terminal. |
| `--timeout` | timeout |
| `--json` | json Default: false. |
| `--help / -h` | Print help |

## lf task restart

Stop the pinned Flow and begin a new one in a fresh Task worker; defaults to the chapter's currently recommended Flow

| Argument | What it does |
|---|---|
| `<issue>` | issue |
| `<advice>` | advice |
| `--flow` | Replacement Flow; validated before any checkpoint or stop |
| `--json` | json Default: false. |
| `--help / -h` | Print help |

## lf context

Show effective context budgets, their sources, and current source usage

| Argument | What it does |
|---|---|
| `--json` | json Default: false. |
| `--wave` | Inspect a Wave's local authored context |
| `--task` | Inspect a Task's checkout and locally stored goal |
| `--skill` | Skill to include in the launch preview Default: realign. |
| `--help / -h` | Print help |

## lf __telemetry-scorecard

Internal: render the repository maintainer scorecard for telemetry-daily

Internal command; invoked by the owning operation.

| Argument | What it does |
|---|---|
| `--json` | Emit structured JSON for operator automation Default: false. |
| `--help / -h` | Print help |

## lf list

Discover commands, skills, and flows

| Argument | What it does |
|---|---|
| `<path>` | path |
| `--json` | json Default: false. |
| `--help / -h` | Print help |

## lf help

Explain a command, skill, or flow without launching it

| Argument | What it does |
|---|---|
| `<path>` | path |
| `--all` | all Default: false. |
| `--help / -h` | Print help |

## lf roadmap

Show the current repository's roadmap: every open Task across the repo's Waves, joined to live evidence and bucketed into Now / Waiting / Available / Later. `--wave` scopes it; `--all` spans every repository on this machine. Local-only, deterministic

| Argument | What it does |
|---|---|
| `--wave` | Scope to one Wave (default: every Wave in the current repository) |
| `--task` | Find an exact issue identifier, including retained historical Tasks |
| `--json` | Emit the roadmap snapshot as JSON Default: false. |
| `--all` | Span every repository on this machine, not just the current one Default: false. |
| `--help / -h` | Print help |

## lf replay

Launch the immutable provider request retained for a captured input

| Argument | What it does |
|---|---|
| `<run>` | Captured input identity or an unambiguous displayed prefix |
| `--help / -h` | Print help |

## lf __flow-step

Execute one captured Flow boundary in its own process

Internal command; invoked by the owning operation.

| Argument | What it does |
|---|---|
| `<id>` | id |
| `<version>` | version |
| `--help / -h` | Print help |

## lf run

Run a definition, preferring a flow over a same-named skill

| Argument | What it does |
|---|---|
| `<name>` | name |
| `<args>` | args |
| `--help / -h` | Print help |

## lf flow

Run or inspect authored flows

| Argument | What it does |
|---|---|
| `--help / -h` | Print help |

## lf flow start

Start or continue a Task through its saved Flow

| Argument | What it does |
|---|---|
| `<template>` | Template for a new Task Flow; existing saved progress remains authoritative |
| `--name` | name |
| `--stack-on` | Fork this Task's worktree from another Task's active PR |
| `--directive` | directive |
| `--reason` | Explain what changed after an execution blocker |
| `--retry` | Retry uncertain native work after confirmed engine exit Default: false. |
| `--json` | json Default: false. |
| `--help / -h` | Print help |

## lf flow list

List authored flows or saved FlowSessions

| Argument | What it does |
|---|---|
| `--json` | json Default: false. |
| `--sessions` | List or show saved FlowSessions instead of reusable templates Default: false. |
| `--all` | Include every repository and flows with unknown repository evidence Default: false. |
| `--limit` | FlowSession page size (default 100) |
| `--after` | Previous page's next identity; retain the same filters |
| `--search` | Literal name or identity containment |
| `--state` | state |
| `--for-task` | Retained Task ID or issue identifier, including completed Tasks |
| `--for-wave` | Retained Wave ID or name |
| `--taskless` | taskless Default: false. |
| `--managed` | Whether the Task currently selects this FlowSession |
| `--help / -h` | Print help |

## lf flow show

Inspect an authored flow or a saved FlowSession

| Argument | What it does |
|---|---|
| `<name>` | name |
| `--json` | json Default: false. |
| `--sessions` | sessions Default: false. |
| `--help / -h` | Print help |

## lf flow resume

Continue a saved Flow invocation

| Argument | What it does |
|---|---|
| `<invocation>` | invocation |
| `--retry` | retry Default: false. |
| `--help / -h` | Print help |

## lf skill

Run a skill explicitly

| Argument | What it does |
|---|---|
| `--help / -h` | Print help |
