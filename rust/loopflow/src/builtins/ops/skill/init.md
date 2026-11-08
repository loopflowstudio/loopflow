---
produces: a working Loopflow path and one next command
---
Set up the part of Loopflow that is wanted here, check it works, and hand back one next command.

The launcher, Skills, Graphs, worktrees and PR tools need no Task, Wave or
Linear. Set up only what the request needs.

## Reviewer mode

The launch prompt names the reviewer.

- **Interactive reviewer:** ask one question at a time, only when the answer
  changes the setup. Connect accounts or edit personal config only once chosen.
- **Parent reviewer:** inspect the same state, but make only repo-scoped,
  reversible changes, through the review protocol with the Task. Never guess a
  preference. OAuth, personal config, Wave Machines and anything created in an
  external service wait for a person; report the blocker and the commands that
  would clear it.

Never show credential values or read tokens from dotfiles or environment
variables; use `lf account`.

## Start from the request

Infer what needs setup from the request, the installed tools and any existing
Task or Wave. With no direction, make a plain agent launch work. Ask only when a
choice can't be inferred. Existing work continues without another opt-in.

For the launcher, Skills, Graphs, worktrees or PRs: check `lf --version`, the
installed agents and the accounts that operation needs. Run its read-only help
or status, offer one next command such as `lf : "fix the failing test"` or
`lf graph pursue`, and stop. No Task, Linear or Wave, and no coding job as a test.

For managed work, read `lf help skill work` and continue below.

## 1. Look at what is there

Read-only. A missing optional command is a fact, not a failure.

```bash
git rev-parse --show-toplevel
lf --version
lf account --cached          # last known state; no provider request
lf account --json            # managed account status
lf account next
lf machine self --json
lf wave list --json
command -v claude codex opencode
test -f .lf/config.yaml && echo "repo config: present"
test -f ~/.lf/config.yaml && echo "user config: present"
find .lf/wave -mindepth 2 -maxdepth 2 -name GOAL.md -print 2>/dev/null
```

Read Waves from `lf wave list`, `lf wave status` and `lf wave show`, not from
processes or worktrees. If `lf id` says the store is not initialized, say so and
continue. Show one compact picture:

```text
Repository  /path/to/repo
Machine     home_... on this machine | not initialized
Agents      codex, claude
Accounts    GitHub connected; Linear missing
Waves       designer running here; infrastructure stopped on home_...
Planning    bound to Linear | not connected
Config      repo present; personal present
```

No `.lf/config.yaml` does not mean "uninitialized"; defaults need none.

## 2. Name and agent

`lf user --json` gives the name used in saved work: personal `user.name`, else
Git's. If neither exists in an interactive Session, ask and save it as
`user.name` in `$LF_HOME/config.yaml` (default `~/.lf/config.yaml`). Never take
a name from an account, directory or commit author, never put one in repo
config, and over SSH leave the remote Machine's name alone. Headless, leave it
unknown.

One agent must be installed: Claude Code, Codex, OpenCode or Pi. If none is,
stop with install commands and `lf init` as the retry; don't run a package
manager. Installed agents belong to this Machine, so never rewrite repo config
to match `command -v`.

- Keep a valid existing `agent` setting.
- One agent installed: use it. Several and no preference: ask, then
  `lf set --global default_agent <harness>`; a repository or step `agent`
  overrides it. Don't guess from install order or from the agent running
  setup. Headless, report `lf -a <harness> init` and leave it open.
- Set `agent` or `supported_harnesses` in `.lf/config.yaml` only for a chosen
  team-wide policy, keeping every existing field. Ask repo-wide or
  Machine-local first.

Don't add exclusions, permission bypasses, model pins or release policy the
repository hasn't shown it needs.

## 3. Pick the path and connect accounts

Use the path the request chose; ask only if it is missing: an existing Wave, a
new Wave, an existing Linear Task, or Skills and Graphs only. That decides which
accounts are needed. Offer only the missing ones:

```bash
lf account connect github
lf account connect linear
lf account connect claude
```

