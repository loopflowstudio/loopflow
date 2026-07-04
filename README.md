# Loopflow

Loopflow helps you create and run **Waves** — persistent agents that work toward an outcome. You write a wave's goal once; it works a roadmap, delegates the implementation to workers, remembers what it learns, and shows you every live session.

Start a wave by hand and steer it interactively. As it earns trust, let it loop — picking work, dispatching flows, and reacting to changes on its own.

## Waves

A wave is a named agent with a goal. Two files author it:

| File | Holds |
|------|-------|
| **`wave/<name>/GOAL.md`** | The wave's intent and loop prompt — what it's for, how it judges progress |
| **`wave/<name>/MEMORY.md`** | What the wave remembers between loops — written by the wave agent |

```markdown
<!-- wave/designer/GOAL.md -->
---
primary_flow: build
metrics:
  - design reviews are complete
---

Keep the design system coherent. Each loop: read the roadmap, pick the next
design task, dispatch a worker to build it, and fold what changed into memory.
```

Run the wave agent, then watch its work:

```bash
lfq wave run designer        # start (or attach to) the wave agent
lfq sessions                 # every live session — the wave agent and its workers
lfq attach <session-id>      # jump into one over tmux
```

Run the outer loop in your terminal:

```bash
lf wave designer             # repeat lf -b goal designer --once until Ctrl-C
```

The five Viable System Model charters ship as builtin goals `s1`…`s5`. Run one directly:

```bash
lf goal s3 --once    # the s3 (control) charter, one loop
```

The wave agent coordinates; it rarely writes code itself. When it picks a substantial task it dispatches a **worker** — a scoped agent that runs a flow, opens a PR, and reports back:

```bash
lfq worker run designer --flow build --task "unify button variants"
```

Workers inherit the wave's `GOAL.md` and `MEMORY.md`, so they build with its intent in view. Their PRs are how results flow back to the wave.

### Modes

The wave's `mode` controls its execution pattern.

| Mode | Behavior | Example |
|------|----------|---------|
| **manual** | Single run | Ship one feature, run one audit |
| **loop** | Continuous until stopped | Work through a backlog, grind PRs |

### Crons

Crons schedule supplementary flows on a wave — maintenance that runs independently of the worker pool. `workers: 0` is valid for a cron-only wave.

```python
import loopflow.api as loopflow

# workers handle the primary flow; crons sweep maintenance
loopflow.create_wave("designer", repo=".", flow="build", workers=2,
                     crons=[{"flow": "sync", "schedule": "0 0 1 * *"}])

# cron-only governance wave — no workers, all work comes from schedules
loopflow.create_wave("governance", repo=".", flow="garden", workers=0,
                     crons=[{"flow": "govern-identity", "schedule": "0 0 * * 0"}])
```

### Triggers

A trigger pairs a signal (what changed) with a flow (what to run). Triggers are a list — multiple triggers of the same signal are fine.

| Signal | What changed | Default flow |
|--------|--------------|--------------|
| **repo** | Paths changed on main | `integrate` |
| **wave** | Another wave completed | `build` |
| **ci_failure** | CI failed on a wave PR | `ci-fix` |

Every new wave ships with two default triggers: `repo` (whole repo → integrate) and `ci_failure` → `ci-fix`.

## Steps

```bash
lf debug -c    # paste an error, watch it fix
lf op pm show --wave designer   # print the wave's live Asana roadmap
lf design      # interactive design session
lf gstack/office-hours   # run a built-in gstack workstyle step
lf office-hours          # same thing — bare name works when unambiguous
lf npx/vercel-labs/deep-research   # fetch any Claude Skill live and run it
lf op sync-skills       # write steps into .claude/skills and .agents/skills
```

Steps are prompts that run coding agents. Add your own in `.lf/steps/`.

