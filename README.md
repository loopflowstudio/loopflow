# Loopflow

Loopflow helps you create and run **Waves** — persistent agents that work toward
an outcome. Write a Wave's goal once; it coordinates Linear-backed Projects and
Tasks, remembers what it learns, and keeps one steerable conversation beside
the live work map.

Start a wave by hand and steer it interactively. As it earns trust, let it loop — selecting Projects, directing Tasks, and reacting to changes on its own.

## Waves

A wave is a named agent with a goal. Two files author it:

| File | Holds |
|------|-------|
| **`wave/<name>/GOAL.md`** | The wave's intent and loop prompt — what it's for, how it judges progress |
| **`wave/<name>/MEMORY.md`** | What the wave remembers between loops — written by the wave agent |

```markdown
<!-- wave/designer/GOAL.md -->
## Objective

Keep the design system coherent. Each wake: read the Linear Projects and Tasks,
direct the Project with the highest-leverage open KR, start a concrete Task only
after it has a Linear issue, and fold what changed into memory.

## Measures

- **Quality**: design reviews are complete.

## Process

Use the build flow for implementation work; write a scratch design first when a
change crosses component or product boundaries.
```

Run the wave in your terminal:

```bash
lf wave designer              # one persistent Wave loop, until Ctrl-C
lf chat --steer "ship the button audit first" # steer the live body, else queue
lf chat --history --json -w designer # read saved Chat without a listener
lf memory add "buttons: variants unified" # curate what it knows
lf stop designer              # stop its listener and resident gracefully
```

Detached Wave, Project, and Task processes use named tmux sessions. Use
`lf project attach <project>` or `lf task attach <issue-id>` for an audited
writable control prompt; tmux is process lifetime and inspection, not the
steering protocol.

`lf wave <name>` starts a long-lived listener and resident at the repo's clean
canonical main checkout. Wave turns coordinate there; Task Sessions own every
repository mutation. Progress and chat are
a single conversation: `lf chat --steer` reaches the body now playing (and
queues when it cannot). Truth is an append-only journal, so a restart keeps the
whole thread while human chat resumes from its recent tail. The saved tail is
readable while the Wave is stopped. Commands, tools,
file edits, and loop bookkeeping stay out of chat; decisions, deliveries, and
human-level failures remain visible. Humans use `lf chat`; agents broadcast with `lf radio pub`; `lf
memory` curates retained facts. A running Wave folds family reports into its
thread with attribution. Outside any wave a publish prints a short drop note and
exits 0, so the verbs are safe in every prompt. See
`rust/loopflow/src/wave/README.md` for the wire contract. Inspect a live
Wave/Project/Task hierarchy and open the app with:

```bash
uv run python scripts/demo_sessions.py infrastructure --task INF-123 --app
```

Add `--start-wave`, `--sync`, or `--start-task` only when you intend their
provider, Linear, or worktree side effects.

The five Viable System Model charters ship as builtin goals `s1`…`s5`. Run one directly:

```bash
lf wave s3            # the s3 (control) charter
lf stop s3            # stop only this wave
```

The wave agent coordinates concrete work through Linear-backed Task Sessions:

```bash
lf task start "unify button variants" --project <linear-project-id> \
  --directive "fix the shared primitive before the call sites"
lf task steer INF-123 "also audit the settings panes"
lf task receipt COMMAND_ID --until incorporated --timeout 30s --json
lf task acknowledge INF-123 --directive 2 --summary "the shared primitive is first"
lf task decide INF-123 DECISION_ID revise --message "cover the boundary race"
lf task wait INF-123
lf task resume INF-123 --model codex --reason "Claude quota exhausted"
```

Present one deliberate interactive child without inventing another parent
lifecycle:

```bash
lf handoff attach ih_0123456789abcdef0123456789abcdef --json
lf handoff complete ih_0123456789abcdef0123456789abcdef \
  --summary "login complete; auth tests pass"
lf handoff back ih_0123456789abcdef0123456789abcdef \
  --summary "finish the review fixes headlessly"
```

`lf handoff open` records the prepared terminal beneath the owning Wave,
Project Session, or Task Session. Every presentation reads the same
store-backed attach descriptor; tmux or the vendor keeps the terminal bytes.
The handoff references the existing body generation, so it cannot become a
second process owner.

