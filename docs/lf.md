# lf Command Reference

One binary, three audiences. `lf` launches prompts for humans, gives agents
the verbs to run and steer other agents, and reads the local ledger for
whoever is watching. The map:

| You are | Start with | Deep dive |
|---|---|---|
| A human running prompts | [Basic Usage](#basic-usage), [Context Flags](#context-flags) | [Get Started](getting-started.md) |
| A human operating waves | [Running Waves, Projects, and Tasks](#running-waves-projects-and-tasks), [Speaking to Waves](#speaking-to-waves) | [Waves](waves.md) |
| An agent driving other agents | [Running Waves, Projects, and Tasks](#running-waves-projects-and-tasks) | [The Agent API](agent-api.md) |
| Watching the whole machine | [Reading the Local Ledger](#reading-the-local-ledger) | [Conducting](conducting.md) |

Every read surface takes `--json`; that JSON is the same wire the Mac app
renders.

## Basic Usage

```bash
lf                                 # open or focus Loopflow.app
lf desktop                         # explicit alias
lf <skill>                        # run a skill file
lf <skill>: args                  # run with arguments
lf <namespace>/<skill>            # run a repo-local or installed namespaced skill
lf npx/<owner>/<repo>            # fetch any Claude Skill live via npx skills
lf : "inline prompt"             # no skill file, just prompt
lf --list                        # show all available skills
```

## Examples

```bash
lf gate                           # run the gate skill
lf implement: add auth            # pass arguments after colon
lf team/review                    # run .lf/skills/team/review.md
lf npx/vercel-labs/deep-research  # fetch a skill from the npx skills catalog
lf : "fix the typo"               # inline prompt
lf debug -c                       # paste clipboard, fix the bug
lf task run DES-123 --directive "fix the flaky test" # keep one Task through merge
lf task run DES-124 --stack-on DES-123                # dependent Task, separate worktree
```

## Skills

Names resolve in this order:

1. `.lf/skills/<skill>.md` or `.lf/skills/<ns>/<skill>.md` — repo-local (also overrides builtins)
2. `.claude/commands/<skill>.md` — Claude Code compatible
3. `~/.lf/skills/<skill>.md`, `~/.lf/skills/<ns>/<skill>.md`, or `~/.claude/commands/<skill>.md` — user-global
4. Core built-in skills — `task/`, `project/`, `wave/`, `ops/` (run `lf --list` for the full catalog)
5. External skill namespaces — `npx/<owner>/<repo>` fetches live via `npx skills` and caches under `.agents/skills/`; cached or searchable skills can often be run as `npx/<name>`. The legacy `rams/rams` alias also resolves when `~/.claude/commands/rams.md` exists.

Namespaced skills and flows use `/`, not `:`. Run `team/review`, not `team:review`.

### Skill Arguments

```bash
lf implement: add user authentication
```

Inside skill files, `{args}` is replaced with whatever comes after the colon.

### Builtin Catalog

Skills and flows are organized by the thing they act on: **task**, **project**,
**wave**, and **ops**. The categories share one flat command namespace. Run
`lf --list` for the live catalog.

Task skills — concrete implementation, investigation, review, and delivery:

| Skill | What it does |
|------|--------------|
| `kickoff` | Elaborate design — alternatives, research, imagine success/failure |
| `research` | Map the territory — architecture, complexity, quality, potential |
| `iterate` | Read research, write design to address it |
| `refresh-plan` | Reconcile scratch/ with the branch after rebasing |
| `5whys` | Root cause analysis on a bug fix |
| `implement` | Build from a design doc |
| `compress` | Simplify touched code |
| `gate` | Ship-ready code and reviewer-friendly docs |
| `debug` | Fix an error |
| `ci-fix` | Fix failing CI checks for the current PR |
| `integrate-upstream` | Adapt wave code after rebasing onto main |
| `qa` | Thorough quality assessment of the current branch |
| `triage` | Assess QA findings, separate blocking from polish |
| `design` | Interactive design session |
| `explore` | Investigate the codebase |
| `review-slice` | Autonomously demonstrate behavior, audit implementation against plan, and publish the slice |
| `review-design` | Reshape AI-elaborated design into user intent |
| `refine` | Refine existing work |
Project skills — shape and pursue measured bets inside a Wave:

| Skill | What it does |
|------|--------------|
| `project_clarify` / `project_pursue` / `project_mutate` | Clarify, advance, and judge a Project |
| `project-promote` | Promote a Project into a resident child Wave |
| `expand` / `reduce` / `polish` | Find higher leverage, simplifications, and finish quality |
| `testing-audit` | Audit test value, rigor, cost, lifecycle ownership, and product proof |

Wave skills — maintain the durable operating context and its portfolio:

| Skill | What it does |
|------|--------------|
| `scan` | Read member wave state — PRs, blocks, progress, git activity |
| `assess` | Judge wave health and identify pressure points |
| `wave-report` | Read health signals across all waves |
| `mutate` | Compose and apply coordinated mutations across member waves |
| `review` | Review mutations, amend or revert if needed |
| `wave_clarify` / `wave_pursue` / `wave_mutate` | Clarify, direct, and evolve a Wave |
| `review-open-work` | Survey branches, PRs, worktrees, and waves for inbox-zero triage |
| `update-wave` / `split-wave` | Maintain Wave structure and memory |
| `s2-scan` / `s2-assess` | Coordination: backlogs, PR/path overlap, conflict risk and safe ordering |
| `s3-scan` / `s3-assess` | Control: live health, velocity, CI, retries, worker-pool size |
| `s4-scan` / `s4-assess` | Intelligence: dependencies, advisories, upstream APIs, what they imply |
| `s5-scan` / `s5-assess` | Identity: wave roster, policy, boundary and autonomy drift |

Ops skills — raw prompt logic around mechanical git, PR, and release commands:

| Skill | What it does |
|------|--------------|
| `init` | Connect the repo to Homes, accounts, Waves, and task execution |
| `loopflow-validate` | Validate flows, skills, and directions |
| `commit-message` | Generate a commit message without committing |
| `rebase-conflicts` | Resolve conflicts after the mechanical rebase stops |
| `pr-message` | Generate a PR title and body without publishing |
| `pr-publish` | Generate PR copy and call `lf pr publish` |
| `pr-submit` | Prepare a PR for a human to land |
| `pr-land` | Prepare and land a PR through Loopflow's git machinery |
| `release-run` | Run the full release workflow (notes, PR, tag, status) |
| `release-notes` | Write narrative `RELEASE_NOTES.md` from release context |
| `token-compress` | Compress text into a token budget without silently dropping information |

## Context Flags

Write global flags before a built-in subcommand. Unambiguous flags also work
after it:

```bash
lf task run DES-123 --json                           # durable Task Work
lf task status DES-123 --json                        # same identity and worktree
lf task changes DES-123 --json                       # committed + working changes
lf task diff DES-123 src/parser.rs --json            # one file's Task patch
lf task file DES-123 src/parser.rs --json            # current worktree contents
lf pm --wave designer show                           # normalized onto `show`
lf pm task --wave designer create --title "Fix it"  # normalized onto `create`
lf commit -m "explain the change"                   # -m remains commit-local
```

Flags may cross nested subcommands to reach a selected command that owns the
spelling. If more than one level owns it, a flag already valid at its current
level stays there. Put `--` before literal arguments that look like flags.

### Files and Directories

| Flag | Description |
|------|-------------|
| `--docs PATH[,PATH...]` | Prefetch docs into context—files, globs, or dirs (default: none) |
| `-w, --wave NAME` | Wave name for wave/ scoping |
| `--diff-files / --no-diff-files` | Include files touched by branch (default: off) |
| `--diff / --no-diff` | Include raw `git diff` output |

### Loopflow Guidance

| Flag | Description |
|------|-------------|
| `--no-loopflow` | Omit `LOOPFLOW.md` operating guidance |

### Clipboard

| Flag | Description |
|------|-------------|
| `-c, --clipboard` | Include clipboard content in prompt |

## Run Mode Flags

| Flag | Description |
|------|-------------|
| `-i, --interactive` | Run interactively (can interrupt, redirect) |
| `-b, --batch` | Run in batch/headless mode |
| `--max-turns N` | Cap agent turns for this invocation |

## Model Flags

| Flag | Description |
|------|-------------|
| `-m, --model MODEL` | Choose model (e.g., `claude:opus`, `codex`, `gemini`, `opencode`) |
| `-d, --direction DIRECTION` | Apply direction (comma-separated for multiple) |

## Output Flags

| Flag | Description |
|------|-------------|
| `--tui` / `--ide` | Hand off Claude, Codex, or OpenCode to the terminal, or Claude/Codex to their app; overrides `session.launch` |

## Browser Automation

| Flag | Description |
|------|-------------|
| `--chrome / --no-chrome` | Enable Chrome browser automation |

## Running Flows

Run a named flow (chains of skills):

```bash
lf <flow>
lf ship -w feature-branch
```

| Flag | Description |
|------|-------------|
| `--docs PATH[,PATH...]` | Prefetch docs into context—files, globs, or dirs (default: none) |
| `-w, --wave NAME` | Wave name for wave/ scoping |
| `-m, --model MODEL` | Model to use |
| `--tui` / `--ide` | Hand off Claude, Codex, or OpenCode to the terminal, or Claude/Codex to their app; overrides `session.launch` |

Flows are defined in `.lf/flows/`. See [Configuration](config.md).

### Builtin Flows

| Flow | Steps |
|------|-------|
| `build` | kickoff → review-design → code → review-slice |
| `code` | implement → compress |
| `pair` | design → code |
| `task-design` | kickoff → review-design |
| `slice` | code → review-slice → publish/refresh Task PR |
| `ship` | task-gate → record-learnings → op: pr land -c |
| `deploy` | gate → op: pr land --create-pr |
| `design-and-ship` | design → implement → reduce → polish → deploy |
| `incident` | restore → 5whys |
| `ship-5whys` | implement the next open prevention from the 5 Whys |
| `queue` | compress → update-wave → gate |
| `garden` | scan → assess → xor(garden-act, silence) |
| `govern-coordination` | s2-scan → s2-assess → mutate |
| `govern-control` | s3-scan → s3-assess → mutate |
| `govern-intelligence` | s4-scan → s4-assess → mutate |
| `govern-identity` | s5-scan → s5-assess → mutate |
| `sync` | rebase → integrate-upstream |

`sync` rebases the current branch and refreshes the default branch. The
default-branch refresh is safe from sibling worktrees: it stashes dirty edits
on the checked-out default branch, syncs, then restores them — unless they
collide with paths the sync rewrote, in which case they stay in a
`sync_main: auto-stash` stash so a sync can never silently revert just-landed
work.

Flow authoring — `op:` steps, `xor` branching, routers — is covered in
[Authoring](authoring.md).

## Running Waves, Projects, and Tasks

```bash
lf start designer                                  # serve it from its placed Home
lf wave designer                                   # foreground development mode
lf stop designer                                   # stop it; leave the Home keeper running
lf project run <linear-project-id>                  # durable Project Work
lf task start <linear-project-id> "fix the flaky chord-timeout test"
pbpaste | lf task start incident-management
lf task run DES-123 --directive "fix the parser before the docs"
lf task run DES-124 --stack-on DES-123
lf task status DES-123
lf work asks project proj_...                          # pending questions from owned Tasks
lf work answer ask_... "keep the public name"       # answer one exact Ask
lf task steer DES-123 "rename the flag"
lf task interrupt DES-123                            # no replacement direction
lf task steer DES-123 "take the smaller approach"
lf task wait DES-123
lf task resume DES-123 --model codex --reason "Claude quota exhausted"
lf project resume <linear-project-id> --model codex
lf work status task task_... --json                  # stable Work projection
lf work place wave wave_... home_...                 # move idle Wave Work to a Home
lf flow scan-pass "scan the runtime"               # one pass, no loop worktree
```

`lf start <name>` asks the placed Home's shared keeper to serve the Wave.
`lf wave <name>` runs that Wave listener and resident in the foreground for
development. Project Work pursues one Linear Project's KRs without a worktree.
Each Linear Project has at most one current Work; terminal Work remains readable
history and the next pursuit creates a successor. Every Task requires the
current Project Work; `task start/run` ensures it before reserving the Task.
The Task starts only after its Linear issue exists and owns one stable worktree.
Its provider process and transcript are replaceable execution state: plain
`resume` keeps compatible history; `resume --model <agent>` preserves the same
Work, durable Steers, worktree, and PR chain while selecting another provider.
The Task remains resumable through serial PRs, review, and explicit
completion.
Every Task runs `first → loop N → finally`. Its Project supplies those three
flows; Task launch pins their resolved names. `--first`, `--loop`, and
`--finally` override them only while creating the Task. A skill that needs judgment runs
`lf ask "<question>"`; the exchange routes to the immediate parent Run, or to
the User for a supported interactive root. `lf ask wait` recovers the same
exchange after shell loss.

Ask/Answer does not move Work Basis or advance the flow. `lf task steer` and
`lf project steer` remain unsolicited durable direction, appended before live
delivery is attempted. Interrupt and replacement direction stay separate:
interrupt the active boundary, then Steer normally.

`--stack-on` places a new Task worktree on another Task's published PR. Its PR
targets that parent branch automatically, then collapses onto `main` after the
parent merges. The two Tasks keep separate identities, worktrees, and workers.
tmux remains containment and a presentation route, not product identity.

## Placing Work and Reaching Homes

```bash
lf home id --json
lf home observe <home-id> ssh://jack@mini.local
lf work place wave <wave-id> <home-id>
lf start shipper --json
lf stop shipper
lf ssh <home-id> --remote-native -- lf status shipper --json
```

`lf start` returns the same Wave rows as `lf ls --json`; it does not define a
second launch-result model. With no names it starts every Wave in the current
repo. `lf stop` stops the selected Wave while the Home keeper and sibling Waves
continue.

`lf ssh <HomeId>` resolves the Home's current observed route and makes the
target prove that identity. Add `--remote-native` for durable lifecycle: no
origin provider, GitHub, PM, account, or secret authority crosses SSH. Raw
`lf ssh <host> -- <command>` keeps the foreground credential-forwarding
behavior.

## Presenting an Opaque AgentInvocation

```bash
lf invocation list --active --json
lf invocation status invocation_... --json
lf invocation present invocation_...                 # exec the tmux/provider attach route
lf invocation handback invocation_... --outcome succeeded
```

`present` is the generic presentation adapter for an opaque TUI Invocation: it
executes that Invocation's attach route but does not create an Ask or become
its identity. The descriptor carries the supervising Run and its stable Work,
Wave, Home, cwd, and containment alongside provider trace, explicit
handback evidence, and optional attach argv.

Closing the app or terminal does not end the Invocation. Record the observed
boundary result with `handback --outcome succeeded|failed|interrupted|unknown`;
process exit alone does not claim success.

## Speaking to Waves

The **thread** is the human surface: durable, replayed, and owned by a running
Wave. Typed Work observations carry Project and Task progress to their parent.

```bash
lf chat "ship the button audit first"       # post into the current wave's thread
lf chat -w infra "CI is red on the PR"      # target a wave by name
lf chat --parent "blocked on schema change" # escalate to the parent wave
lf chat --follow -w intelligence            # watch and speak from one terminal pane
lf chat --history --json -w intelligence    # read the saved tail while stopped
```

| Command | What it does |
|---------|--------------|
| `lf chat [TEXT]` | Post into a wave's thread; `--follow` replays the latest 12 turns and continues live while typed lines post, `/status` reads health, and `/quit` leaves. `--history --json` reads the same bounded tail directly from the journal without a listener. Commands, tools, and loop bookkeeping stay out of chat; turn failures remain visible. Without `--follow`, omitted TEXT reads stdin. Outside any wave, one-shot chat prints a short drop note and exits 0 |

A Wave's durable memory is the ordinary repository file `wave/<name>/MEMORY.md`
— read and edit it directly.

Managed Work processes default to their invoking Wave through `LF_WAVE_ID`. From a
human shell, pass `--wave`; repository location does not identify one of the
Waves sharing `main`.

| Flag | Description |
|------|-------------|
| `-w, --wave NAME` | Target a wave by name |
| `--parent` | Target the invoking wave's parent (`lf chat`) |
| `--follow` | Replay the selected thread's latest 12 turns and continue live while typed lines post (`lf chat`) |
| `--history --json` | Read the selected Wave's durable local thread without requiring a listener (`lf chat`) |
| `--limit N` | Bound a `--history` read (default: 12) |

## Reading the Local Ledger

```bash
lf ls --json                    # every durable Wave and its Home/runtime evidence
lf status <wave> --json         # one Wave's Work hierarchy, Runs, and attention
lf roadmap --json               # current plan across Waves joined to runtime truth
lf runs                         # one row per skill call: context, tokens, cost
lf execs                        # one row per lf process
lf trace 66863649               # select an exec or trace; render its process tree
lf trace 66863649 --json        # inspect the same tree and its skill invocations
lf trace 66863649 --json --content --invocation <invocation> --turn <turn>
lf context --days 30 --repo "$PWD" --project context --task W2-71 --json
lf context --days 30 --repo "$PWD" --steered-only --current-revision-only --json
lf usage                        # subscription % per account + spend by repo/provider
lf usage --refresh              # poll every account's provider now
lf usage --json --days 30       # one additive row per measured provider Turn
lf ci --since 7d                # CI repair attempts, latency, and outcomes
lf ci --since 7d --json         # complete machine-wide incident receipt
lf top                          # persisted last-hour Turn throughput + live processes
lf doctor                       # audit continuity, identity, lineage, coverage, receipts
lf doctor --json                # machine-readable audit
```

`lf ls` reads the Wave registry. `lf status` focuses one Wave's operational
truth. `lf roadmap` overlays the current Linear-backed plan without creating a
second runtime model.

```bash
lf -m codex --account manabot-eng@ : "fix the tests"   # prefer this login, then route
lf --account claude=jack@ --account codex=loopflow-eng@ implement
lf --only-account codex=manabot-eng@ review             # no fallback login
```

`--account <email-prefix>` prefers each matching managed login before its
provider's normal route. The first preferred attempt bypasses stored health;
a missing credential continues through the healthy fallback route.
`--only-account` restricts the invocation and its children to exactly the
selected provider accounts. Both flags are repeatable and accept
`claude=<selector>` or `codex=<selector>`. They cannot be combined.

Use the flags for Claude and Codex terminal sessions too (`--tui`): logging
into a managed login with a bare `codex login` creates a second session and
evicts the managed one ("needs re-login"); entering through lf shares one
session.

Without an account flag, managed Claude and Codex launches use the repository
route, then the default route. If neither exists, all automatic managed logins
are eligible and Loopflow skips known cooling or limited accounts. If no
managed login exists, the provider CLI uses its ambient default credentials.

`lf usage` leads with each managed account's subscription state — provider-
reported plan, session and weekly windows as percent *used*, reset times —
from stored observations (harness streams report them mid-run) topped up by a
live poll when older than 15 minutes. `--refresh` polls everything now;
`--cached` skips polling. A revoked credential shows the fix
(`lf auth connect <provider> <email>`), not a blank. The spend table below it sums
provider-measured Turn rows; TOTAL is input+output with cache reads their own
column, and `% TOKENS` is each row's slice of all tokens in the window — a
distribution across repos, not a subscription measure.

Under forwarded account authority, subscription polling is unavailable so the
remote account store is never consulted. `lf usage` still prints process token
spend from the local execution ledger.

A run is one agent-backed skill invocation. It owns the context, model, token,
cost, and outcome evidence. An exec is one `lf` process; nested execs share a
trace. `lf trace` accepts an exec or trace id and leaves killed processes open instead
of hiding them. `lf context` aggregates one filtered Invocation set without opening
bodies. Its Project and Task filters use captured control identity rather than
inferring ownership from a worktree path. The research-state flags require an
observed steer or a launch containing a current resolvable file-backed instruction
revision; missing revision identity does not match the current-only filter.
`lf trace --content` is the explicit
reader for the exact prompt and normalized conversation at one immutable
run/invocation/turn address.

`lf ci` reads durable CI incidents from the local Home store. One failed head is
one attempt; later passing and merge observations close every open attempt on
that PR. `--wave` and `--repo owner/repo` filter the same local report.

`lf doctor` also prints the binary's build provenance, the resolved database
path, and the latest known and applied migrations. Those fields still print
when the database is too new or came from a divergent development build.
## Measuring Codebase Weight

```bash
lf tokens                       # lines and model tokens by tracked path
lf tokens --days 365            # daily history, grouped by file extension
lf tokens --json                # token-weighted tree for other tools
```

`lf tokens` counts with the same tokenizer used by the context budget. It skips
untracked and non-UTF-8 files; a symlink counts its tracked link text instead of
duplicating its target; history walks git blobs without checking them out.

## What's Included by Default

Every skill automatically includes:

| Context | Default | How to disable |
|---------|---------|----------------|
| **Agent doc** (AGENTS.md / CLAUDE.md / STYLE.md) | ✓ included | — |
| **Loopflow operating guidance** | ✓ included | `--no-loopflow` |
| **scratch/** | ✓ included | — |
| **wave/** | ✓ included | — |

## What's Opt-In

These require explicit flags or config:

| Context | How to enable |
|---------|---------------|
| **Docs** (files, globs, directories) | `--docs README.md,docs/` or `docs:` config |
| **Raw diff** (line-by-line changes) | `--diff` |
| **Branch files** (full changed file bodies) | `--diff-files` |
| **Clipboard** | `-c` / `--clipboard` |
| **Chrome automation** | `--chrome` |

See [Configuration](config.md) for setting defaults via config file.

## Examples

### Debug with clipboard

```bash
# Run tests, copy the error
lf debug -c
```

### Prefetch docs into context

```bash
lf qa --docs src/api/
```

Gathers `*.md` under `src/api/` into context before the prompt runs. Unlike
the old area scope, `--docs` only prefetches—it doesn't restrict which
files the agent touches.

### Use a different model

```bash
lf implement: add caching -m codex
```

### Apply a direction

```bash
lf gate -d ux
lf implement -d ux,clarity
```

### Disable loopflow operating guidance

```bash
lf gate --no-loopflow
```

`LOOPFLOW.md` carries loopflow-specific guidance for inline execution and
mechanical git/PR operations. Tier skills add scoped delegation. Use
`--no-loopflow` for a leaner prompt.

### Include clipboard content

```bash
lf debug -c    # include current clipboard text in the prompt
```

### Launch Claude, Codex, or OpenCode interactively

```bash
lf design                 # interactive skill → uses session.launch (default: tui)
lf gate --tui             # force a terminal handoff for a normally-headless skill
lf : "fix the bug" --ide -m codex   # force the Codex app instead
```

`--tui` opens Claude, Codex, or OpenCode in the terminal. `--ide` opens Claude
or Codex in its app. Both override the repo default. Set `session.launch: ide`
in `.lf/config.yaml` to make the app the default for interactive skills.

### External skills

```bash
lf npx/vercel-labs/deep-research   # fetch + run from the npx skills catalog
lf npx/explain-code                # already-cached skill (no network)
```

`npx/` uses `.agents/skills/` in the current repo as a cache. Use `npx/<owner>/<repo>` when you know the package name; cached or searchable skills can often be run as `npx/<name>`. On a cache miss, Loopflow runs `npx skills add` first, then falls back to `npx skills find` when it needs a package hint. The core `task/` / `project/` / `wave/` / `ops/` catalogs are always available, and the legacy `rams/rams` alias still works when `~/.claude/commands/rams.md` is installed.

## PR Operations

The publish/submit/land contract every launched agent receives is
`rust/loopflow/src/engine/builtins/LOOPFLOW.md` — that file is canonical for
agent-facing semantics; this section is the human reference.

### lf pr publish

Push and create or refresh a PR, then print its state and URL. Opens no
browser — this is the headless publication command agents use.

```bash
lf pr publish
lf pr publish --title "area: short title" --body "## Summary ..."
lf -m codex pr publish        # one-off agent override for copy generation
```

When `-m` is omitted, copy generation uses `agent:` from `.lf/config.yaml` or
`~/.lf/config.yaml`. Use the `pr` ops skill to generate `--title`/`--body`
with agent judgment. Publication commits and pushes the current branch as-is;
it never fetches to integrate, rebases, rewrites Task stack metadata, or
launches conflict recovery. A PR may remain behind its base until `lf rebase`,
`lf gate`, `lf pr submit`, or `lf pr land` owns integration. Push or GitHub
failure returns an error and presents nothing.

### lf pr open

Publish (same as `lf pr publish`), then open the PR for review — the GitHub
page in the browser. The explicit, human-initiated review action; agents use
`publish`, `submit`, or `land`. If launching the browser fails, only `open`
fails — the PR is already published and its URL printed.

### lf pr submit

Prepare the exact PR head, assign it to you, and stop for your merge click.
Nothing merges automatically.

```bash
lf pr submit
```

On Task PRs, submit records one User merge request containing the exact head
and Continue/Complete disposition. A later Task resume or head-changing
Loopflow operation clears it.

### lf pr land

Arm auto-merge. GitHub merges when required checks and repository rules pass.

```bash
lf pr land                    # land one PR; the Task stays open
lf pr land -c                 # land, then complete the owning Task
lf pr land --next parser-proof  # name the next serial Task PR
```

On Task PRs, land records the same head-and-disposition request with Auto as
the operator. `--match-head-commit` fences the arming command; Loopflow revokes
Auto before its own later head mutation. Concurrent Loopflow finalization and
push commands in one worktree are refused rather than interleaved.

Submit and land clear `scratch/`, preserve a recovery ref, collapse the
authored range to one tree-identical commit, replay that commit onto the pinned
target, verify it, and push once. Ordinary `lf rebase` keeps commit history.

### lf pr abandon

Close the PR, remove the worktree, delete the branch.

```bash
lf pr abandon feature-branch
lf pr abandon feature-branch --force   # skip confirmation, allow dirty
```

## lf commit

```bash
lf commit                     # stage all changes, generate a message, commit
lf commit -m "message"        # override the generated message
lf commit -p                  # commit and push
lf commit --no-add            # commit only what is already staged
```

## lf rebase

Plan or update the current branch against the right base.

```bash
lf rebase          # update the branch
lf rebase --plan   # show the strategy without changing git
lf rebase origin/main          # explicit target
```

Classifies the branch before mutating git: disposable branches can reset to
their base, authored work uses a normal rebase path. Clean updates stay
mechanical. A conflict keeps the first sequencer in place for one authorized
recovery agent; Loopflow verifies the pinned target, branch, dirty state, and
remote head before reporting success. If `scratch/` needs to survive a reset,
Loopflow stashes it under `.lf/tmp/scratch-stash/` and restores it afterward.
Loopflow records reviewed conflict resolutions with command-scoped rerere and
keeps auto-staging disabled. Repeating the same conflict reuses that resolution
mechanically and stages only its unmerged paths.

Keep conflict resolution local when the branch is too large or sensitive to
hand to another agent:

```bash
lf rebase --manual
# edit the conflict paths printed by lf
lf rebase --continue   # stages only the current conflict paths; repeat
lf rebase --abort      # restore the pre-rebase branch
```

Manual recovery stays local and never pushes. `--continue` and `--abort`
atomically adopt a stale Loopflow operation after its owner dies. A rebase
started with raw Git has no owner record, so name that destructive intent:

```bash
lf rebase --continue --adopt
lf rebase --abort --adopt
```

Plain `lf rebase` never adopts or aborts an existing Git operation.

## lf wt

Inspect, switch, and clean worktrees. Normal roadmap work starts with
`lf task run <issue-id>`; `lf wt` remains a low-level Git primitive. Place
dependent roadmap work through `lf task run CHILD --stack-on PARENT`, not
`lf wt`.

```bash
lf wt switch bugs             # by directory name, identity leaf, or full branch
lf wt list                    # worktrees as a tree; --format json
lf wt ci                      # CI status for the current branch
lf wt prune --dry-run         # show terminal or week-stale worktrees
lf wt prune                   # remove them and their local branches
```

`prune` never removes a worktree with uncommitted files. It removes clean
worktrees immediately when the remote branch is gone, the work landed, or the
current-head PR closed. It also removes a clean branch after seven days without
branch activity when no current-head PR is open. Main, the current worktree,
nonterminal Tasks, and worktrees owned by live processes remain protected.
Use `lf wt remove NAME --force` for an explicit destructive override.

`lfd` runs a lossless sweep on startup and every 15 minutes: only clean
landed, remotely deleted, or terminal Task worktrees are removed. Disable
with `autoprune: false` in config. Subscribe the daemon to GitHub merge and
branch-deletion webhooks by defining `LF_GITHUB_WEBHOOK_URL` and
`LF_GITHUB_WEBHOOK_SECRET` in Doppler; the secret travels over stdin and
never appears in process arguments or the service file.

## lf cron

Install local launchd jobs that run `lf` commands on a schedule. (Wave crons
in `GOAL.md` frontmatter are separate — the resident fires those; see
[Waves](waves.md#crons).)

```bash
lf cron add --wave coordination --flow govern-coordination --schedule daily
lf cron list
lf cron remove --wave coordination --flow govern-coordination
```

`add` writes `~/Library/LaunchAgents/loopflow.cron.<wave>.<flow>.plist` and
loads it with launchd; the job runs `lf <flow> --wave <wave>` from the
current repo.

## lf pm

Read and edit a wave's Linear planning state. Each wave is backed by one
Linear Initiative, projects are Linear Projects, tasks are Issues. `sync`
refreshes the local SQLite read model used by every other read surface.

```bash
lf pm status                                # linked waves and task counts
lf pm init --wave designer --team-key DSG   # connect or rebind Initiative + team
lf pm sync --wave designer                  # refresh SQLite from Linear
lf pm sync --plan                           # report drift without writing
lf pm show --wave designer                  # read; refresh when stale
lf pm show --wave designer --no-sync        # cache-only agent/app read
lf pm show --wave designer --project ui     # filter to one project
lf pm project create --wave designer --title "..." --definition "..." \
  --first task-design --loop slice --finally ship --kr "..."
lf pm project update --wave designer --project ui --first incident \
  --loop ship-5whys --finally ship
lf pm project archive --wave designer --project retired-bet
lf pm task create --wave designer --project ui --title "Dark mode"
lf pm task update --id 1207... --title "Refine dark mode"
lf pm task done --id 1207... --pr "https://github.com/acme/app/pull/42"
lf pm task move --id 1207... --wave designer --project api
lf pm rename --wave designer --title "Designer"   # rename the Initiative
lf pm reteam --wave designer --apply    # move the hierarchy when no body is writing
lf pm doctor                            # flag issues stranded in the old team
```

Connect Linear first with `lf auth linear`. `lf pm init` pins the Initiative
and a Wave-owned team into `GOAL.md` frontmatter; the team key becomes each
Task's prefix (`PRD-1`, `INF-1`) so every wave owns its identifiers. When no
id is pinned, init links one exact Initiative-title match, creates one when
absent, and fails on duplicates. Creation fails closed: `project create` and
`task create` require a bound team and error with the `lf pm init` recovery
rather than silently attaching work to a shared team. Reads stay
team-agnostic.

Linear's Projects view is flat, so provider titles use `<Wave> — <Project>`;
Loopflow strips that display prefix and keeps the canonical slug
(`Product — Loopflow API` remains `project:loopflow-api`). `show` serves
snapshots younger than an hour without a network request, tries a
five-second refresh for older ones, and refuses to silently serve a snapshot
older than a week. Use `--no-sync` in agents and UI paths so rendering never
waits on Linear.

`lf pm reteam` migrates a wave's existing issues into its own team. It
**defaults to a dry run** and only mutates with `--apply`; it defers an issue
only while a Task body can write in its worktree. Completed issues move too:
Linear cannot remove the shared team from a Project while any issue in that
Project still belongs to it. Before each issue moves, Loopflow records its old
identifier in a comment; after every issue is on the wave team, it narrows the
Projects to that team and reconciles cached Task identifiers. Interrupted runs
resume without duplicating comments or moves.

## lf release

Mechanical release subcommands; `lf release run` is the full workflow.

```bash
lf release run patch          # full release workflow
lf release check              # exact commits in the target range
lf release notes 1.2.3        # narrative notes from decisions + commits + PRs
lf release bump 1.2.3         # bump manifests
lf release tag 1.2.3          # create + push git tag
lf release publish v1.2.3 --notes RELEASE_NOTES.md --asset dist/lf.tar.gz
lf release publish v1.2.3 --finalize
lf release status             # workflow + GitHub Release status
```

`release.targets.<name>.publisher` is an argv list for the credentialed host
publisher. `lf release run` appends `check` before changing release state, then
downloads the successful hosted build and invokes it with `publish --tag ...
--artifacts ...` from an exact-tag worktree. No merged changes is a successful
no-op. An incomplete latest tag resumes; it never cuts a newer tag around a
failed publication.

| Path | What it holds |
|------|--------------|
| `release/unreleased/DECISIONS.md` | Append-only ledger of release-worthy decisions during the current cycle |
| `release/vX.Y.Z/DECISIONS.md` | Archived decision ledger for a shipped version |
| `release/vX.Y.Z/NOTES.md` | Snapshot of that version's release notes |
| `RELEASE_NOTES.md` | Always-latest release notes at the repo root |

Interactive runs append durable product and process decisions to the
unreleased ledger; headless runs do not. The release workflow promotes
`release/unreleased/` to `release/v<version>/`, uses `DECISIONS.md` as the
intent source, the exact git range as shipped-behavior truth, and merged PRs as
narrative context, then archives the generated notes. If the ledger is absent,
notes fall back to commits and PR history. Headless release automation needs no
runner-local agent CLI — if no harness can start, Loopflow writes deterministic
notes from the same context. Configure repository-specific verification,
preparation, and completion evidence under `release.targets`; see
[Configuration](config.md).

## See Also

[The Agent API](agent-api.md) · [Conducting](conducting.md) · [Authoring](authoring.md) · [Get Started](getting-started.md) · [Configuration](config.md)