`lf op sync-skills` mirrors resolved steps into vendor Skill directories so compact skill invocations work in Claude and Codex sessions (`/step` for Claude, `$step` for Codex handoffs). It writes repo-local skills by default; add `--global --yes` to write generated skills under `~/.claude/skills` and `~/.agents/skills`.

Names resolve in this order: your repo (`.lf/steps/<name>.md`, `.lf/steps/<ns>/<name>.md`, or `.claude/commands/<name>.md`) → your global dir (`~/.lf/steps/<name>.md`, `~/.lf/steps/<ns>/<name>.md`, or `~/.claude/commands/<name>.md`) → core builtins (`build/`, `govern/`, `ops/`) → namespaced builtins (`gstack/`, …) → external skill namespaces. A bare name resolves to a namespaced builtin only when exactly one namespace has that name. Namespaced steps and flows use `/`, not `:`. For third-party skills, use `lf npx/<owner>/<repo>` (or `lf npx/<name>` once cached or searchable via `npx skills`). The legacy `rams/rams` shim also resolves when `~/.claude/commands/rams.md` exists.

Steps and flows are organized into three categories by agency: **build** (manual work you drive), **govern** (autonomous coordination the system drives), **ops** (side-channel utilities).

### Build steps (`build/`)

Manual work — you invoke these, often interactively.

| Step | What it does |
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
| `scaffold` | Stand up a greenfield product skeleton from `GOAL.md` |
| `run` | Build and execute the artifact, recording observed behavior |
| `refine` | Refine existing work |
| `review-open-work` | Survey branches, PRs, worktrees, and waves for inbox-zero triage |

### Govern steps (`govern/`)

Autonomous coordination — crons, triggers, and waves-watching-waves drive these.

| Step | What it does |
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

### Ops steps (`ops/`)

Side-channel utilities — wrappers around git, PR, release, and wave state.

| Step | What it does |
|------|--------------|
| `init` | Set up loopflow in this repo |
| `commit` | Commit with generated message |
| `rebase` | Rebase onto main |
| `pr` | Generate PR title/body and call `lf op pr --title --body` |
| `land` | Land PR, rotate worktree |
| `lint` | Run linter, fix issues |
| `update-wave` | Create, update, or delete wave state |
| `split-wave` | Split a wave into smaller independent waves |
| `release` | Run the full release workflow (notes, PR, tag, status) |
| `release-notes` | Write narrative `RELEASE_NOTES.md` from release context, preferring release decisions when present |
| `synthesize` | Combine multiple perspectives into one |
| `token-compress` | Compress text into a target token budget without silently dropping important information |
| `validate` | Validate flows, steps, and directions |

## Flows

```bash
lf design && lf implement && lf gate    # chain steps manually
lf build                                # or use a named flow
```

Steps chain into flows. Flows feed into waves.

Flows can include mechanical ops items directly:

```yaml
- implement
- gate
- op: land --create-pr
```

### Build flows (`build/`)

| Flow | Steps |
|------|-------|
| `build` | kickoff → review-design → loop(code → xor(demo, code-review), exit: gate) → deploy |
| `build-or-silent` | xor(build, silence) |
| `design-and-ship` | design → implement → reduce → polish → deploy |
| `greenfield` | scaffold → implement → run → gate |
| `queue` | compress → update-wave → gate |
| `code` | implement → compress → lint → gate |
| `pair` | design → code |
| `deploy` | gate → op: land --create-pr |
| `ship` | refresh-plan → implement → gate → op: pr → op: land |
| `incident` | debug → 5whys → code → deploy |

### Govern flows (`govern/`)

| Flow | Steps |
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

| Flow | Steps |
|------|-------|
| `release` | op: release run patch |
| `sync` | rebase → integrate-upstream |

`deploy` lands the branch. `sync` rebases the current branch and refreshes the default branch. That default-branch refresh is safe from sibling worktrees: it stashes any dirty edits on the checked-out default branch, syncs, then restores them — but only when those edits don't touch paths the sync itself rewrote. If they collide (e.g. the branch just absorbed a merge over the same files), the edits stay in a `sync_main: auto-stash` stash instead of being merged back, so a sync can never silently revert just-landed work.