The Linear task exists before its worktree. One durable Task Session retains
that stable sibling worktree, directive, supervision state, and PR history
through serial PRs, review, CI repair, provider failure, and explicit
completion. A provider process and transcript are a replaceable body generation;
plain `resume` keeps compatible provider history, while `--model` hands the same
Session to another agent. Root Task PRs target `main`; dependent Tasks get their
own worktree and target the parent Task's active PR until it merges.

Task Sessions inherit the Wave objective, curated memory, Project definition,
and KRs. Typed, idempotent Task observations keep the Wave informed without
copying raw tool chatter into its thread. Every Task reports through the one
Project Session for its Linear Project and asks that Project for routine
decisions. The owning Wave can inspect or override any Task as root authority,
without becoming a second supervision path. Loopflow does not invent a default
Project; `task start` requires one and `task run` resolves one from the issue.

Inspect the Task's actual workspace without reconstructing Git state in a
client:

```bash
lf task changes INF-123
lf task diff INF-123 src/parser.rs
lf task file INF-123 src/parser.rs
```

The Mac Task inspector uses the same reads for its changed-file list, patch,
and current-file views. It also opens multiple embedded Ghostty/tmux shells or
the same worktree in Warp.

Run an existing Linear Project or task:

```bash
lf project run <linear-project-id> --directive "pursue onboarding first"
lf project steer <linear-project-id> "prioritize the CLI path"
lf project acknowledge <linear-project-id> --directive 2 --summary "CLI proof is first"
lf project wait <linear-project-id> --until waiting

lf task run INF-123 --name release-scoped-migrations
# work
lf pr land -c
```

Start dependent work without sharing the parent's worktree:

```bash
lf task run INF-124 --stack-on INF-123
lf pr open   # targets INF-123's active branch
```

The parent Task must already have an open PR. After it merges, `lf rebase` or
`lf pr land` replays only INF-124's commits onto `main` and retargets its PR.
The selector binds to INF-123's active PR at launch; it does not follow that
Task onto a later serial PR.

The Task keeps one worktree and an ordered PR history. Each serial PR owns one
branch. Use bare `lf pr land --next <slug>` when another PR
follows; use `-c` only when this merge completes the Task. Finish clean
investigation work without a PR using `lf task complete INF-123 --summary "..."`.

Launch persists directive v1 before the provider starts. `steer` and an
interrupt with replacement advance the version; `follow-up` does not replace
current intent. A receipt distinguishes provider application from explicit
child incorporation.

`lf project run` starts or resumes the current durable KR-pursuit Session with
no branch or worktree. Completing or abandoning it preserves that history; the
next run creates one successor for the same Linear Project. It creates and
supervises Task Sessions, stops while only child progress can change the answer,
and resumes from typed Task observations. The Wave stays directly steerable
while Project and Task Sessions run.

Inspect exactly what an agent received and what Loopflow observed:

```bash
lf runs                         # skill calls with context, tokens, and cost
lf execs                        # every lf process, including lookups
lf trace <exec-id>
lf trace <exec-id> --events
lf trace <exec-id> --events --jsonl --launch <launch-id-prefix>
lf context --days 14 --wave intelligence
```

`lf trace` keeps prompt and normalized conversation artifacts below
`~/.lf/traces`; it prints paths and metadata by default, never prompt or event
bodies. `--events` explicitly reads the recorded exchange. `lf context --json`
emits turn, asset, and inclusion-decision rows for local analysis. Human
context summaries keep initial assembled context separate from follow-up input
and provider-reported history.

### Crons

Crons schedule supplementary Wave wakes. They live in `GOAL.md` frontmatter;
the Wave resident opens one system turn when a schedule is due.

```markdown
<!-- wave/governance/GOAL.md -->
---
crons:
  - flow: govern-identity
    schedule: "0 0 0 * * Sun *"
---
```

## Skills

```bash
lf debug -c    # paste an error, watch it fix
lf pm show --wave designer   # print tasks; refresh a stale snapshot when possible
lf design      # interactive design session
lf gstack/office-hours   # run a built-in gstack skill
lf office-hours          # same thing — bare name works when unambiguous
lf npx/vercel-labs/deep-research   # fetch any Claude Skill live and run it
```

Skills are prompts that run coding agents. Add your own in `.lf/skills/`.

