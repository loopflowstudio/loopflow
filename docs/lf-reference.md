# lf command reference

```bash
lf help --all
lf help pr land
lf monitor --json
```

Generated from the compiled Clap tree. Canonical command paths are shown; unique shortcuts also resolve. See
[the workflow guide](lf.md) for examples. Hidden commands
are internal process boundaries and are marked below.

## Selection and output

`--machine <label-or-id>` runs the command in the saved remote repository.
`--secret` and `--forward-agent` require `--machine`.
`--task` selects a Task checkout; `--wt` selects an existing worktree.
`--wave` adds context without moving directories and must match a Task's
owning Wave. `task run` places a Task's worktree and then runs like
`lf --task ISSUE run FLOW`; every Flow naming a Task is equally its work.

A preference (`--account`) permits fallback. A restriction (`--only-account`)
limits this launch and its children. A Flow retains its provider selections;
children check their destination's access before starting a provider.
Account observations distinguish unavailable, expired and measured capacity.

JSON readers emit one document; `monitor active --watch --json` emits NDJSON
until stdin closes. Diagnostics go to stderr. Exit 0 means the requested
operation succeeded; 1 denotes an operational failure, 2 a syntax or lookup
failure, and 130 interruption. A successful auto-merge request is not a merge.

## Flow decisions and recovery

```bash
lf task run EXP-12 pursue
lf flow show FLOW_ID --processes --json
lf task interrupt EXP-12
```

Each start is a new Flow: one lf process and the step processes it starts. Its ID
is the Flow process's. `--state` selects `current` (the driver has no recorded
exit), `completed` (it succeeded) or `stopped` (it exited before the last
step). A stopped Flow's Processes are its history; nothing resumes it. Inspect
them, then launch the work that remains.

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
| `--interactive / -i` | Run interactively Default: false. |
| `--batch / -b` | Run headless: print the output and return when the work ends Default: false. |
| `--tui` | Hand off Claude, Codex, or OpenCode to the terminal (overrides session.launch) Default: false. |
| `--ide` | Hand off Claude or Codex to the vendor app (overrides session.launch) Default: false. |
| `--chrome` | Override Chrome integration; omission inherits configuration |
| `--__cron-receipt` | Exact cron receipt attribution for mechanical release execution Internal. |
| `--__cron-lock-fd` | cron lock fd Internal. |
| `--diff` | Select changed-code context; omission inherits configuration |
| `--max-turns` | Maximum agent turns for this invocation |
| `--machine` | Run the command on this saved machine in its repository |
| `--secret` | Resolve a named Doppler secret locally and forward its value (repeatable) |
| `--forward-agent` | Forward the SSH agent to the selected machine Default: false. |
| `--wave` | Add Wave context and identity without changing the working directory |
| `--task` | Execute in this Task's checkout |
| `--steers-after` | Give the agent only Task direction newer than this steer |
| `--wt` | Execute in an existing worktree by name or branch |
| `--__cwd` | Keep a Work-bound internal launch in this exact checkout Internal. |
| `--no-loopflow` | Exclude loopflow operating guidance Default: false. |
| `--help / -h` | Print help |
| `--version / -V` | Print version |

## lf pr

Pull request lifecycle

| Argument | What it does |
|---|---|
| `--help / -h` | Print help |

## lf pr reconcile

Check recorded repository landings once, record CI failures, and settle verified merges

| Argument | What it does |
|---|---|
| `--help / -h` | Print help |

## lf pr checks

Show CI status for current branch

| Argument | What it does |
|---|---|
| `--watch / -w` | watch Default: false. |
| `--logs / -l` | logs Default: false. |
| `--help / -h` | Print help |

## lf pr next

After an out-of-band merge, rotate this Task to its next serial PR, carrying committed and uncommitted follow-up onto the new branch

