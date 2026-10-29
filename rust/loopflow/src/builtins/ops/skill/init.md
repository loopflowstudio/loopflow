---
produces: connected Loopflow path and an evidence-backed next command
---
Set up the part of Loopflow the user wants to use. The launcher, Skills, Graphs,
worktrees and PR tools need no Task, Wave or Linear setup. Desktop offers a guided
work experience; other clients can use the same work APIs with their own workflow.

## Reviewer mode

The launch prompt identifies the reviewer.

- **Interactive reviewer:** ask one consequential question at a time. You may guide
  interactive account connections and edit personal config only after the
  user explicitly chooses them.
- **Parent reviewer:** inspect the same state, but make only repo-scoped,
  reversible changes through the review protocol with the Task. Never guess a
  user preference. OAuth, personal config, Wave Machines, and external object
  creation require the present User. When absent, report the exact blocker and
  next commands; do not manufacture a session.

Never expose credential values. Use Loopflow's account commands; do not read
tokens from dotfiles or environment variables.

## Start from the request

Use the request, installed tools and existing Work association to infer what
needs setup. With no further direction, make a plain agent launch work. Ask only
when a concrete setup choice cannot be inferred; do not ask the user to select
a usage level. Continue existing Work without requiring another opt-in.

For launcher, Skill, Graph, worktree or PR use, check `lf --version`, installed
agent availability, and only the accounts needed for the requested operation.
Preserve existing configuration; defaults need no `.lf/config.yaml`. Prove the
selected capability with its read-only help/status and offer one next command,
such as `lf : "fix the failing test"` or `lf graph pursue`. Do not launch a coding
job as a setup probe. Stop here: no Task creation, Linear connection or Wave lookup.

For existing or explicitly requested managed work, read `lf help skill work` and
continue below. A terminal client needs no Desktop installation and keeps its
own workflow. Only inspect or connect the parts the request needs.

## 1. Discover the selected work system

Run read-only checks first. Missing optional commands are observations, not
failures.

```bash
git rev-parse --show-toplevel
uname -s
lf --version
lf account --cached          # cached; no provider request
lf account --json            # inspect accepted managed evidence
lf account route
lf id --json
lf wave list --json
command -v claude
command -v codex
command -v opencode
test -f .lf/config.yaml && echo "repo config: present"
test -f ~/.lf/config.yaml && echo "user config: present"
find .lf/wave -mindepth 2 -maxdepth 2 -name GOAL.md -print 2>/dev/null
```

Do not reconstruct distributed state from processes, worktrees, or provider
web pages. `lf wave list`, `lf wave status`, and `lf wave show` are the shared read surfaces.
If `lf id` says the local store is not initialized, record that plainly
and continue; do not invent a Machine identity.

Present one compact topology:

```text
Repository  /path/to/repo
Machine        home_... on this machine | not initialized
Agents      codex, claude
Accounts    GitHub connected; Linear missing
Waves       designer running here; infrastructure stopped on home_...
Planning    bound to Linear | not connected
Config      repo present; personal present
```

Separate observed facts from missing capabilities. Do not call a repository
"uninitialized" merely because it has no `.lf/config.yaml`; existing Wave,
Machine, account, or planning state still counts.

## 2. Establish the minimum local authority

Read `lf user --json`: a personal Loopflow `user.name` override wins,
otherwise Git's configured `user.name` supplies the baseline. An available name
needs no additional setup. Agents may also use a name already known in the session.
In an interactive session, if neither source supplies a name, ask what name to
use in saved artifacts. Record an explicit choice or correction as `user.name`
in personal configuration (`$LF_HOME/config.yaml`, or `~/.lf/config.yaml` when
unset), preserving other settings. Blank or absent preferences fall through to
Git. Never infer a name from an account, directory, or commit author, and never
put a personal name in repo Loopflow configuration.
On SSH, preserve the destination owner's preference; the caller's preference
belongs on the originating Machine. In unattended work, leave an absent name unknown.

At least one supported agent must be available: Claude Code, Codex,
OpenCode, or Pi. If none is installed, stop with install commands and end with
`lf init` as the retry. Do not run a package manager.

Installed harnesses are a capability of this Machine, not repository policy. One
Machine may have Codex while another has Claude, OpenCode or Pi. Never rewrite
team-wide repo configuration merely to mirror `command -v` on this machine.

Resolve repo agent configuration conservatively:

- Preserve a valid existing `agent` override.
- An absent `agent` is valid: Loopflow then uses the first of Codex, Claude,
  OpenCode and Pi installed on this Machine. Do not write `agent` to work around a
  missing harness.
- Change `agent` or `supported_harnesses` only when the user explicitly wants
  a team-wide policy. Ask whether the choice is repo-wide or Machine-local before
  writing it.
- A local harness mismatch affects where work can run. It does not invalidate
  the repository.

Create `.lf/config.yaml` only when a real repo-scoped policy is missing and the
user chooses one. Preserve every existing field. For example:

```yaml
supported_harnesses:
  - codex
  - claude
```

Leaving `.lf/config.yaml` absent is correct when defaults suffice. Do not add
exclusion patterns, permission bypasses, model pins, IDE settings, or release
policy without evidence that this repository needs them.

Personal launch preferences belong in `~/.lf/config.yaml`. They are optional
and never block initialization. Discuss them only after the distributed path
works; never modify them for a parent reviewer.

## 3. Connect shared truth for the intended path

Use the work path already selected; ask only for missing scope:

1. an existing Wave,
2. a new durable Wave,
3. an existing Linear Task,
4. only direct skills/graphs for now.

This answer determines the minimum accounts and files. Do not turn init into a
questionnaire.

For durable planning and delivery, inspect `lf account` and offer only the
missing connections:

```bash
lf account connect github
lf account connect linear
lf account connect claude
lf account --cached          # cached; no provider request
lf account --json            # inspect accepted managed evidence
```

OAuth client credentials resolve from environment first, with a Doppler fallback
when configured. If this repository uses Doppler and credentials are missing,
use `doppler run -- lf account connect linear`. Otherwise follow the customer's secret
manager and the exact missing variable names. Never print credential values.

Account connection is an external side effect. The user must choose it and
complete the provider graph. Claim a new connection only after connect completes;
cached status is retained evidence, not a fresh authorization check. Managed
verification requires an accepted managed row from `auth status --json`.
Direct skills can proceed with a local agent even
when Linear is absent; do not block that path on PM setup.

## 4. Make one durable path real

### Existing Wave

Read its `.lf/wave/<name>/GOAL.md`, then verify its shared state:

```bash
lf wave status <wave> --json
lf wave show <wave> --json
lf wave status <wave>
```

If PM is not bound and Linear is connected, offer the explicit binding command:

```bash
lf wave connect <wave>
```

The first Wave establishes the repository Team and defaults its key from the
repository name; `--team-key <KEY>` is an explicit override. Later Waves must
reuse that binding. Choosing the initial repository Team requires the user's
choice. Do not invent another Team, Initiative, Project, or KR; an existing
repository binding changes only through the repository-wide migration.

### New durable Wave

Ask for one outcome and a short name. Create:

```text
.lf/wave/<name>/GOAL.md
```

Write the goal as a durable operating contract: objective, observable success,
boundaries, and when to stop or escalate. Keep chapter priorities, KRs and Tasks
in the chapter plan. Curate durable decisions in `.lf/wave/<name>/MEMORY.md` through
the repository workflow; do not invent runtime memory commands or duplicate the
plan in the goal body.

Then offer Linear binding as above. Connection creates or links the Initiative
and preserves existing Projects; it does not provision an initial Project.
Inspect available Linear status/content: one In Progress Project is the current
plan, Planned Projects hold future plans, and Completed Projects retain history.
A Task takes up the graph it names, else its Wave's default, the repository's `graph:`, or `code`.

If no current Project exists, report that planning gap. Establishing a chapter
requires accepted repository-wide direction and plan dispositions: preview with
`lf wave new-chapter --all <name> --dry-run --json`, then apply with
`lf wave new-chapter --all <name> --json` to apply those dispositions. Every Wave
participates; do not rotate the repository merely to initialize one Wave. A Wave
with no Projects receives an empty plan unless an authored Planned successor
supplies its content. Default graphs remain on the Wave or repository. Existing successors and predecessors
retain identity; rotation completes predecessors rather than deleting them.

Author future content in the Planned Linear Project through an available
provider interface. For an existing current Project,
`lf update-plan --wave <wave> --plan <plan.json>` replaces its complete
content, for example `{"metric_targets":[],"krs":[]}`.
There is no future-plan writer or historical chapter selector in these commands.
Missing access or competing current Projects stays an explicit next action;
never invent a local chapter store or guess a plan from the newest name.

### Existing Linear Task