Names resolve in this order: your repo (`.lf/skills/<name>.md`, `.lf/skills/<ns>/<name>.md`, or `.claude/commands/<name>.md`) → your global dir (`~/.lf/skills/<name>.md`, `~/.lf/skills/<ns>/<name>.md`, or `~/.claude/commands/<name>.md`) → core builtins (`build/`, `govern/`, `ops/`) → namespaced builtins (`gstack/`, …) → external skill namespaces. A bare name resolves to a namespaced builtin only when exactly one namespace has that name. Namespaced skills and flows use `/`, not `:`. For third-party skills, use `lf npx/<owner>/<repo>` (or `lf npx/<name>` once cached or searchable via `npx skills`). The legacy `rams/rams` shim also resolves when `~/.claude/commands/rams.md` exists.

Skills and flows are organized into three categories by agency: **build** (manual work you drive), **govern** (autonomous coordination the system drives), **ops** (side-channel utilities).

### Build skills (`build/`)

Manual work — you invoke these, often interactively.

| Skill | What it does |
|------|--------------|
| `kickoff` | Elaborate design — alternatives, research, imagine success/failure |
| `research` | Map the territory — architecture, complexity, quality, potential |
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

### Govern skills (`govern/`)

Autonomous coordination — crons and waves-watching-waves drive these.

| Skill | What it does |
|------|--------------|
| `scan` | Read member wave state — PRs, blocks, progress, git activity |
| `assess` | Judge wave health and identify pressure points |
| `wave-report` | Read health signals across all waves |
| `mutate` | Compose and apply coordinated mutations across member waves |
| `review` | Review mutations, amend or revert if needed |
| `s5-scan` | Scan wave identity, children, policy, and recent structural change |
| `s5-assess` | Assess identity, boundary, roster, and autonomy drift |
| `s4-scan` | Scan dependencies, advisories, upstream APIs, and other external signals |
| `s4-assess` | Assess which environmental changes matter and what they imply |
| `s3-scan` | Scan live health, velocity, CI, retries, and usage signals |
| `s3-assess` | Assess control health, mechanical blocks, and worker-pool size |
| `s2-scan` | Scan backlogs, PR overlap, path overlap, and conflict history |
| `s2-assess` | Assess coordination risk, conflict map, and safe ordering |

### Ops skills (`ops/`)

Side-channel utilities — wrappers around git, PR, release, and wave state.

| Skill | What it does |
|------|--------------|
| `init` | Set up loopflow in this repo |
| `commit` | Commit with generated message |
| `rebase` | Rebase onto main |
| `pr` | Generate PR title/body and call `lf pr open --title --body` |
| `land` | Land the PR and prune its merged worker worktree |
| `lint` | Run linter, fix issues |
| `update-wave` | Create, update, or delete wave state |
| `split-wave` | Split a wave into smaller independent waves |
| `release` | Run the full release workflow (notes, PR, tag, status) |
| `release-notes` | Write narrative `RELEASE_NOTES.md` from release context, preferring release decisions when present |
| `token-compress` | Compress text into a target token budget without silently dropping important information |
| `validate` | Validate flows, skills, and directions |

## Flows

```bash
lf design && lf implement && lf gate    # chain skills manually
lf build                                # or use a named flow
```

Skills chain into flows. Flows feed into waves.

Flows can include mechanical ops items directly:

```yaml
- implement
- gate
- op: pr land --create-pr
```

### Build flows (`build/`)

| Flow | Skills |
|------|-------|
| `build` | kickoff → review-design → implement → compress → lint → xor(demo, code-review) → gate |
| `build-or-silent` | xor(build, silence) |
| `design-and-ship` | design → implement → reduce → polish → deploy |
| `queue` | compress → update-wave → gate |
| `code` | implement → compress → lint → gate |
| `pair` | design → code |
| `deploy` | gate → op: pr land --create-pr |
| `ship` | refresh-plan → implement → gate → op: pr open → op: pr land |
| `incident` | debug → 5whys → code → deploy |

### Govern flows (`govern/`)

| Flow | Skills |
|------|-------|
| `garden` | scan → assess → xor(garden-act, silence) |
| `garden-act` | mutate → review |
| `govern-operations` | xor(s1-build, silence) |
| `govern-coordination` | s2-scan → s2-assess → mutate |
| `govern-control` | s3-scan → s3-assess → mutate |
| `govern-intelligence` | s4-scan → s4-assess → mutate |
| `govern-identity` | s5-scan → s5-assess → mutate |
| `s1-build` | kickoff → code → deploy |

### Ops flows (`ops/`)

| Flow | Skills |
|------|-------|
| `release` | op: release run patch |
| `sync` | rebase → integrate-upstream |