OAuth client credentials come from the environment, then Doppler. In a Doppler
repository with them missing, use `doppler run -- lf account connect linear`;
otherwise name the missing variables. A connection counts only after it is
chosen and the provider flow completes.

## 4. Make the path real

**Existing Wave.** Read `.lf/wave/<name>/GOAL.md`, `lf wave status <wave> --json`
and `lf wave show <wave> --json`. If Linear is connected and the Wave isn't
bound, offer `lf wave plan connect <wave>`. The first Wave picks the repository's
Linear Team (key from the repository name, `--team-key <KEY>` to override), and
that choice needs a person; later Waves reuse it. Never create another Team,
Initiative, Project or KR.

**New Wave.** Ask for one outcome and a short name:

```bash
lf wave create <name> --objective "<outcome>"
```

This starts the first chapter and commits `.lf/wave/<name>/` on the main
checkout. Edit `GOAL.md` there: the objective, what success looks like, what is
out of scope, and when to stop or ask. KRs and Tasks go in the chapter plan;
lasting decisions go in `MEMORY.md`. Then offer Linear binding as above; it
links the Initiative and creates no Project.

With no current (In Progress) Project, say so: `lf wave sync <wave>` adopts an
existing Linear Project, and `lf wave plan bind <wave> <Project UUID>` picks
one explicitly. A new plan needs accepted direction for the whole repository;
start-chapter writes each Wave's plan in place. For an
existing current Project, `lf update-plan --wave <wave> --plan <plan.json>`
replaces its content, e.g. `{"metric_targets":[],"krs":[]}`. Write future plans
in the Planned Linear Project. Never guess a plan from the newest name.

**Existing Linear Task.** Get the exact issue ID, then:

```bash
lf task run <ISSUE-ID>
lf task status <ISSUE-ID> --json
```

Don't make a separate worktree or turn it into a local prompt.

**Skills and Graphs only.** Offer, don't run: `lf debug -c`, `lf design`,
`lf list`. Mention `.lf/skills/` and `.lf/graphs/` only if reusable ones are wanted.

## 5. Choose where work runs

Work runs on this Machine by default; one Machine needs no setup. For a remote
one, use the ids the commands print:

```bash
lf machine add <ssh-target> --label <label>
lf --machine <machine-id> account --cached
lf --machine <machine-id> account next
lf --wave <wave> set machine <machine-label>   # this Wave's new Tasks
lf --repo <repo> set machine <machine-label>   # this repository's new Tasks
lf task move <issue> --machine <machine-label> # one Task, with its Session
lf machine list
```

The Machine choice is personal and never committed. Existing Tasks stay put
until `lf task move`. Before setting a Wave Machine, check remotely that it has
`lf`, the repository and the accounts. `lf --machine` runs the remote `lf`; use
`ssh` for anything else, and `lf machine connect` in a foreground terminal for
logins.

On macOS, a scheduled job that runs an agent or reads Keychain credentials needs
a gui-domain LaunchAgent (`StartCalendarInterval`); cron can't reach the
Keychain.

## 6. Check it works

Run the smallest reads that show the chosen path works: `lf account --cached`,
`lf machine self --json`, plus `lf wave status <wave> --json` for a Wave or
`lf task status <ISSUE-ID> --json` for a Task. Skip the roadmap and doctor; both
are large and doctor reports old, unrelated problems. Don't start work as a test.

Finish with what you saw and one next command:

```text
Loopflow is connected for this repository.

Machine         home_... (local)
Machine agents  Codex + Claude installed
Repo policy     inherited defaults
Accounts        GitHub + Linear connected
Wave            designer placed on home_...
Planning        Linear bound; 1 current chapter / 7 open Tasks

Next            lf --wave designer wave-operate
Also            lf wave show designer | lf task run DES-123 | lf debug -c
```

Name anything still missing and the command that fixes it; never hide a gap
behind "setup complete." On macOS, offer `lf open` (this repository in Loopflow
Desktop) without launching it.