| Argument | What it does |
|---|---|
| `<slug>` | Name the next serial branch (defaults to the settled PR's next slug, then the sequence number) |
| `--help / -h` | Print help |

## lf pr publish

Publish a ready PR headlessly: push, create or refresh, print state + URL. Opens no review surface

| Argument | What it does |
|---|---|
| `--model / -m` | model |
| `--title` | title |
| `--body` | body |
| `--help / -h` | Print help |

## lf pr open

Push and create or update a draft PR, then open its GitHub page. Existing ready PRs stay ready; opening a draft does not publish it

| Argument | What it does |
|---|---|
| `--model / -m` | model |
| `--title` | title |
| `--body` | body |
| `--help / -h` | Print help |

## lf pr submit

Prepare a PR to land: sync, clear scratch, mark ready, and assign it to you. Nothing merges until you click merge on GitHub

| Argument | What it does |
|---|---|
| `--strict` | strict Default: false. |
| `--create-pr / -p` | create pr Default: false. |
| `--complete / -c` | Complete after verified merge (the default unless --next is supplied) Default: false. |
| `--next` | next |
| `--worktree / -w` | worktree |
| `--message / -m` | message |
| `--title` | title |
| `--body` | body |
| `--help / -h` | Print help |

## lf pr arm

Prepare a PR, request exact-head auto-merge, and return without watching

| Argument | What it does |
|---|---|
| `--strict` | strict Default: false. |
| `--local` | local Default: false. |
| `--complete / -c` | Complete after verified merge (the default unless --next is supplied) Default: false. |
| `--next` | next |
| `--worktree / -w` | worktree |
| `--message / -m` | message |
| `--title` | title |
| `--body` | body |
| `--help / -h` | Print help |

## lf pr land

Request auto-merge, retain settlement intent, and return

| Argument | What it does |
|---|---|
| `--strict` | strict Default: false. |
| `--local` | local Default: false. |
| `--complete / -c` | Complete after verified merge (the default unless --next is supplied) Default: false. |
| `--next` | next |
| `--worktree / -w` | worktree |
| `--message / -m` | message |
| `--title` | title |
| `--body` | body |
| `--help / -h` | Print help |

## lf pr abandon

Abandon branch: close PR, remove worktree, delete branch

| Argument | What it does |
|---|---|
| `<branch>` | Branch to abandon (default: current) |
| `--force / -f` | force Default: false. |
| `--help / -h` | Print help |

## lf wt

Worktree operations

| Argument | What it does |
|---|---|
| `--help / -h` | Print help |

## lf wt create

Create a low-level sibling worktree

| Argument | What it does |
|---|---|
| `<name>` | Worktree name |
| `--plan` | Print the placement plan without creating a worktree Default: false. |
| `--persistent` | Keep this workspace after delivery and keep scratch local Default: false. |
| `--help / -h` | Print help |

## lf wt switch

Switch to a worktree by name, identity leaf, or full branch

| Argument | What it does |
|---|---|
| `<name>` | Worktree name or full branch name to switch to |
| `--help / -h` | Print help |

## lf wt list

List worktrees (read-only; reflects the last-synced main)

| Argument | What it does |
|---|---|
| `--json` | json Default: false. |
| `--sync` | Fetch origin and fast-forward main before listing (mutates the canonical checkout). Off by default so a list never touches it Default: false. |
| `--help / -h` | Print help |

## lf wt timing

Report how long `lf wt list` has taken on this machine

| Argument | What it does |
|---|---|
| `--json` | json Default: false. |
| `--help / -h` | Print help |

## lf wt prune

Remove clean terminal or inactive worktrees

| Argument | What it does |
|---|---|
| `--dry-run` | Show what would be pruned without removing anything Default: false. |
| `--help / -h` | Print help |

## lf wt delete

Delete a worktree and its local and remote branch; retain PR and Task outcomes

| Argument | What it does |
|---|---|
| `<name>` | Worktree name to remove |
| `--force / -f` | force Default: false. |
| `--help / -h` | Print help |

## lf sync

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

## lf commit

Commit changes

| Argument | What it does |
|---|---|
| `--message / -m` | message |
| `--push / -p` | Push the branch after committing Default: false. |
| `--no-add` | no add Default: false. |
| `<paths>` | Commit only these paths, preserving other staged and unstaged edits |
| `--help / -h` | Print help |

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
| `--parent` | Direct children of an exact or unambiguous parent Process |
| `--caller` | Commands issued by this AgentSession |
| `--search` | Literal command text, ignoring ASCII case |
| `--outcome` | outcome |
| `--task` | Recorded work for a Task, including completed Tasks |
| `--wave` | Recorded work for a Wave |
| `--help / -h` | Print help |

## lf monitor show

Inspect a process or Session by identity

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

## lf monitor work

Stream planning and activity for the selected Work, each part again only when it changes

| Argument | What it does |
|---|---|
| `--json` | json Default: false. |
| `--watch` | Stream NDJSON until stdin closes Default: false. |
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

## lf session resume

Resume a conversation by ID, or the last interactive Session in this worktree

| Argument | What it does |
|---|---|
| `<id>` | Loopflow Session ID or Claude/Codex conversation ID |
| `<message>` | With `-b`: send this as the conversation's next headless turn |
| `--help / -h` | Print help |

## lf session history

Read this conversation's native start, usage and completion receipts

| Argument | What it does |
|---|---|
| `<id>` | Session ID or provider conversation ID |
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
| `--waiting` | Only conversations waiting on you Default: false. |
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
| `<id>` | Session ID or provider conversation ID |
| `--json` | json Default: false. |
| `--replace` | Stop Loopflow-owned clients before resuming here Default: false. |
| `--try` | Ask the provider to resume even when another client is active Default: false. |
| `--help / -h` | Print help |

## lf session ensure

Find or start the one ongoing conversation of this repository, a Wave or a Task

| Argument | What it does |
|---|---|
| `--wave / -w` | The Wave's conversation instead of the repository's |
| `--task` | The Task's: the one chosen, else its only or most recently used conversation |
| `--choose` | Make this conversation of the Task its primary |
| `--json` | json Default: false. |
| `--help / -h` | Print help |

## lf session replace

Give a primary Session's scope a fresh conversation

| Argument | What it does |
|---|---|
| `<id>` | id |
| `--json` | json Default: false. |
| `--help / -h` | Print help |

## lf session rename

Rename a Session; a human name is never replaced by a suggestion

| Argument | What it does |
|---|---|
| `<id>` | Session ID or provider conversation ID |
| `<name>` | name |
| `--suggest` | Propose an agent-generated name; keeps a human-assigned name Default: false. |
| `--json` | json Default: false. |
| `--help / -h` | Print help |

## lf session bind

Assign a Task to a Session that has none; the Task never changes after

| Argument | What it does |
|---|---|
| `<id>` | Session ID or provider conversation ID |
| `--task` | The Task, by its issue identifier (e.g. INF-123) or stable Task ID |
| `--dry-run` | Resolve the exact target without assigning the Session Default: false. |
| `--json` | json Default: false. |
| `--help / -h` | Print help |

## lf session serve-conversation

Run one prepared conversation in its durable terminal

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

Sign the provider's ordinary home in as a stored login: `lf account <provider> use <email>`

| Argument | What it does |
|---|---|
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
| `--plan` | plan |
| `--dry-run` | dry run Default: false. |
| `--json` | json Default: false. |
| `--help / -h` | Print help |

## lf repo release

Release operations (run, check, notes, bump, tag, status)

| Argument | What it does |
|---|---|
| `--help / -h` | Print help |

## lf repo release history

Show original due opportunities and their release evidence

| Argument | What it does |
|---|---|
| `--wave / -w` | wave |
| `--days` | days Default: 35. |
| `--json` | json Default: false. |
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

Show how failed CI is detected, repaired, and landed across this Machine

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

## lf self

Manage the installed Loopflow release and exported skills

| Argument | What it does |
|---|---|
| `--help / -h` | Print help |

## lf self doctor

Diagnose installation, storage, Process integrity and scheduled receipts

| Argument | What it does |
|---|---|
| `--planning` | Diagnose repository planning without changing it Default: false. |
| `--json` | Emit the audit as JSON Default: false. |
| `--help / -h` | Print help |

## lf self install

Install the latest published Loopflow release from any directory

| Argument | What it does |
|---|---|
| `--help / -h` | Print help |

## lf self install schedule

Install the latest Loopflow at login and weekly by default (macOS launchd)

| Argument | What it does |
|---|---|
| `<frequency>` | Weekly: Monday 09:00; daily: 09:00; otherwise on clock boundaries (local time) Default: weekly. |
| `--help / -h` | Print help |

## lf self install recover-switch

Continue one interrupted installation switch from its pinned candidate

Internal command; invoked by the owning operation.

| Argument | What it does |
|---|---|
| `--switch` | The fixed installation switch receipt to continue |
| `--help / -h` | Print help |

## lf self install preflight

Preview whether this build may replace the global lf (read-only). Reads the shared store's migration frontier and validates executable planning references against this binary without changing that frontier. Exits non-zero on refusal so a caller can gate on it

Internal command; invoked by the owning operation.

| Argument | What it does |
|---|---|
| `--json` | Emit the structured PromotionPreview as JSON Default: false. |
| `--help / -h` | Print help |

## lf self install advance-switch

Advance the receipt-selected store with this exact candidate's registry

Internal command; invoked by the owning operation.

| Argument | What it does |
|---|---|
| `--switch` | switch |
| `--help / -h` | Print help |

## lf self install promote

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

## lf self install rollback

Repoint the global CLI at retained prior bytes only after that binary's own preflight proves it recognizes the current store frontier

Internal command; invoked by the owning operation.

| Argument | What it does |
|---|---|
| `--cli-target` | The global CLI symlink to replace (e.g. ~/.local/bin/lf) |
| `--candidate` | The immutable content-addressed prior executable to activate |
| `--help / -h` | Print help |

## lf self sync-skills

Compile loopflow skills into your home vendor Skills directories

Internal command; invoked by the owning operation.

| Argument | What it does |
|---|---|
| `--yes / -y` | Confirm writes under ~/ without prompting Default: false. |
| `--no-prune` | Keep stale loopflow-generated skills Default: false. |
| `--help / -h` | Print help |

## lf config

Read Loopflow configuration

| Argument | What it does |
|---|---|
| `--help / -h` | Print help |

## lf config user

Print the configured participant display name

| Argument | What it does |
|---|---|
| `--json` | json Default: false. |
| `--help / -h` | Print help |

## lf open

Open or focus Loopflow.app

| Argument | What it does |
|---|---|
| `--help / -h` | Print help |

## lf machine

Name and connect to machines

| Argument | What it does |
|---|---|
| `--help / -h` | Print help |

## lf machine id

Print this machine's stable local Machine identity

| Argument | What it does |
|---|---|
| `--json` | json Default: false. |
| `--help / -h` | Print help |

## lf machine add

Discover a remote machine and save its SSH destination

| Argument | What it does |
|---|---|
| `<target>` | target |
| `--label` | label |
| `--repo` | Remote repository path; defaults to this checkout's home-relative path |
| `--json` | json Default: false. |
| `--help / -h` | Print help |

## lf machine list

List saved machines without connecting

| Argument | What it does |
|---|---|
| `<label>` | label |
| `--json` | json Default: false. |
| `--help / -h` | Print help |

## lf machine status

Check reachability and version without prompting

| Argument | What it does |
|---|---|
| `<label>` | label |
| `--json` | json Default: false. |
| `--help / -h` | Print help |

## lf machine rename

Change a saved machine's label

| Argument | What it does |
|---|---|
| `<label>` | label |
| `<name>` | name |
| `--help / -h` | Print help |

## lf machine remove

Forget a connection without touching remote work

| Argument | What it does |
|---|---|
| `<label>` | label |
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

## lf wave new-chapter

Rotate this Wave using its exact destination in a retained chapter plan

| Argument | What it does |
|---|---|
| `<wave>` | wave |
| `<name>` | name |
| `--plan` | plan |
| `--dry-run` | dry run Default: false. |
| `--json` | json Default: false. |
| `--help / -h` | Print help |

## lf wave ensure

Ensure the configured Project is active, or create one with a durable identity

| Argument | What it does |
|---|---|
| `<wave>` | wave |
| `--json` | json Default: false. |
| `--help / -h` | Print help |

## lf wave bind-project

Bind an existing Project UUID as this Wave's shared current selection

| Argument | What it does |
|---|---|
| `<wave>` | wave |
| `<project>` | project |
| `--json` | json Default: false. |
| `--help / -h` | Print help |

## lf wave cron

Local launchd jobs that run lf commands on a schedule

| Argument | What it does |
|---|---|
| `--help / -h` | Print help |

## lf wave cron disposition

Record repair ownership without changing the failed evidence

| Argument | What it does |
|---|---|
| `<subject>` | subject |
| `--wave` | wave |
| `--owner` | owner |
| `--reason` | reason |
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

Validate Machine authority and declared jobs without changing launchd

| Argument | What it does |
|---|---|
| `--wave / -w` | Wave whose GOAL.md `crons:` are validated |
| `--help / -h` | Print help |

## lf wave cron sync

Reconcile installed launchd jobs to match a wave's declared `crons:`

| Argument | What it does |
|---|---|
| `--wave / -w` | wave |
| `--repo` | Install the finite repository Task check on this Machine Default: false. |
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

Set the Machine for Wave schedules and newly created work

| Argument | What it does |
|---|---|
| `<name>` | name |
| `<machine_id>` | machine id |
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

Replace the current chapter's KRs, targets and workflow

| Argument | What it does |
|---|---|
| `--wave / -w` | wave |
| `--plan` | The complete plan as JSON |
| `--help / -h` | Print help |

## lf project

Project-owned planning configuration

| Argument | What it does |
|---|---|
| `--help / -h` | Print help |

## lf project workflow

Select and inspect reusable Workflows

| Argument | What it does |
|---|---|
| `--help / -h` | Print help |

## lf project workflow list

List Workflow definitions, including unavailable local files

| Argument | What it does |
|---|---|
| `--json` | json Default: false. |
| `--help / -h` | Print help |

## lf project workflow show

Show the Project's selected Workflow

| Argument | What it does |
|---|---|
| `<project>` | project |
| `--json` | json Default: false. |
| `--help / -h` | Print help |

## lf project workflow set

Select the Workflow future Tasks take up; captured Tasks stay unchanged

| Argument | What it does |
|---|---|
| `<project>` | project |
| `<name>` | name |
| `--help / -h` | Print help |

## lf project workflow customize

Copy a builtin Workflow when needed and print its local path

| Argument | What it does |
|---|---|
| `<name>` | name |
| `--help / -h` | Print help |

## lf task

Concrete work and Task lifecycle

| Argument | What it does |
|---|---|
| `--help / -h` | Print help |

## lf task workflow

Inspect or reset this Task's captured Workflow

| Argument | What it does |
|---|---|
| `--help / -h` | Print help |

## lf task workflow show

Show the Task's captured graph, position and history

| Argument | What it does |
|---|---|
| `<issue>` | issue |
| `--json` | json Default: false. |
| `--help / -h` | Print help |

## lf task workflow restart

Move the captured Workflow to start without executing or reloading it

| Argument | What it does |
|---|---|
| `<issue>` | issue |
| `--help / -h` | Print help |

## lf task automation

Inspect delivery scheduling and Task CI repair settings

| Argument | What it does |
|---|---|
| `--json` | json Default: false. |
| `--help / -h` | Print help |

## lf task reconcile

Check authorized deliveries once, then exit

| Argument | What it does |
|---|---|
| `--json` | json Default: false. |
| `--help / -h` | Print help |

## lf task follow-up

Record accepted work remaining after merge, or resolve it with evidence

| Argument | What it does |
|---|---|
| `<issue>` | issue |
| `--outcome` | outcome |
| `--evidence` | evidence |
| `--check-at` | Next observation or decision, as an RFC 3339 timestamp |
| `--clear` | Evidence that the remaining work is satisfied or no longer needed |
| `--help / -h` | Print help |

## lf task automate

Enable or hold CI repair for a Task without interrupting running work

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

## lf task checkout

Ensure tracked Task Work and its worktree without launching a Flow

| Argument | What it does |
|---|---|
| `<issue>` | issue |
| `--name` | name |
| `--stack-on` | Fork this Task's worktree from another Task's active PR |
| `--directive` | directive |
| `--json` | json Default: false. |
| `--help / -h` | Print help |

## lf task run

Place a Task's worktree, then run a Flow there like `lf --task ISSUE run FLOW`

| Argument | What it does |
|---|---|
| `<issue>` | issue |
| `<flow>` | The edge's Flow, or a workflow to take up; the Task's only edge or its Project's workflow when omitted |
| `--name` | name |
| `--stack-on` | Fork this Task's worktree from another Task's active PR |
| `--directive` | directive |
| `--reason` | Direction for this run, published to the Task |
| `--force` | Reach `end` although Linear already calls the active Task complete Default: false. |
| `--help / -h` | Print help |

## lf task move

Put a Task at a node of its workflow without running anything; `end` completes it

| Argument | What it does |
|---|---|
| `<issue>` | issue |
| `<node>` | `start`, `end` or one of the workflow's nodes |
| `--reason` | Why, kept in the Task's workflow history |
| `--force` | Reach `end` although Linear already calls the active Task complete Default: false. |
| `--help / -h` | Print help |

## lf task create

File a Task in the current chapter

| Argument | What it does |
|---|---|
| `--wave` | Wave name; defaults to the bound Wave |
| `--title` | Task title; omitted when stdin supplies the report and first line |
| `--notes` | Description; defaults to a report read from stdin |
| `--json` | json Default: false. |
| `--help / -h` | Print help |

## lf task status

Show durable Task facts and its recorded work

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

## lf task files

List one directory in this Task's worktree

| Argument | What it does |
|---|---|
| `<issue>` | issue |
| `<directory>` | directory Default: .. |
| `--cursor` | cursor |
| `--show-ignored` | show ignored Default: false. |
| `--json` | json Default: false. |
| `--help / -h` | Print help |

## lf task file

Read one file from the Task checkout

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

## lf task refile

Move a Task that has no recorded work to another Wave's current Project

| Argument | What it does |
|---|---|
| `<issue>` | issue |
| `--wave / -w` | The Wave to file it under |
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

## lf flow list

List authored flows, or Flows that ran

| Argument | What it does |
|---|---|
| `--json` | json Default: false. |
| `--processes` | List or show Flows that ran, from their Processes, instead of reusable templates Default: false. |
| `--all` | Include every repository and flows with unknown repository evidence Default: false. |
| `--limit` | Page size (default 100) |
| `--after` | Previous page's next identity; retain the same filters |
| `--search` | Literal name or identity containment |
| `--state` | state |
| `--for-task` | Retained Task ID or issue identifier, including completed Tasks |
| `--for-wave` | Retained Wave ID or name |
| `--taskless` | taskless Default: false. |
| `--help / -h` | Print help |

## lf flow show

Inspect an authored flow, or one that ran

| Argument | What it does |
|---|---|
| `<name>` | name |
| `--json` | json Default: false. |
| `--processes` | processes Default: false. |
| `--help / -h` | Print help |

## lf flow customize

Print the repository file that defines a Flow, creating it from the builtin when the repository has none

| Argument | What it does |
|---|---|
| `<name>` | name |
| `--help / -h` | Print help |

## lf skill

Run a skill explicitly

| Argument | What it does |
|---|---|
| `--help / -h` | Print help |