`deploy` lands the branch. `sync` rebases the current branch and refreshes the default branch. That default-branch refresh is safe from sibling worktrees: it stashes any dirty edits on the checked-out default branch, syncs, then restores them — but only when those edits don't touch paths the sync itself rewrote. If they collide (e.g. the branch just absorbed a merge over the same files), the edits stay in a `sync_main: auto-stash` stash instead of being merged back, so a sync can never silently revert just-landed work.

## Release artifacts

```bash
cat release/unreleased/DECISIONS.md
lf release run patch
find release -maxdepth 2 -type f | sort
```

| Path | What it does |
|------|--------------|
| `release/unreleased/DECISIONS.md` | Append-only ledger of release-worthy intent and policy decisions during the current cycle |
| `release/vX.Y.Z/DECISIONS.md` | Archived decision ledger for a shipped version |
| `release/vX.Y.Z/NOTES.md` | Snapshot of the release notes generated for that shipped version |
| `RELEASE_NOTES.md` | Always-latest release notes at the repo root |

Interactive runs append to `release/unreleased/DECISIONS.md` when they make a durable product or process decision. Headless runs do not. If the ledger exists, `lf release run` promotes `release/unreleased/` to `release/v<version>/`, uses `DECISIONS.md` to shape the narrative release notes, and archives the generated root notes to `release/v<version>/NOTES.md`. If the ledger is absent, release notes fall back to merged PR history.

### Branches (xor)

Branches route a flow based on an agent's assessment of the current state. Exactly one path runs.

```yaml
# flow: garden
- scan
- assess
- xor:
    router: assess
    paths:
      act:
        flow: garden-act
        description: "Adjustments needed — mutate waves, then review"
      silence:
        description: "Everything is healthy"
```

The `xor` construct runs a router skill that reads scratch/ and chooses a path. The router's prompt gets routing instructions appended automatically — the skill author focuses on *what to think about*, not *how to express the choice*. A path with no `flow:` or `skill:` (like `silence`) is a clean no-op exit.

If no `router:` is specified, a generic routing agent picks a path based on scratch/ contents.

## Playing in the Waves

Once you're chaining skills into flows, you're ready to ride a wave. Write its `wave/<name>/GOAL.md`, then run the agent:

```bash
lf wave engbot              # start the Wave
```

Directions compose extra nuance into any skill or flow the wave dispatches.

```bash
lf research -d ux,clarity
lf research -d ceo
```

## Install

```bash
curl -fsSL https://github.com/loopflowstudio/loopflow/releases/latest/download/install.sh | sh
```

