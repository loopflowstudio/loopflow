# Loopflow

Loopflow helps you create and run **Waves**. Waves are chains of coding agents working together in pre-defined ways.  

Waves are first built manually through more interactive exploration. Eventually waves become autonomous through looping, scheduled cron runs, and watching for changes.

## Waves

A wave is **area × direction × flow**.

| Field | Usage | Form |
|-------|------|------|
| **Area** | Scope and context | pathset |
| **Flow** | Process followed / steps taken | sequence of prompts |
| **Direction** | Defines success, quality, and aesthetics | prompt |

```yaml
# wave/designer/designer.yaml
flow: build
mode: loop
direction:
  - ux
area:
  - designs/
triggers:
  - signal: wave
    source_wave_id: infra
    flow: build
```

### Modes

The wave's `mode` controls its execution pattern.

| Mode | Behavior | Example |
|------|----------|---------|
| **manual** | Single run | Ship one feature, run one audit |
| **loop** | Continuous until stopped | Work through a backlog, grind PRs |

### Crons

Crons schedule supplementary flows on a wave without changing its primary mode. They run independently of the worker pool, and `workers: 0` is valid for cron-driven waves.

```yaml
# member wave — workers handle the primary flow, crons sweep maintenance
flow: build
workers: 2
mode: loop
crons:
  - flow: sync
    schedule: "0 0 1 * *"

# root wave — no workers, all work comes from crons
flow: garden
workers: 0
mode: manual
crons:
  - flow: govern-identity
    schedule: "0 0 * * 0"
  - flow: govern-coordination
    schedule: "0 0 * * *"
```

### Triggers

A trigger pairs a signal (what changed) with a flow (what to run). Triggers are a list — multiple triggers of the same signal are fine.

| Signal | What changed | Default flow |
|--------|--------------|--------------|
| **repo** | Paths changed on main | `integrate` |
| **wave** | Another wave completed | `build` |
| **ci_failure** | CI failed on a wave PR | `ci-fix` |

Every new wave ships with two default triggers: `repo` (whole repo → integrate) and `ci_failure` → `ci-fix`. These don't need to be declared in the YAML.

## Steps

```bash
lf debug -c    # paste an error, watch it fix
lf op ingest --item 2-daemon-integrity.md    # move one roadmap item to scratch/
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
| `ingest` | Refresh PM-backed waves, then pick a wave item into scratch/ |
| `s5-scan` | Scan chord identity, roster, policy, and recent structural change |
| `s5-assess` | Assess identity, boundary, roster, and autonomy drift |
| `s4-scan` | Scan dependencies, advisories, upstream APIs, and other external signals |
| `s4-assess` | Assess which environmental changes matter and what they imply |
| `s3-scan` | Scan live health, velocity, CI, retries, and usage signals |
| `s3-assess` | Assess control health, mechanical blocks, and worker-pool size |
| `s2-scan` | Scan backlogs, PR overlap, area overlap, and conflict history |
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
| `build-or-silent` | op: pm pull → ingest → xor(build, silence) |
| `design-and-ship` | design → implement → reduce → polish → deploy |
| `queue` | gate → update-wave → deploy |
| `code` | implement → compress → lint → gate |
| `pair` | design → code |
| `deploy` | gate → op: land --create-pr → op: pm push-diff |
| `ship` | refresh-plan → implement → gate → op: pr → op: land |
| `incident` | debug → 5whys → code → deploy |

### Govern flows (`govern/`)

| Flow | Steps |
|------|-------|
| `garden` | scan → assess → xor(garden-act, silence) |
| `garden-act` | mutate → review |
| `govern-operations` | ingest → xor(s1-build, silence) |
| `govern-coordination` | s2-scan → s2-assess → mutate |
| `govern-control` | s3-scan → s3-assess → mutate |
| `govern-intelligence` | s4-scan → s4-assess → mutate |
| `govern-identity` | s5-scan → s5-assess → mutate |
| `s1-build` | kickoff → code → deploy |

### Ops flows (`ops/`)

| Flow | Steps |
|------|-------|
| `release` | op: release run patch |
| `sync` | rebase → integrate-upstream → op: pm pull |

`deploy` lands the branch, then syncs PM state when there is PM work to do. `sync` rebases and pulls PM state for the current branch's wave. That default-branch refresh is safe from sibling worktrees and restores dirty local edits on the checked-out default branch before the flow continues. On branches with no PM-enabled wave, or no `wave/<name>/` changes, the PM step in either flow exits cleanly.

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

Once you have played with chaining steps into flows, you're ready to ride some waves.

```bash
lfq create engbot .                # create a wave
lfq run engbot                            # ride a wave
```

Configure flow/area/direction with `loopflow.update_wave(...)`, then ride it with `loopflow.run_wave(...)`.

```bash
python - <<'PY'
import loopflow.api as loopflow

