# lf Command Reference

One binary, three audiences. `lf` launches prompts for humans, gives agents
the verbs to run and steer other agents, and reads the local ledger for
whoever is watching. The map:

| You are | Start with | Deep dive |
|---|---|---|
| A human running prompts | [Basic Usage](#basic-usage), [Context Flags](#context-flags) | [Get Started](getting-started.md) |
| A human operating waves | [Running Waves, Projects, and Tasks](#running-waves-projects-and-tasks), [Speaking to Waves](#speaking-to-waves) | [Waves](waves.md) |
| An agent driving other agents | [Running Waves, Projects, and Tasks](#running-waves-projects-and-tasks), [The Agent Bus](#the-agent-bus) | [The Agent API](agent-api.md) |
| Watching the whole machine | [Reading the Local Ledger](#reading-the-local-ledger) | [Conducting](conducting.md) |

Every read surface takes `--json`; that JSON is the same wire the Mac app
renders.

## Basic Usage

```bash
lf                                 # open or focus Loopflow.app
lf desktop                         # explicit alias
lf <skill>                        # run a skill file
lf <skill>: args                  # run with arguments
lf <namespace>/<skill>            # run a namespaced skill (e.g. gstack/office-hours)
lf npx/<owner>/<repo>            # fetch any Claude Skill live via npx skills
lf : "inline prompt"             # no skill file, just prompt
lf --list                        # show all available skills
```

## Examples

```bash
lf gate                           # run the gate skill
lf implement: add auth            # pass arguments after colon
lf gstack/office-hours            # run a built-in gstack skill
lf office-hours                   # bare name works when unambiguous
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
4. Core built-in skills — `build/`, `govern/`, `ops/` (run `lf --list` for the full catalog)
5. Namespaced built-in skills — e.g. `gstack/<skill>`. Bare names (without `<ns>/`) resolve here only when exactly one namespace owns the name.
6. External skill namespaces — `npx/<owner>/<repo>` fetches live via `npx skills` and caches under `.agents/skills/`; cached or searchable skills can often be run as `npx/<name>`. The legacy `rams/rams` alias also resolves when `~/.claude/commands/rams.md` exists.

Namespaced skills and flows use `/`, not `:`. Run `gstack/office-hours`, not `gstack:office-hours`.

### Skill Arguments

```bash
lf implement: add user authentication
```

Inside skill files, `{args}` is replaced with whatever comes after the colon.

### Builtin Catalog

Skills and flows are organized into three categories by agency: **build**
(manual work you drive), **govern** (autonomous coordination the system
drives), **ops** (side-channel utilities). Run `lf --list` for the live
catalog.

Build skills — you invoke these, often interactively:

| Skill | What it does |
|------|--------------|
| `kickoff` | Elaborate design — alternatives, research, imagine success/failure |
| `research` | Map the territory — architecture, complexity, quality, potential |
| `testing-audit` | Audit test value, rigor, cost, lifecycle ownership, and product proof |
| `iterate` | Read research, write design to address it |
| `refresh-plan` | Reconcile scratch/ with the branch after rebasing |
| `reduce` | Find simplification opportunities |
| `polish` | Find polish priorities |
| `expand` | Find expansion opportunities |
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
| `demo` | Experience-first walkthrough of observable changes |
| `code-review` | Walk through structural and architectural decisions |
| `review-design` | Reshape AI-elaborated design into user intent |
| `refine` | Refine existing work |
| `review-open-work` | Survey branches, PRs, worktrees, and waves for inbox-zero triage |

Govern skills — crons and waves-watching-waves drive these:

| Skill | What it does |
|------|--------------|
| `scan` | Read member wave state — PRs, blocks, progress, git activity |
| `assess` | Judge wave health and identify pressure points |
| `wave-report` | Read health signals across all waves |
| `mutate` | Compose and apply coordinated mutations across member waves |
| `review` | Review mutations, amend or revert if needed |
| `s2-scan` / `s2-assess` | Coordination: backlogs, PR/path overlap, conflict risk and safe ordering |
| `s3-scan` / `s3-assess` | Control: live health, velocity, CI, retries, worker-pool size |
| `s4-scan` / `s4-assess` | Intelligence: dependencies, advisories, upstream APIs, what they imply |
| `s5-scan` / `s5-assess` | Identity: wave roster, policy, boundary and autonomy drift |

Ops skills — wrappers around git, PR, release, and wave state:

| Skill | What it does |
|------|--------------|
| `init` | Set up loopflow in this repo |
| `commit` | Commit with generated message |
| `rebase` | Rebase onto main |
| `pr` | Generate PR title/body and call `lf pr publish --title --body` |
| `land` | Land the PR and prune its merged worker worktree |
| `lint` | Run linter, fix issues |
| `update-wave` | Create, update, or delete wave state |
| `split-wave` | Split a wave into smaller independent waves |
| `release` | Run the full release workflow (notes, PR, tag, status) |
| `release-notes` | Write narrative `RELEASE_NOTES.md` from release context |
| `token-compress` | Compress text into a token budget without silently dropping information |
| `validate` | Validate flows, skills, and directions |

## Context Flags

Write global flags before a built-in subcommand. Unambiguous flags also work
after it:

```bash
lf task run DES-123 --json                           # durable Task Session
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
| `--tui` / `--ide` | Hand off to an interactive vendor session (terminal or vendor app); overrides `session.launch` |

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
| `--tui` / `--ide` | Hand off to an interactive vendor session (terminal or vendor app); overrides `session.launch` |

Flows are defined in `.lf/flows/`. See [Configuration](config.md).

### Builtin Flows

| Flow | Steps |
|------|-------|
| `build` | kickoff → review-design → implement → compress → lint → xor(demo, code-review) → gate |
| `code` | implement → compress → lint → gate |
| `pair` | design → code |
| `ship` | refresh-plan → implement → gate → op: pr publish → op: pr land |
| `deploy` | gate → op: pr land --create-pr |
| `design-and-ship` | design → implement → reduce → polish → deploy |
| `incident` | debug → 5whys → code → deploy |
| `queue` | compress → update-wave → gate |
| `garden` | scan → assess → xor(garden-act, silence) |
| `govern-coordination` | s2-scan → s2-assess → mutate |
| `govern-control` | s3-scan → s3-assess → mutate |
| `govern-intelligence` | s4-scan → s4-assess → mutate |
| `govern-identity` | s5-scan → s5-assess → mutate |
| `release` | op: release run patch |
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
lf wave designer                                   # start the named Wave
lf stop designer                                   # stop its listener and resident
lf project run <linear-project-id>                  # durable Project Session
lf task start "fix the flaky chord-timeout test" --project <linear-project-id>
lf task run DES-123 --directive "fix the parser before the docs"
lf task run DES-125 --headless                       # route Reviews to the Project
lf task run DES-124 --stack-on DES-123
lf task status DES-123
lf queue                                             # User-attention Reviews, oldest first
lf work review task task_...                        # explicit: /continue advances
lf work review task task_... --continue-on-success  # EOF advances
lf work review task task_... --continue-on-exit     # every client exit advances
lf task steer DES-123 "rename the flag"
lf task interrupt DES-123                            # no replacement direction
lf task steer DES-123 "take the smaller approach"
lf task wait DES-123
lf task resume DES-123 --model codex --reason "Claude quota exhausted"
lf project resume <linear-project-id> --model codex
lf work status task task_... --json                  # stable Work projection
lf work close task task_...                         # advance the current Review
lf flow scan-pass "scan the runtime"               # one pass, no loop worktree
```

`lf wave <name>` starts the durable Wave listener, resident, and persistent
playhead. A Project Session pursues one Linear Project's KRs without a worktree.
Each Project has at most one current Session; terminal Sessions remain readable
history and the next pursuit creates a successor. Every Task requires the
current Project Session; `task start/run` ensures it before reserving the Task.
The Task starts only after its Linear issue exists and owns one stable worktree.
Its provider process and transcript are replaceable execution state: plain
`resume` keeps compatible history; `resume --model <agent>` preserves the same
Work, durable Steers, worktree, and PR chain while selecting another provider.
The Session remains resumable through serial PRs, review, and explicit
completion.
Every Task runs `kickoff → iterate N → gate`. A flow step declares `review: true`;
the active Launch routes that Review to the User or the immediate parent Run.
Standard Tasks route kickoff and gate to the human, while the owning Project
conducts interactive iteration steps. `--headless` routes all three to the
Project without skipping their skills.

A Review is the current interactive flow step plus its live Launch and route;
there is no Review id or disposition. `lf task steer` and `lf project steer`
append durable direction before attempting live delivery. `lf work close`
advances the interactive step under its current Basis fence. Interrupt and
replacement direction stay separate: interrupt the active boundary, then
Steer normally.

`lf work review` renders complete root Turn output and sends typed input as
Basis-fenced Steers. `/continue` advances the current flow interval. Bare mode
leaves the Review open on EOF; `--continue-on-success` advances on clean EOF;
`--continue-on-exit` also advances after signals or a client crash, but only if
the same Launch and Basis still own User attention.

`--stack-on` places a new Task worktree on another Task's published PR. Its PR
targets that parent branch automatically, then collapses onto `main` after the
parent merges. The two Tasks keep separate identities, worktrees, and workers.
tmux remains containment and read-only inspection; Review input never writes
terminal bytes into the provider process.

## Presenting an Opaque Launch

```bash
lf launch list --active --json
lf launch status launch_... --json
lf launch present launch_...                 # exec the tmux/provider attach route
lf launch handback launch_... --outcome succeeded
```

`present` is the generic presentation adapter for an opaque TUI Launch: it
executes that Launch's attach route but does not create a Review or become its
identity. The descriptor carries stable Work and Wave identity, Home route,
provider, cwd, attention route, explicit handback evidence, and optional attach
argv; tmux or the provider owns terminal bytes.

Closing the app or terminal does not end the Launch. Record the observed
boundary result with `handback --outcome succeeded|failed|interrupted|unknown`;
process exit alone does not claim success.

## Speaking to Waves

Two wires, not one. The **thread** is the human surface: durable, replayed,
owned by a running Wave. The **bus** is how agents call to each other: a table in
the shared store, ephemeral, no server in the path.

```bash
lf chat "ship the button audit first"       # post into the current wave's thread
lf chat -w infra "CI is red on the PR"      # target a wave by name
lf chat --parent "blocked on schema change" # escalate to the parent wave
lf chat --follow -w intelligence            # watch and speak from one terminal pane
lf chat --history --json -w intelligence    # read the saved tail while stopped
lf memory                                   # print the wave's MEMORY.md
lf memory add "buttons: variants unified"   # publish one replayable fact
lf memory add "workers report via stream" --receipt chat_turn:turn-3
lf memory log                               # print facts added since the last update
lf memory log --json                        # facts with their evidence receipts
lf memory update < MEMORY.md                # replace it from stdin
lf receipt show chat_turn:turn-3            # drill one receipt to its record
lf receipt show pr:loopflow/loopflow#912 --json
```

| Command | What it does |
|---------|--------------|
| `lf chat [TEXT]` | Post into a wave's thread; `--follow` replays the latest 12 turns and continues live while typed lines post, `/status` reads health, and `/quit` leaves. `--history --json` reads the same bounded tail directly from the journal without a listener. Commands, tools, and loop bookkeeping stay out of chat; turn failures remain visible. Without `--follow`, omitted TEXT reads stdin. Outside any wave, one-shot chat prints a short drop note and exits 0 |
| `lf memory [show\|log\|update\|add]` | Read or curate a wave's memory — `log` prints the add stream since the last update; `log --json` emits facts with their evidence receipts; `update` replaces the compiled `MEMORY.md`; `add` publishes a replayable fact, with repeatable `--receipt kind:reference` evidence bindings |
| `lf receipt show TOKEN` | Drill one evidence receipt (`kind:reference`) to its canonical local record — a journal turn, run-events report, trace turn, PM snapshot item, or Task PR. `--json` emits the resolved record |

Managed sessions default to their invoking Wave through `LF_WAVE_ID`. From a
human shell, pass `--wave`; repository location does not identify one of the
Waves sharing `main`.

| Flag | Description |
|------|-------------|
| `-w, --wave NAME` | Target a wave by name |
| `--parent` | Target the invoking wave's parent (`lf chat` / `lf memory`) |
| `--follow` | Replay the selected thread's latest 12 turns and continue live while typed lines post (`lf chat`) |
| `--history --json` | Read the selected Wave's durable local thread without requiring a listener (`lf chat`) |
| `--limit N` | Bound a `--history` read (default: 12) |

## The Agent Bus

```bash
lf radio pub "landed PR #91, tests green"       # report on your own channel
lf radio pub -c infra.148e "rebase and retry"   # steer a specific hand
lf radio pub --parent "blocked on schema change" # escalate to the parent's channel
lf radio sub                                      # hear your channel and its hands
lf radio sub infra.148e --json                    # one hand's traffic as NDJSON
```

Channels are a dot tree: `infra` is the wave, `infra.148e` is one of its hands.
A subscription is a prefix, so `lf radio sub infra` hears the whole family.

| Command | What it does |
|---------|--------------|
| `lf radio pub [TEXT]` | Broadcast one frame on a channel. An INSERT into the shared store, so it works with no wave running; reads stdin when TEXT is omitted. No channel resolves, or no store on this machine — the broadcast drops with exit 0 |
| `lf radio sub [CHANNEL] [--json]` | Tune in to a channel and its descendants until killed. Never opens a socket — the Wave need not be running |

Broadcast, not delivery. `lf radio sub` tunes in at the head and hears only what is
said while it listens: nothing is replayed, and a frame published to a channel
nobody was on is gone. A frame survives one hour, then the sweeper takes it —
the bus is a wire, and `lf runs` plus the merged PR are the records of record. A
running Wave is the one durable subscriber: it polls from a saved cursor, so it
catches its hands' reports across a restart, and when a frame aged out before it
woke, the miss is announced in its thread rather than passed over in silence.

| Flag | Description |
|------|-------------|
| `-c, --channel NAME` | Broadcast on any channel (`lf radio pub`) |
| `--parent` | Broadcast on the parent wave's channel (`lf radio pub`) |
| `--from LABEL` | Byline for machine speech (`--from ci`). Testimony, not proof: the row records it beside the channel the frame arrived on |

## Reading the Local Ledger

```bash
lf runs                         # one row per skill call: context, tokens, cost
lf execs                        # one row per lf process
lf trace 66863649               # select an exec or trace; render its process tree
lf trace 66863649 --json        # inspect the same tree and its skill launches
lf trace 66863649 --json --content --launch <launch> --turn <turn>
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

Use the flags for interactive vendor sessions too (`--tui`): logging into a
managed login with a bare `codex login` creates a second session and evicts
the managed one ("needs re-login"); entering through lf shares one session.

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
of hiding them. `lf context` aggregates one filtered session set without opening
bodies. Its Project and Task filters use captured control identity rather than
inferring ownership from a worktree path. The research-state flags require an
observed steer or a launch containing a current resolvable file-backed instruction
revision; missing revision identity does not match the current-only filter.
`lf trace --content` is the explicit
reader for the exact prompt and normalized conversation at one immutable
run/launch/turn address.

`lf ci` reads durable CI incidents from the local Home store. One failed head is
one attempt; later passing and merge observations close every open attempt on
that PR. `--wave` and `--repo owner/repo` filter the same local report.

`lf doctor` also prints the binary's build provenance, the resolved database
path, and the latest known and applied migrations. Those fields still print
when the database is too new or came from a divergent development build.
The `receipts` check sweeps every wave's memory facts for receipt health:
missing (zero receipts), orphaned (reference resolves to no known record),
cross-wave (receipt wave differs from the claim's wave), and inaccessible
(the evidence source couldn't be read this run, so the receipt can't be
judged — surfaced with the read error, never silently called orphaned).
During the post-contract grace window all findings are warnings, not failures.

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

### Launch an interactive vendor session

```bash
lf design                 # interactive skill → uses session.launch (default: tui)
lf gate --tui             # force a terminal handoff for a normally-headless skill
lf : "fix the bug" --ide -m codex   # force the Codex app instead
```

`--tui` and `--ide` override the repo default. Set `session.launch: ide` in
`.lf/config.yaml` to make the vendor app the default for interactive skills.

### External skills

```bash
lf npx/vercel-labs/deep-research   # fetch + run from the npx skills catalog
lf npx/explain-code                # already-cached skill (no network)
```

`npx/` uses `.agents/skills/` in the current repo as a cache. Use `npx/<owner>/<repo>` when you know the package name; cached or searchable skills can often be run as `npx/<name>`. On a cache miss, Loopflow runs `npx skills add` first, then falls back to `npx skills find` when it needs a package hint. The bundled `gstack/` namespace and core `build/` / `govern/` / `ops/` catalogs are always available, and the legacy `rams/rams` alias still works when `~/.claude/commands/rams.md` is installed.

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
with agent judgment. Before publishing, Loopflow syncs the default branch in
the main repo so the PR is based on current upstream state even from a
sibling worktree. Push or GitHub failure returns an error and presents
nothing.

### lf pr open

Publish (same as `lf pr publish`), then open the PR for review — the GitHub
page in the browser. The explicit, human-initiated review action; agents use
`publish`, `submit`, or `land`. If launching the browser fails, only `open`
fails — the PR is already published and its URL printed.

### lf pr land

Arm auto-merge. GitHub merges when required checks and repository rules pass.

```bash
lf pr land                    # land one PR; the Task stays open
lf pr land -c                 # land, then complete the owning Task
lf pr land --next parser-proof  # name the next serial Task PR
```

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
their base, authored work uses a normal rebase path. If `scratch/` needs to
survive a reset, Loopflow stashes it under `.lf/scratch-stash/` and restores
it afterward.

Keep conflict resolution local when the branch is too large or sensitive to
hand to another agent:

```bash
lf rebase --manual
# edit the conflict paths printed by lf
lf rebase --continue   # stages only the current conflict paths; repeat
lf rebase --abort      # restore the pre-rebase branch
```

Manual recovery stays local and never pushes.

## lf wt

Inspect, switch, and clean worktrees. Normal roadmap work starts with
`lf task run <issue-id>`; `lf wt` remains a low-level Git primitive. Place
dependent roadmap work through `lf task run CHILD --stack-on PARENT`, not
`lf wt`.

```bash
lf wt switch bugs             # by directory name, identity leaf, or full branch
lf wt list                    # worktrees as a tree; --format json
lf wt ci                      # CI status for the current branch
lf wt prune --dry-run         # show every unprotected worktree
lf wt prune                   # force-remove them and their local branches
```

`prune` is intentionally destructive: it preserves main, the current
worktree, nonterminal Task Sessions, and worktrees owned by live processes —
everything else goes, including dirty and unpushed work. Run `--dry-run`
first when the repository contains work created outside Loopflow.

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
lf cron add --wave memory --flow export-memory --schedule daily
lf cron list
lf cron remove --wave memory --flow export-memory
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
lf pm project create --wave designer --title "..." --definition "..." --kr "..."
lf pm project update --wave designer --project ui --definition "..." --kr "..."
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
only while a Task body can write to its Session. Completed issues move too:
Linear cannot remove the shared team from a Project while any issue in that
Project still belongs to it. Before each issue moves, Loopflow records its old
identifier in a comment; after every issue is on the wave team, it narrows the
Projects to that team and reconciles cached Task identifiers. Interrupted runs
resume without duplicating comments or moves.

## lf release

Mechanical release subcommands; `lf release run` is the full workflow.

```bash
lf release run patch          # full release workflow
lf release check              # PRs merged since last tag?
lf release notes 1.2.3        # narrative RELEASE_NOTES.md from decisions + PRs
lf release bump 1.2.3         # bump manifests
lf release tag 1.2.3          # create + push git tag
lf release status             # workflow + GitHub Release status
```

| Path | What it holds |
|------|--------------|
| `release/unreleased/DECISIONS.md` | Append-only ledger of release-worthy decisions during the current cycle |
| `release/vX.Y.Z/DECISIONS.md` | Archived decision ledger for a shipped version |
| `release/vX.Y.Z/NOTES.md` | Snapshot of that version's release notes |
| `RELEASE_NOTES.md` | Always-latest release notes at the repo root |

Interactive runs append durable product and process decisions to the
unreleased ledger; headless runs do not. The release workflow promotes
`release/unreleased/` to `release/v<version>/`, uses `DECISIONS.md` as the
intent source and merged PRs as the shipped-behavior source, and archives
the generated notes. If the ledger is absent, notes fall back to merged PR
history. Headless release automation needs no runner-local agent CLI — if no
harness can start, Loopflow writes deterministic notes from the same context.

## See Also

[The Agent API](agent-api.md) · [Conducting](conducting.md) · [Authoring](authoring.md) · [Get Started](getting-started.md) · [Configuration](config.md)