Or grab the desktop app: download [`Loopflow-latest.dmg`](https://downloads.loopflow.studio/Loopflow-latest.dmg) and drag **Loopflow** to Applications. The app bundles `lf`.

```bash
lf              # open or focus Loopflow.app
lf desktop      # explicit alias
```

Default install location is `~/.local/bin`. Override with `LF_INSTALL_DIR=/path`.

`install.sh` downloads the `lf` binary. Run `lf init` to connect coding agents and preferences.

From a dev checkout, build everything locally with one entry:

```bash
uv run python scripts/install.py local --use   # full build: lf + Loopflow.app -> local-bin/, make active
uv run python scripts/install.py refresh       # CLI refresh: pull default branch, rebuild/install lf, sync skills
```

`install.py` is the local entry point. `local --use` builds this worktree's
`lf` and `Loopflow.app` into `<worktree>/local-bin/`, then promotes that build.
`refresh` is the fast CLI-only path: pull the default branch, rebuild/install
`lf`, and sync Loopflow skills into `~/.claude/skills` and
`~/.agents/skills`.

Built-in skills and flows included. `lf init` sets up your coding agent and preferences.

```bash
cargo install --git https://github.com/loopflowstudio/loopflow --bin lf
```
Install the Rust binaries directly with cargo.

## Execute and observe

```bash
lf wave engbot        # start the Wave (Ctrl-C to stop)
lf task run ENG-123 --name parser-recovery   # stable semantic worktree
lf task run ENG-124 --stack-on ENG-123       # separate dependent worktree
tmux ls              # list detached Wave, Project, and Task processes
lf task attach ENG-123    # writable audited control prompt
```

Read `wave/engbot/GOAL.md` and `wave/engbot/MEMORY.md` for a wave's state, or
watch it in Loopflow. To remove a wave, stop it, then delete `wave/engbot/`.

```bash
lf auth status    # provider auth status (GitHub / Claude / Codex / OpenCode Zen / Linear)
lf auth github    # connect GitHub in your browser
lf auth claude    # connect Claude in your browser
lf auth codex     # connect Codex in your browser
lf auth zen       # connect OpenCode Zen in your browser
lf auth linear     # connect Linear with OAuth
lf auth disconnect github
```

Linear refreshes OAuth automatically before expiry. Connections created before
this release may need one `lf auth linear` reconnect to record their PKCE client ID.

Register provider accounts, then route each repository through personal
profiles:

```bash
lf profile create jack@loopflow.studio --chrome-profile jack@loopflow.studio
lf profile create loopflow-eng@loopflow.studio --chrome-profile loopflow-eng@loopflow.studio
lf profile create jackstah@gmail.com --chrome-profile jackstah@gmail.com

lf auth import claude --account primary --profile jackstah@gmail.com
lf auth connect claude --account loopflow --profile jack@loopflow.studio
lf auth connect codex --account engineering
lf auth set codex engineering \
  --login-email loopflow-eng@loopflow.studio \
  --routing automatic \
  --plan max
lf auth accounts claude

lf profile account set jack@loopflow.studio claude loopflow
lf profile account set jack@loopflow.studio codex loopflow
lf profile account set loopflow-eng@loopflow.studio claude primary
lf profile account set loopflow-eng@loopflow.studio codex engineering
lf profile account set jackstah@gmail.com claude primary
lf profile account set jackstah@gmail.com codex primary

lf profile route set \
  --default jack@loopflow.studio \
  --backup loopflow-eng@loopflow.studio \
  --backup jackstah@gmail.com
lf profile route show

lf auth set claude primary --paid-through 2026-08-14
lf auth set claude primary --routing explicit-only
lf auth reset claude primary
lf auth disconnect claude --account reserve
```

Claude authorization runs through Claude in Chrome when its browser extension
is connected. If the controller is unavailable, Loopflow falls back to a hidden
terminal prompt for the one-time handoff code. `profile create
--chrome-profile` binds the profile directory, name, or signed-in email on this
host; `auth connect --profile` reuses that binding. `auth import` adopts an
existing isolated Claude credential—or the current macOS Keychain login when
the account home is empty—without another OAuth flow.

Each provider account keeps independent auth and session state under
`~/.lf/accounts/`. Profiles reuse those accounts and give each repository a
default plus ordered backups. Shared accounts are tried once and share one
cooldown. Shared Claude/Codex configuration and compiled skills stay in their
canonical host-level trees. A provider child remains pinned to its selected
profile and account for its lifetime.

`paid-through` is provider-specific. Once that date passes, automatic routing
treats the account as `explicit-only`; clear the date and set `automatic` if a
downgraded account should remain a normal fallback.

```bash
lf ssh mini -- lf wave product
```

`lf ssh` forwards the current repository's ordered profile route and each
referenced Claude or Codex credential once, even when profiles share an
account. It writes no credential files on the remote host. Provider-child
restarts inherit the routing lease and its accrued cooldowns. Before connecting,
the local provider CLIs validate or refresh each automatically eligible routed
login; `lf ssh` fails instead of sending an incomplete route. After the Wave,
tmux session, or host restarts, start or reconnect from the local machine to
resolve and forward fresh credentials.

```bash
lf status product          # inspect live execution and attention in one Wave
lf roadmap                 # read planned work across every Wave on this machine
lf roadmap --wave product  # scope the same roadmap snapshot to one Wave
lf pm show --wave product  # read the raw cached plan
```

`lf status` answers what is running and whether it is healthy. Its run history
shows the Wave's skill calls—the same context-and-token dataset as `lf runs`.
Use `lf execs` for process-level plumbing. `lf roadmap` starts from the durable
local plan and overlays live Task evidence, including clickable Linear
identifiers and Task workspaces. `lf pm` owns planning sync and mutation.

Planning lives in Linear. Pin a wave to its Linear Initiative and its own team
in `wave/<name>/GOAL.md` frontmatter — `lf pm init` writes both for you:

```yaml
# wave/designer/GOAL.md frontmatter
pm:
  provider: linear
  linear_initiative: 8c4ba3f9-cf23-4136-87ed-37847aa7dc82
  linear_team: 2d1f0a77-8b3e-4c9a-9f21-6e5d4c3b2a10
```

When no id is pinned, `lf pm init` links one exact Initiative-title match,
creates one when none exists, and fails on duplicates. It also adopts or creates
the wave's Linear **team** — the team key becomes each Task's prefix, so every
wave owns its own identifiers (`PRD-1`, `INF-1`, `INT-1`) instead of sharing one.
`--team-key` sets the prefix; a conflicting key or reused name is refused with a
recovery action rather than guessed. Both ids are stable provider identity;
keys and titles stay mutable presentation. Passing `--team-key` explicitly on
an already-bound Wave adopts that team and replaces the old binding.

```bash
lf pm init --wave designer --team-key DSG  # connect the wave to its Initiative + team
lf pm init --wave designer                 # same, defaulting the team key from the name
lf pm sync --wave designer                # refresh the local SQLite snapshot
lf pm show --wave designer                # read; refresh when the snapshot is stale
lf pm show --wave designer --no-sync      # cache-only read for agents and apps
lf pm show --wave designer --sync         # force a Linear refresh first
lf pm show --wave designer --project ui   # filter to one Linear Project
lf pm project update --wave designer --project ui --definition "..." --kr "..."
lf pm project archive --wave designer --project retired-bet
lf pm task create --wave designer --project ui --title "Add dark mode" --notes "..."
lf pm task done --id 1207... --pr "..."   # close a shipped task
lf pm reteam --wave designer              # dry-run: what would move to the team
lf pm reteam --wave designer --apply      # move open issues into the team
lf pm sync --plan                         # compare without writing SQLite
lf pm status                              # show linked waves and task counts
```

`lf pm reteam` migrates a wave's existing issues into its own team once it's
bound. It moves each active Project onto the Wave team before moving its Issues.
It **defaults to a dry run** — naming every Project and Issue it would move,
every Issue it defers, and the target team — and only mutates with `--apply`. It defers an
issue only while a Task body can write to its Session. Waiting-for-review,
blocked, and failed Tasks move safely: Linear keeps the issue id, and Loopflow
atomically reconciles the Session's new display identifier. It leaves **completed
issues in the shared team as historical** — a team move renumbers the issue
(`W2-155` becomes `PRD-<next>`, not `PRD-155`), and shipped PR/commit references
to `W2-*` are immutable. Each moved issue gets a comment recording its prior
identifier. Re-running is a no-op: issues already carrying the team key are
skipped. `lf pm doctor` flags issues still stranded in the old team.

`lf pm` maps wave → Initiative, project → Project, and task → Issue. Linear owns
project definitions and KRs in Project content. `lf pm sync` stores one local
SQLite snapshot for fast CLI, agent, and app reads; it does not write planning
files into the repo. Linear titles Projects as `<Wave> — <Project>` for its flat
Projects view; Loopflow strips that presentation prefix and keeps the concise
canonical slug (`Product — Loopflow API` remains `project:loopflow-api`). `lf pm show` serves snapshots younger than one hour, tries
a five-second refresh for older snapshots, and refuses to silently serve one
older than a week when Linear is unreachable. Use `--no-sync` for deterministic
cache-only reads. Issue descriptions and comments are Markdown, which Linear
renders natively.

Use the install script or cargo to install `lf`.

[Documentation →](docs/index.md)

## tmux Plugin

```bash
# Add to .tmux.conf
set -g @plugin 'loopflowstudio/loopflow.tmux'
run '~/.tmux/plugins/tpm/tpm'
```

Status bar shows wave state: `[lf: main]` or `[lf: 3 waves | engbot]`. Customize the format:

```bash
# .tmux.conf
set -g @loopflow_status_format '⚡#{status}'       # change wrapper
set -g @loopflow_status_format '[#{branch}]'        # branch only
set -g @loopflow_status_format '[lf: #{status}]'    # default
```

Variables: `#{status}` (computed text), `#{branch}`, `#{skill}`, `#{waves}`, `#{wave}`.

Keybindings start with `prefix+l`:

| Key | Action |
|-----|--------|
| `r` | Run skill/wave |
| `s` | Stop |
| `o` | Open logs |
| `p` | Open PR |
| `n` | New worktree |
| `d` | Land PR |
| `w` | Pick wave/worktree |
| `L` | Pick layout |
| `?` | Help |

Two built-in layouts: `lf-dev` (editor + agent + shell), `lf-swarm` (monitor + 3 worktree workers).

Works without `lf` installed — status shows placeholder, keybindings display clear messages.


## License

MIT