## Release artifacts

```bash
cat release/unreleased/DECISIONS.md
lf op release run patch
find release -maxdepth 2 -type f | sort
```

| Path | What it does |
|------|--------------|
| `release/unreleased/DECISIONS.md` | Append-only ledger of release-worthy intent and policy decisions during the current cycle |
| `release/vX.Y.Z/DECISIONS.md` | Archived decision ledger for a shipped version |
| `release/vX.Y.Z/NOTES.md` | Snapshot of the release notes generated for that shipped version |
| `RELEASE_NOTES.md` | Always-latest release notes at the repo root |

Interactive runs append to `release/unreleased/DECISIONS.md` when they make a durable product or process decision. Headless runs do not. If the ledger exists, `lf op release run` promotes `release/unreleased/` to `release/v<version>/`, uses `DECISIONS.md` to shape the narrative release notes, and archives the generated root notes to `release/v<version>/NOTES.md`. If the ledger is absent, release notes fall back to merged PR history.

### Browse the catalog

```bash
lfd serve
curl -s "http://127.0.0.1:2486/v0/catalog?repo=$(pwd)" | jq '.result.flows[] | {name, category, source}'
```

Open **Flows** in Concerto to browse the same catalog visually. The left pane groups flows and steps by `build`, `govern`, and `ops`; the right pane shows every parent flow that uses the selected flow or step.

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

The `xor` construct runs a router step that reads scratch/ and chooses a path. The router's prompt gets routing instructions appended automatically — the step author focuses on *what to think about*, not *how to express the choice*. A path with no `flow:` or `step:` (like `silence`) is a clean no-op exit.

If no `router:` is specified, a generic routing agent picks a path based on scratch/ contents.

## Playing in the Waves

Once you're chaining steps into flows, you're ready to ride a wave. Write its `wave/<name>/GOAL.md`, then run the agent:

```bash
lfq wave run engbot        # start (or attach to) the wave agent
lfq list                   # see every wave
```

Or drive it from Python:

```bash
python - <<'PY'
import loopflow.api as loopflow

loopflow.create_wave("engbot", repo=".", flow="build")
loopflow.run_wave("engbot")
PY
```

Directions compose extra nuance into any step or flow the wave dispatches.

```bash
lf research -d ux,clarity
lf research -d ceo
```

## Install

```bash
curl -fsSL https://github.com/loopflowstudio/loopflow/releases/latest/download/install.sh | sh
```