Require its exact issue identifier. If Linear is connected and the Task belongs
to the repository Team and one Wave-owned Project, the durable execution path is:

```bash
lf task run <ISSUE-ID>
lf task status <ISSUE-ID> --json
```

Do not create an ad-hoc worktree or convert the Task into a local prompt.

### Direct skills and graphs

This path needs no managed setup. Offer a next command:

```bash
lf debug -c
lf design
lf list
```

These are next commands, not setup probes; do not run them automatically.
Mention repo-local `.lf/skills/` and `.lf/graphs/` only if the user wants to
author reusable behavior.

## 5. Place execution deliberately

The current Machine is the default execution authority. Do not place or start a
Wave merely to prove setup.

If the user wants remote execution, explain the durable sequence and use the
actual ids observed from the commands:

```bash
lf machine add <ssh-target> --label <label>
lf machine list --json
lf --machine <machine-id> account --cached
lf --machine <machine-id> account route
lf wave list --json
lf wave status <wave> --json
lf --machine <machine-id> --wave <wave> wave-operate
```

Pick the user's Machine for a Wave's or the repository's new Tasks. It is their
personal choice, never committed repository content:

```bash
lf --wave <wave> set machine <machine-label>   # this Wave's default, copied to their other Machines
lf --repo <repo> set machine <machine-label>   # this repository's default
lf task move <issue> --machine <machine-label> # move one Task there with its Session
lf machine list                                # each Machine with the defaults naming it
```

`lf set` without `--wave` sets the repository. Resolution for a new Task is Wave → parent Waves →
repository → the Machine running the command, so one Machine needs no setup.
An unreachable Machine gets a warning; rerun later. Setting a default moves the
user's Wave or repository Session to that Machine with its history. Scheduled
commands are ordinary scheduler entries on the Machine that runs them. On macOS,
a command that runs an agent or reads Keychain credentials belongs in a
gui-domain LaunchAgent (`StartCalendarInterval`), because cron runs outside the
login session and cannot reach the Keychain; crontab serves Linux and jobs that
need no Keychain. Existing Tasks stay on the Machine in their claim until
`lf task move` moves them.

`lf --machine` always runs the remote `lf`; ordinary `ssh` owns arbitrary remote
commands. Connect required logins with `lf machine connect` in a foreground
terminal. Remote processes use credentials installed on their Machine. Before
setting a Wave Machine, use remote reads to verify that the remote has `lf`, the repository,
required accounts, and the intended route. `lf machine add` discovers the identity
and records its label, repository and SSH route. `lf --wave <wave> wave-operate` makes a finite pass over your started Tasks locally; prefix it with
`lf --machine <machine-id>` to run on the remote Machine. Adding a machine,
setting a Wave Machine, handing off a Task and starting a Wave each change durable
execution state.

## 6. Prove the result

Run the smallest checks that prove the selected path:

```bash
lf account --cached          # cached; no provider request
lf account --json            # inspect accepted managed evidence
lf account route
lf id --json
lf wave list --json
```

For a selected Wave, also run `lf wave status <wave> --json` and
`lf wave show <wave> --json`. After setting a Wave Machine, use
`lf wave status <wave> --json`. For a selected Task, run
`lf task status <ISSUE-ID> --json`. Do not run the machine-wide roadmap or
doctor as routine setup: both can be large, and doctor can surface unrelated
historical problems. Do not start work as a setup test.

Finish with observed state and one primary next command:

```text
Loopflow is connected for this repository.

Machine         home_... (local)
Machine agents  Codex + Claude installed
Repo policy  inherited defaults
Accounts     GitHub + Linear connected
Wave         designer placed on home_...
Planning     Linear bound; 1 current chapter / 7 open Tasks

Next         lf --wave designer wave-operate
Also         lf wave show designer | lf task run DES-123 | lf debug -c
```

If something remains unavailable, say exactly which authority is missing and
the command that would establish it. Never hide a missing account, Machine,
Wave/PM binding, or agent behind "setup complete."

On macOS, offer `lf open` (the repository's conversation in Loopflow Desktop) as an optional interactive control surface after the
selected path is proved. Do not launch it automatically.

## Conversation style

- Lead with the observed topology, not configuration trivia.
- Ask one consequential question at a time.
- Prove the chosen capability without enrolling the user in other features.
- Expose Wave and Task; keep the chapter Project internal. Keep Machine, account, and skill distinct.
- Stop when the chosen path is proved and the next command is obvious.