loopflow.update_wave("engbot", flow="build", direction=["ux"], area=["designs/"])
loopflow.run_wave("engbot")
PY
```

You can compose multiple directions to add additional nuance or perspectives.

```bash
lf research -d ux,clarity
lf research -d ceo
```

## Install

```bash
curl -fsSL https://github.com/loopflowstudio/loopflow/releases/latest/download/install.sh | sh
```

Default install location is `~/.local/bin`. Override with `LF_INSTALL_DIR=/path`.

`install.sh` only downloads the `lf` and `lfd` binaries. To connect Claude, GitHub, and optional providers, run `lfd install`—add `--no-interactive` to skip the prompts (CI, Docker, scripted installs).

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
lfq logs engbot      # tail agent output
lfq stop engbot      # stop a running wave
lfq delete engbot    # remove wave and history
lfq usage            # token usage summary (group by wave)
lfq usage --wave engbot  # usage for one wave (group by step)
lfq providers        # list providers with auth status and models
lf op auth status   # local provider auth for lf steps and ops
lf op auth asana    # connect Asana locally for `lf op` / step integrations
lf op auth notion   # connect Notion locally for `lf op` / step integrations
lf op auth linear  # connect Linear locally for `lf op` / step integrations
lfq auth status      # provider auth status (GitHub / Claude / Codex / OpenCode Zen / Asana / Linear)
lfq auth github      # connect GitHub in your browser
lfq auth claude      # connect Claude in your browser
lfq auth codex       # connect Codex in your browser
lfq auth zen         # connect OpenCode Zen in your browser
lfq auth asana       # connect Asana with OAuth
lfq auth linear      # connect Linear with OAuth
lfq auth notion      # connect Notion with OAuth
lfq auth disconnect github
lfq token revoke abc123   # revoke connection tokens by hash prefix
lfq token revoke --all    # revoke all connection tokens
```

PM provider config:

```yaml
# .lf/config.yaml
pm:
  provider: notion
notion:
  parent_page: 32af8f99-...  # optional: reuse an existing parent page/teamspace
  title_property: Name        # optional schema overrides
  status_property: Status
  done_value: Done
  priority_property: Priority
```

```bash
lf op branches list --user @me --stale 60d   # preview stale remote branches
lf op branches prune --user @me --stale 60d  # delete after confirmation
lf op pm init pm           # connect/create one wave project, link items, write IDs
lf op pm init --all        # bootstrap every wave/ project on the shared PM team
lf op pm pull pm           # rewrite one wave from PM; remote changes win
lf op pm pull --all        # rewrite every wave from PM; remote changes win
lf op pm export pm         # push one wave to PM; local changes win
lf op pm export --all      # push every PM-enabled wave
lf op pm push-diff pm      # push only branch-changed items to PM; no-op if wave/<name>/ is unchanged
lf op pm push-diff --all   # push-diff every PM-enabled wave
lf op pm status            # show linked waves and local/remote counts
```

Asana task descriptions preserve basic markdown formatting on sync. Loopflow writes rich text through `html_notes` and falls back to plaintext `notes` when older tasks don't have rich text yet.

PM-backed `lf op ingest` refreshes the wave from the provider before it picks an item. If the pull fails, ingest warns and falls back to the local `wave/<name>/` mirror.

Flow-driven `pm pull` and `pm push-diff` also skip cleanly when the current branch doesn't resolve to a PM-enabled wave. CI-only and non-PM branches can reuse the same flows without extra flags.
Explicit `lf op pm pull <wave>` and `lf op pm push-diff <wave>` still target the named wave. Only the flow-driven variants auto-skip.

`uv tool install loopflow` installs the Python CLI (`lfq`) and Python API only.  
Use the install script or cargo to install `lf` and `lfd`.

## Python API

```bash
uv pip install loopflow
```

```python
import loopflow.api as loopflow

loopflow.waves()
loopflow.create_wave("engbot", repo=".", flow="build", direction=["clarity"])
loopflow.create_wave("ux", repo=".", flow="build", direction=["ux"], area=["docs/"])
loopflow.create_wave("infra", repo=".", flow="govern-control", direction=["infra"], area=["rust/"])
loopflow.add_trigger("ux", signal="wave", source_wave_id="infra")
loopflow.run_wave("ux")
```

```python
import loopflow.api as loopflow

loopflow.create_wave("conductor", repo=".")
conductor = loopflow.wave("conductor")
print(conductor.primary_flow)
print(conductor.area)
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