Or grab the desktop app: download [`Loopflow-latest.dmg`](https://downloads.loopflow.studio/Loopflow-latest.dmg) and drag **Loopflow** to Applications. The app bundles `lf` and `lfd`.

Default install location is `~/.local/bin`. Override with `LF_INSTALL_DIR=/path`.

`install.sh` only downloads the `lf` and `lfd` binaries. To connect Claude, GitHub, and optional providers, run `lfd install`—add `--no-interactive` to skip the prompts (CI, Docker, scripted installs).

From a dev checkout, build everything locally with one entry:

```bash
uv run python scripts/install.py local --use   # full build: lf, lfd, Loopflow.app -> local-bin/, make active
uv run python scripts/install.py refresh       # CLI refresh: pull default branch, rebuild/install lf+lfd, sync skills
```

`install.py` is the local entry point. `local --use` builds this worktree's `lf`, `lfd`, and `Loopflow.app` into `<worktree>/local-bin/`, then promotes that build. `refresh` is the fast CLI-only path: pull the default branch, rebuild `lf`/`lfd`, install them into the local bin dir, and sync loopflow steps into `~/.claude/skills` and `~/.agents/skills`. Both paths run `lf op sync-skills --global --yes` after installing, so Claude and Codex always see the latest steps.

Built-in steps and flows included. `lf init` sets up your coding agent and preferences.

```bash
cargo install --git https://github.com/loopflowstudio/loopflow --bin lf --bin lfd
```
Install the Rust binaries directly with cargo.

## Query lfd (lfq)

```bash
uv tool install loopflow
lfq                  # status overview
lfq list             # list waves
lfq show engbot      # show wave details
lfq wave run engbot  # start or attach the Wave-agent session
lfq worker run engbot --flow implement --task "Add the endpoint"
lfq whoami           # show current lfd agent identity
lfq sessions         # list live terminal sessions
lfq attach <id>      # attach to one over tmux
lfq logs engbot      # tail agent output
lfq stop engbot      # stop a running wave
lfq delete engbot    # remove wave and history
lfq providers        # list providers with auth status and models
lf op auth status   # local provider auth for lf steps and ops
lf op auth asana    # connect Asana locally for `lf op` / step integrations
lfq auth status      # provider auth status (GitHub / Claude / Codex / OpenCode Zen / Asana)
lfq auth github      # connect GitHub in your browser
lfq auth claude      # connect Claude in your browser
lfq auth codex       # connect Codex in your browser
lfq auth zen         # connect OpenCode Zen in your browser
lfq auth asana       # connect Asana with OAuth
lfq auth disconnect github
```

The roadmap lives in Asana. Pin a wave to its Asana project in `wave/<name>/GOAL.md` frontmatter — `lf op pm init` writes this for you:

```yaml
# wave/designer/GOAL.md frontmatter
pm:
  asana_project: 1207xxxxxxxxxxxx
```

```bash
lf op branches list --user @me --stale 60d   # preview stale remote branches
lf op branches prune --user @me --stale 60d  # delete after confirmation
lf op pm init --wave designer                # connect/create the Asana project, write asana_project into GOAL.md
lf op pm show --wave designer                # print the wave's live Asana roadmap
lf op pm update --wave designer --title "Add dark mode" --notes "..."   # create a task
lf op pm update --wave designer --id 1207... --title "..." --status done # update or close a task
lf op pm status                              # show linked waves
```

`lf op pm` reads and edits the roadmap directly in Asana — there is no local mirror and nothing to sync. Task notes preserve basic markdown formatting: Loopflow writes rich text through `html_notes` and falls back to plaintext `notes` when a task has none yet.

`uv tool install loopflow` installs the Python CLI (`lfq`) and Python API only.  
Use the install script or cargo to install `lf` and `lfd`.

## Python API

```bash
uv pip install loopflow
```

```python
import loopflow.api as loopflow

loopflow.waves()
loopflow.create_wave("engbot", repo=".", flow="build")
loopflow.create_wave("ux", repo=".", flow="build")
loopflow.create_wave("infra", repo=".", flow="govern-control")
loopflow.add_trigger("ux", signal="wave", source_wave_id="infra")
loopflow.run_wave("ux")
```

```python
import loopflow.api as loopflow

loopflow.create_wave("conductor", repo=".")
conductor = loopflow.wave("conductor")
print(conductor.primary_flow)
```

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

Variables: `#{status}` (computed text), `#{branch}`, `#{step}`, `#{waves}`, `#{wave}`.

Keybindings start with `prefix+l`:

| Key | Action |
|-----|--------|
| `r` | Run step/wave |
| `s` | Stop |
| `o` | Open logs |
| `p` | Open PR |
| `n` | Next iteration |
| `d` | Land PR |
| `u` | Start/bootstrap |
| `w` | Pick wave/worktree |
| `L` | Pick layout |
| `?` | Help |

Two built-in layouts: `lf-dev` (editor + agent + shell), `lf-swarm` (monitor + 3 worktree workers).

Works without `lf` or `lfq` installed — status shows placeholder, keybindings display clear messages.


## License

MIT
