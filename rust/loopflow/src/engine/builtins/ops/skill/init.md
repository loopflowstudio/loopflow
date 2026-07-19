---
interactive: true
produces: connected Loopflow path and an evidence-backed next command
---
Connect this repository to Loopflow's distributed control system.

Loopflow is not primarily a prompt launcher. It is the shared control surface
for durable Waves, Linear-backed Projects and Tasks, GitHub delivery, stable
execution Homes, and the agents that do the work. Establish that system first.
Skills, flows, models, and launch preferences are secondary configuration.

## Reviewer mode

The launch prompt identifies the reviewer.

- **Human reviewer:** ask one consequential question at a time. You may guide
  interactive account connections and edit personal config only after the
  human explicitly chooses them.
- **Parent reviewer:** inspect the same state, but make only repo-scoped,
  reversible changes through the review protocol with the Task. Never initiate
  OAuth, modify personal config, start or place a Wave, create Linear/GitHub
  objects, or guess a human preference. Report those actions as exact next
  commands.

Never expose credential values. Use Loopflow's auth commands; do not read
tokens from dotfiles or environment variables.

## 1. Discover the existing system

Run read-only checks first. Missing optional commands are observations, not
failures.

```bash
git rev-parse --show-toplevel
uname -s
lf --version
lf auth status
lf route show
lf home id --json
lf ls --json
command -v claude
command -v codex
command -v opencode
test -f .lf/config.yaml && echo "repo config: present"
test -f ~/.lf/config.yaml && echo "user config: present"
find wave -mindepth 2 -maxdepth 2 -name GOAL.md -print 2>/dev/null
```

Do not reconstruct distributed state from processes, worktrees, or provider
web pages. `lf ls`, `lf status`, and `lf roadmap` are the shared read surfaces.
If `lf home id` says the local store is not initialized, record that plainly
and continue; do not invent a Home identity.

Present one compact topology:

```text
Repository  /path/to/repo
Home        home_... on this machine | not initialized
Agents      codex, claude
Accounts    GitHub connected; Linear missing
Waves       designer running here; infrastructure stopped on home_...
Planning    bound to Linear | not connected
Config      repo present; personal present
```

Separate observed facts from missing capabilities. Do not call a repository
"uninitialized" merely because it has no `.lf/config.yaml`; existing Wave,
Home, account, or planning state still counts.

## 2. Establish the minimum local authority

At least one supported agent must be available: Claude Code, Codex, or
OpenCode. If none is installed, stop with install commands and end with
`lf init` as the retry. Do not run a package manager.

Installed harnesses are a capability of this Home, not repository policy. One
Home may have Codex while another has Claude or OpenCode. Never rewrite
team-wide repo configuration merely to mirror `command -v` on this machine.

Resolve repo agent configuration conservatively:

- Preserve a valid existing `agent` override.
- Codex is the implicit default. An absent `agent` is valid even when this Home
  lacks Codex; report the local mismatch instead of changing repo policy.
- Change `agent` or `supported_harnesses` only when the human explicitly wants
  a team-wide policy. Ask whether the choice is repo-wide or Home-local before
  writing it.
- A local harness mismatch affects where work can run. It does not invalidate
  the repository.

Create `.lf/config.yaml` only when a real repo-scoped policy is missing and the
human chooses one. Preserve every existing field. For example:

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

Ask the human what they want to make operational now:

1. an existing Wave,
2. a new durable Wave,
3. an existing Linear Task,
4. only direct skills/flows for now.

This answer determines the minimum accounts and files. Do not turn init into a
questionnaire.

For durable planning and delivery, inspect `lf auth status` and offer only the
missing connections:

```bash
lf auth github
lf auth linear
lf auth claude
lf auth status
```

Account connection is an external side effect. A human must choose it and
complete the provider flow. Never claim a provider is connected until
`lf auth status` proves it. Direct skills can proceed with a local agent even
when Linear is absent; do not block that path on PM setup.

## 4. Make one durable path real

### Existing Wave

Read its `wave/<name>/GOAL.md`, then verify its shared state:

```bash
lf status <wave> --json
lf roadmap --wave <wave> --json
lf pm show --wave <wave> --no-sync
```

If PM is not bound and Linear is connected, offer the explicit binding command:

```bash
lf pm init --wave <wave> --team-key <KEY>
```

Creating or rebinding Linear state requires the human's choice. Do not infer a
team key, Initiative, Project, or KR.

### New durable Wave

Ask for one outcome and a short name. Create:

```text
wave/<name>/GOAL.md
```

Write the goal as a durable operating contract: objective, observable success,
project-selection judgment, boundaries, and when to stop or escalate. Do not
create or edit `MEMORY.md`; the Wave runtime owns compiled memory. Runtime
learnings arrive through `lf memory add` and `lf memory update`. Do not create
Projects or Tasks in the goal body.

Then offer Linear binding as above. Every Project belongs to exactly one Wave;
Projects carry definitions and KRs, while Tasks carry concrete work.

### Existing Linear Task

Require its exact issue identifier. If Linear is connected and the Task belongs
to a Wave-owned Project, the durable execution path is:

```bash
lf task run <ISSUE-ID>
lf task status <ISSUE-ID> --json
```

Do not create an ad-hoc worktree or convert the Task into a local prompt.

### Direct skills and flows

Offer the lightweight path without pretending it is the whole product:

```bash
lf debug -c
lf design
lf --list
```

These are next commands, not setup probes; do not run them automatically.
Mention repo-local `.lf/skills/` and `.lf/flows/` only if the user wants to
author reusable behavior.

## 5. Place execution deliberately

The current Home is the default execution authority. Do not place or start a
Wave merely to prove setup.

If the human wants remote execution, explain the durable sequence and use the
actual ids observed from the commands:

```bash
lf ssh <host> --remote-native -- lf home id --json
lf home observe <home-id> ssh://<user>@<host>
lf ssh <home-id> --remote-native -- lf auth status
lf ssh <home-id> --remote-native -- lf route show
lf ls --json
lf work place wave <wave-id> <home-id>
lf home probe <wave> --json
lf start <wave>
```

`--remote-native` means the remote Home uses its installed provider, GitHub,
PM, and secret authority. Before placement, use remote-native reads to verify
that the remote has `lf`, the repository, required accounts, and the intended
route; origin authority is not forwarded. `lf home observe` records the mutable
SSH route for the stable HomeId. Placement is allowed only while no Run is
live. Ask before observing a route, changing placement, or starting a Wave;
each changes durable execution state.

## 6. Prove the result

Run the smallest read-only checks that prove the selected path:

```bash
lf auth status
lf route show
lf home id --json
lf ls --json
```

For a selected Wave, also run `lf status <wave> --json` and
`lf roadmap --wave <wave> --json`. After placement, use
`lf home probe <wave> --json`. For a selected Task, run
`lf task status <ISSUE-ID> --json`. Do not run the machine-wide roadmap or
doctor as routine setup: both can be large, and doctor can surface unrelated
historical problems. Do not start work as a setup test.

Finish with observed state and one primary next command:

```text
Loopflow is connected for this repository.

Home         home_... (local)
Home agents  Codex + Claude installed
Repo policy  inherited defaults
Accounts     GitHub + Linear connected
Wave         designer stopped on home_...
Planning     Linear bound; 2 open Projects / 7 open Tasks

Next         lf start designer
Also         lf roadmap --wave designer | lf task run DES-123 | lf debug -c
```

If something remains unavailable, say exactly which authority is missing and
the command that would establish it. Never hide a missing account, Home,
Wave/PM binding, or agent behind "setup complete."

On macOS, offer `lf desktop` as an optional human control surface after the
selected path is proved. Do not launch it automatically.

## Conversation style

- Lead with the observed topology, not configuration trivia.
- Ask one consequential question at a time.
- Prefer a working durable path over exhaustive optional setup.
- Keep Wave, Project, Task, Home, account, and skill distinct.
- Stop when the chosen path is proved and the next command is obvious.
