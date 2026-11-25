---
produces: a working Loopflow path and one next command
---
Set up the part of Loopflow that is wanted here, check it works, and hand back one next command.

The launcher, Skills, Graphs, worktrees and PR tools need no Task, Wave or
Linear. Set up only what the request needs; with no direction, make a plain
agent launch work. Ask one question at a time, and only when the answer
changes the setup. Headless, make only repository-scoped, reversible changes:
logins, personal config, Machines and anything created in an outside service
wait for a person, so report the command that would do it.

Never show credential values or read tokens from dotfiles or the environment;
`lf machine login <provider>` runs each provider's own sign-in.

## 1. Look at what is there

Read-only. A missing optional command is a fact, not a failure, and no
`.lf/config.yaml` doesn't mean uninitialized; the defaults need none.

```bash
git rev-parse --show-toplevel
lf --version
lf machine logins --json     # what this Machine is signed in to
lf machine self --json
lf wave list --json
command -v claude codex opencode pi agy
```

Show one compact picture:

```text
Repository  /path/to/repo
Machine     home_... on this machine
Agents      codex, claude
Accounts    GitHub connected; Linear missing
Waves       designer running here; infrastructure stopped on home_...
Planning    bound to Linear | not connected
```

For the launcher, Skills, Graphs, worktrees or PRs, that is nearly all: check
the accounts that operation needs, offer one next command such as
`lf : "fix the failing test"` or `lf graph pursue`, and stop. Don't run a
coding job as a test.

## 2. Name and agent

`lf config user` gives the name used in saved work. If there is none and
someone is here, ask and save it as `user.name` in `~/.lf/config.yaml`. Never
take a name from an account, directory or commit author, and never put one in
repository config.

One agent must be installed: Claude Code, Codex, OpenCode, Pi or Antigravity
(`agy`). With none,
stop with install commands; don't run a package manager. Keep a valid existing
`agent` setting. With one installed, use it; with several and no preference,
ask, then `lf set --global default_agent <harness>`. Set `agent` in
`.lf/config.yaml` only for a team-wide policy someone chose. Don't add
exclusions, permission bypasses or model pins the repository hasn't shown it
needs.

## 3. Connect what the path needs

The path is an existing Wave, a new Wave, an existing Task, or Skills and
Graphs only. Offer only the missing accounts:

```bash
lf machine login github
lf machine login linear
lf machine login claude        # or codex
```

A connection counts only after the provider flow completes.

## 4. Make the path real

**Existing Wave.** Read `.lf/wave/<name>/GOAL.md` and
`lf wave status <wave> --json`. If Linear is connected and the Wave isn't
bound, offer `lf wave plan connect <wave>`. The first Wave picks the
repository's Linear Team, and that choice needs a person. Never create another
Team, Initiative or Project.

**New Wave.** Ask for one outcome and a short name, then
`lf wave create <name> --objective "<outcome>"`. It commits
`.lf/wave/<name>/` on the main checkout. Edit `GOAL.md` there: the objective,
what success looks like, what is out of scope, and when to stop or ask.

**Existing Task.** Get the exact issue ID, then `lf task run <ISSUE-ID>` and
`lf task status <ISSUE-ID> --json`. Don't make a separate worktree for it.

**Skills and Graphs only.** Offer, don't run: `lf debug -c`, `lf design`,
`lf list`.

## 5. Where work runs

Work runs on this Machine by default; one Machine needs no setup. For a remote
one:

```bash
lf machine add <ssh-target> --label <label>
lf machine logins <machine-label> --json
lf --wave <wave> set machine <machine-label>   # this Wave's new Tasks
lf --repo <repo> set machine <machine-label>   # this repository's new Tasks
lf task move <issue> --machine <machine-label> # one existing Task
```

The Machine choice is personal and never committed. Check the remote has `lf`,
the repository and its logins before pointing work at it; sign it in with
`lf machine login <provider> <machine-label>` from a foreground terminal.

## 6. Check it works

Run the smallest reads that show the chosen path works: `lf machine logins --json`,
`lf machine self --json`, plus `lf wave status <wave> --json` or
`lf task status <ISSUE-ID> --json`. Don't start work as a test.

Finish with what you saw and one next command. Name anything still missing and
the command that fixes it; never hide a gap behind "setup complete".

```text
Machine   home_... (local)
Agents    Codex + Claude installed
Accounts  GitHub + Linear connected
Wave      designer, bound to Linear; 7 open Tasks

Next      lf --wave designer wave-operate
```
