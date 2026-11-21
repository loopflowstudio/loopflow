# Configuration

Configure Loopflow with `lf set` or by editing YAML files directly. Every
setting works across four scopes (your global defaults, your private repo/Wave
overrides, committed repo defaults, and committed Wave defaults) and can
target a single Skill or a single step inside a Graph.

## Common recipes

### Mix harnesses by Skill (e.g. Claude for design, Codex for implementation)

```bash
lf set --global default_agent codex
lf set --global skill.design.agent claude
lf set --global skill.implement.agent codex
```

Or in `~/.lf/config.yaml`:

```yaml
default_agent: codex
skill:
  design:
    agent: claude
  implement:
    agent: codex
```

- `agent` pins a choice (wins over any `default_agent`, and stays on that
  harness instead of falling back through `agents`).
- `default_agent` sets the preferred starting agent: it jumps to the front of
  `agents` while keeping the rest of `agents` behind it as fallbacks, and yields
  to any applicable `agent` pin.
- `agents` is the ordered list of providers/models Loopflow falls back through.

### Keep going when a provider runs out

```bash
lf set --global agents claude,codex
lf set --global skill.compress.default_agent codex
```

```yaml
# ~/.lf/config.yaml
agents: [claude, codex]                # or order specific models: [claude:opus, codex, claude:fable]
skill:
  compress:
    default_agent: codex               # tries Codex first, falls back to Claude
```

When headless work (Graph steps, `lf -b <skill>`, commit messages, PR titles,
or sync conflict resolution) hits a usage limit or unavailable model, Loopflow
moves to the next entry in `agents` and logs why:

```text
codex: provider usage limit; continuing on claude:fable
```

- **Graph steps** also skip entries whose CLI isn't installed on this Machine.
  If every entry hits a usage limit, a Graph step waits 5, 15, 30, then 60
  minutes, retrying from the first entry after each wait before stopping.
- **Other headless commands** stop once the list is exhausted (commit message
  generation falls back to a default title).
- **Interactive Sessions** keep refusals visible and wait for you to retry or
  switch with `lf -a codex resume <session>`.
- **Single-entry bare `claude`** (`agents: [claude]` or `default_agent: claude`)
  tries Claude's other models (Opus → Sonnet → Haiku). Pinning a model
  (`claude:opus`) stays on that model.
- **Every harness** moves on the same three refusals: usage spent, a login
  revoked, a model it can't serve. OpenCode out of credits moves on with
  `opencode_error: Insufficient balance…`.

Scope `agents` per repo, Wave, or Skill; a narrower `agents` replaces the
wider one as a unit:

```bash
lf set agents codex,claude                   # this repo: Codex first
lf --wave frontend set agents claude         # this Wave: Claude only
lf set --global skill.review.agents codex    # review: Codex only, everywhere
```

### Private vs. shared repo and Wave settings

```bash
lf set skill.design.agent claude                     # private repo preference (.lf/config.local.yaml)
lf --wave frontend set skill.implement.agent claude  # private Wave preference
lf --wave frontend set --shared default_agent claude # committed to .lf/wave/frontend/config.yaml
lf unset skill.design.agent                          # remove override and reveal the next value
```

### Target one step inside a Graph

Give a Graph step an `id:` to configure that occurrence independently:

```yaml
# .lf/graphs/release.yaml
- step: {name: implement, id: write-code}
- gate
- step: {name: implement, id: fix-review}
```

```bash
lf set graphs.release.steps.fix-review.agent claude
```

### Override a single command

```bash
lf -a claude design                                  # one conversation
lf -a codex graph release                            # every step in this Graph run
lf --setting 'skill.implement.yolo=false' implement  # any typed setting
LF_SETTINGS='{skill: {design: {agent: claude}}}' lf design
```

### Give different work different context

```yaml
# .lf/config.yaml
context:
  docs: [README.md]
  exclude: [docs/generated/**]
  diff: true
skill:
  design:
    context:
      docs: [docs/product.md]
  implement:
    context:
      docs: [scripts/testing.md]
graphs:
  release:
    steps:
      fix-review:
        chrome: false
```

`context.docs` and `context.exclude` are additive across scopes; `[]` clears
earlier contributions.

## Inspect effective settings

```bash
lf config settings --skill implement           # effective values and where they came from
lf config settings --skill implement --explain # every candidate, file, and why it won
lf config settings --skill implement --all     # include built-in defaults
lf config settings --graph release --step write-code --json
lf config catalog --json                       # every supported key, type, and scope
lf context --json                              # context budgets and current usage
```

`lf set` validates keys and types strictly and preserves comments in YAML
files. Desktop's **Configuration** panel reads and writes the same files with
the same Explain and Unset controls.

## Where settings live

| File | Applies to | Committed? |
|---|---|---|
| `~/.lf/config.yaml` (`LF_HOME` overrides `~/.lf`) | Your global preferences | No |
| `.lf/config.local.yaml` | Your private repo and Wave preferences (shared across Task worktrees, auto-excluded from Git) | No |
| `.lf/config.yaml` | Team repo defaults | Yes |
| `.lf/wave/<name>/config.yaml` | Team Wave defaults | Yes |

## Which value wins

Each setting resolves independently:

1. Command-line flags (`-a`, `--setting`)
2. Environment (`LF_SETTINGS`, `LF_AGENT_OVERRIDE`, `LF_YOLO_OVERRIDE`, `LF_CHROME_OVERRIDE`, `LF_USER_NAME`)
3. Personal config (`.lf/config.local.yaml`, `~/.lf/config.yaml`)
4. Repository config (`.lf/wave/<name>/config.yaml`, `.lf/config.yaml`, Skill frontmatter, Graph step)
5. Built-in defaults

Within one source: **Wave > repository > global**, and **Graph step > Skill >
general**.

For agent selection, every applicable `agent` (pinned choice) beats every
`default_agent` (preferred starting agent). Within each class, the order above
applies. `default_agent` jumps to the front of `agents`, keeping the rest of
`agents` behind it as fallbacks, whenever its declaration ranks at least as
high as `agents`.

| Declarations | Effective order |
|---|---|
| Personal `default_agent: codex`; repo implement `agent: claude` | Claude |
| Personal implement `agent: codex`; repo step `agent: claude` | Codex |
| Personal `agents: [claude, codex]`; personal implement `default_agent: codex` | Codex, then Claude |
| Personal `agents: [claude, codex]`; repo implement `default_agent: codex` | Claude, then Codex |
| Personal global `default_agent: claude`; personal review `agents: [codex]` | Codex |

## Quick reference

| Setting | CLI flag | Config key |
|---|---|---|
| Harness / model | `-a claude:opus` | `agent: claude:opus` · `default_agent` · `agents` |
| Tracker | — | `tracker: {linear: {team: "<id>"}}` or `{asana: {team: "<gid>"}}`; without one, the plan [travels with the repository](work.md#share-the-plan-with-your-team) |
| Run mode | `-i` (interactive) / `-b` (headless) | Interactive on direct TTY; headless in Graphs |
| Include docs | `--docs README.md,docs/` | `context: {docs: [README.md, docs/]}` |
| Changed file paths | `--diff files` | `context: {diff_files: true}` |
| Raw diff reference | `--diff patch` | `context: {diff: true}` |
| Exclude from LF context | — | `context: {exclude: ["*.test.ts", dist]}` |
| Hide from a Task's Diff in Desktop | — | `diff_hidden: ["scratch/**"]` (adds across layers; `[]` clears) |
| Headings of a Task's Diff in Desktop | — | `diff_groups` (replaces as a unit) |
| Clipboard | `-c, --clipboard` | `context: {paste: true}` |
| Claude Chrome tool | `--chrome on` / `--chrome off` | `chrome: true` / `false` |
| Permission prompts | `--yolo` (skip) · `--setting yolo=false` (ask) | `yolo: false` (ask when attended; default skips) |
| Merge method | — | `merge_method: squash` (`merge`, `squash`, `rebase`) |
| Display name | — | `user.name` (defaults to Git `user.name`) |

## Harness-specific settings

### Claude model

```bash
lf set --global claude.model opus
```

Sets the model used whenever bare `claude` is selected (overridden by an
explicit `claude:fable` or `claude:opus`). Existing Sessions keep their
recorded model until resumed with `-a claude`.

### Codex service tier

```bash
lf set --global codex.service_tier default     # default (standard) or priority (fast)
lf --setting codex.service_tier=priority -a codex implement
```

Leaves `~/.codex/config.toml` untouched. With no preference set, Codex's own
config decides.

### OpenCode

OpenCode model names use `provider/model` form:

```yaml
agent: opencode                          # OpenCode's default model
agent: opencode:opencode/glm-5.2
agent: opencode:moonshotai/kimi-k2
```

### Pi

```bash
npm install -g --ignore-scripts @earendil-works/pi-coding-agent@1.3.0
pi                                       # configure model and /login
lf -a pi implement
lf -b -a pi:anthropic/claude-sonnet-4-6 implement
```

Pi 1.3.0 is the baseline. Pi owns its own credentials and session files
(API keys in your environment or Doppler work automatically). Headless runs use
Pi RPC; interactive runs keep Pi's native terminal UI. Pi does not sandbox
tools and does not support cross-harness history transfer.

### Antigravity

```bash
brew install --cask antigravity-cli     # installs the `agy` command
agy                                     # sign in
lf -a agy implement
lf -b -a agy:gemini-3.8-flash-high implement
lf task run LOO-12 -a agy               # every step of the Task
```

Bare `agy` uses the model `agy` is set to; `agy:<model>` takes a name from
`agy models`. Antigravity owns its
sign-in and conversations under `~/.gemini/antigravity-cli`; Loopflow gives
each launch its instructions and hooks in a folder under the checkout's
`.lf/prompts/` and writes nothing to `~/.gemini`. A step gets the Task's
context and hears `lf steer` after its next tool. A spent quota, a signed-out
`agy` or an unserved model moves a Graph step to the next `agents` entry:

```bash
lf task status LOO-12
#   implement went on under claude after: agy_error: invalid model selection (--model "gemini-9" --effort ""): model gemini-9 is not recognized as a known model or custom model in settings
```

Under `yolo: false`, `agy` runs terminal commands in its own sandbox, which
does not reach `~/.lf`. The first `lf` command a conversation runs fails
there, and `agy` asks "Run this command?" to run it outside: answer `2` to
allow `lf` for the rest of the conversation. With `yolo` on, the default, and
on every headless step, nothing is sandboxed and nothing asks.

`/new`, `/fork` and `/resume` in the terminal keep the conversation on lf: the
one you switch to gets the context block and your steers from its first
prompt, and `lf resume` reopens the one you quit in.

Usage counts the conversation lf started, not its subagents. A plain `agy`
conversation is not listed in `lf session list`. Native response schemas, turn
limits and `--effort` are unavailable.

### Supported harnesses

Limit which harnesses appear in pickers (additive across global and repo
config):

```yaml
supported_harnesses: [claude, codex, opencode, pi, agy]
```

## Permissions (`yolo`) and worktree access

Agents act without asking permission. With no `yolo` setting, Claude, Codex,
OpenCode and Antigravity run every edit and shell command straight through; Pi
keeps its own tool policy.

```bash
lf implement                              # no prompts
lf --setting yolo=false implement         # this conversation asks, as the vendor configures
lf set yolo false                         # every conversation you attend in this repository asks
lf --yolo implement                       # no prompts for one launch, over a configured `yolo: false`
```

`yolo: false` brings back each vendor's own config and prompts in a
conversation you attend: `lf`, `lf <skill>`, `lf task open`, `lf resume`.
Unattended runs (`lf -b`, Task steps, `lf task auto on`, commit and PR text)
never ask, whatever `yolo` says, because nobody is there to answer. A
`yolo: false` on a skill or Graph step applies when someone opens that step.

| | you attend | unattended |
|---|---|---|
| `yolo` unset or `true` | no prompts | no prompts |
| `yolo: false` | the vendor's own prompts | no prompts |

"No prompts" is Claude's `--dangerously-skip-permissions`, Codex's
`danger-full-access` sandbox with `approval_policy = "never"`, OpenCode's
allow-all permissions, and Antigravity's `--dangerously-skip-permissions`. lf
has no sandbox of its own: an agent can do anything the OS user running `lf`
can. To contain untrusted work, run it on a dedicated Machine, VM or container
([Security](security.md#choose-the-execution-boundary-first)).

**Worktree access:** Claude, Codex and Antigravity conversations you attend
from a Git worktree get write access to the main repo's `.git` metadata, so
Git commands work under `yolo: false` too.

## Context budgets and assembly

```bash
lf context                         # budgets, sources, and current usage
lf context --wave billing          # measure one Wave's memory
```

```yaml
# .lf/config.yaml or .lf/wave/<name>/config.yaml
context:
  budgets:
    memory_tokens: 10000           # default: 16,000 tokens / 128 KiB (memory_bytes)
```

Budgets are authoring targets (exceeding them flags `*` in
`lf session usage --context` but never blocks a launch).

What enters a launch:
- Providers load `AGENTS.md` natively.
- Loopflow adds `LOOPFLOW.md` (the operating contract), `scratch/` Markdown,
  and the active Wave's `GOAL.md`/`MEMORY.md` (plus ancestor Waves) in a
  refreshed block that survives compaction.
- `--docs` / `context.docs` enter as complete UTF-8 bodies;
  `--diff` references Git inspection commands; `-c` appends the clipboard.
- `scratch/` notes written during a branch are cleaned up automatically by
  `lf pr submit` and `lf land` (`lf scratch clear` removes added files while
  keeping pre-existing files, edits, and `.gitkeep`).

## Skill and Graph discovery

Skills are discovered in order: **repository > personal (`~/.lf/skills`) >
builtins**. Within each scope, Loopflow checks `.lf/skills`, `.claude/skills`,
`.claude/commands`, `.agents/skills`, `.codex/skills`, and `.codex/prompts`
(supporting both single `.md` files and `<name>/SKILL.md` bundles). Inspect
resolution with `lf list skill` and `lf help <skill>`.

Export Loopflow skills and Graphs into native Claude/Codex skill directories:

```bash
lf self sync-skills --yes   # personal + builtins → ~/.claude/skills and ~/.agents/skills
lf self sync-skills --repo  # repo skills + Graphs → checkout directories
```

## Remote work

```bash
lf set machine.tasks '<machine-id>'   # default Machine for new Tasks
```

Work launched from your laptop onto a remote Machine carries your laptop's
personal preferences; work started directly on the remote uses the remote's
saved preferences. Credentials never forward automatically; opt in per
variable or home file with `forward.env` and `forward.files`
([Remote Machines and rentals](subscriptions.md#remote-machines-and-rentals)).

## Releases

Declare per-target release hooks in `.lf/config.yaml`:

```yaml
release:
  targets:
    cli:
      area: [packages/cli/]
      tag_prefix: cli/
      manifests: [packages/cli/package.json]   # omit to auto-detect
      verify: [scripts/check-release cli]
      prepare: [scripts/prepare-release cli {version}]
      workflow: .github/workflows/release-cli.yml
      completion: github-release               # tag | workflow | github-release
```

`verify` and `prepare` accept `{target}`, `{version}`, and `{previous_tag}`.
The first release of a target takes an explicit `X.Y.Z`; later releases take
`patch`, `minor`, or `major`.

## Merge policy

```yaml
# .lf/config.yaml (optional; otherwise picks squash → merge → rebase from GitHub's allowed methods)
merge_method: squash
```

If your repo uses a GitHub merge queue, the queue owns the merge method and
Loopflow sends no method override.

## FAQ

**How do worktrees and separate clones share preferences?**
Task worktrees share their main checkout's `.lf/config.local.yaml`. Separate
clones of the same repo keep independent private preferences. A
`lf.config.local.yaml` left in the checkout root by an earlier release moves
there the first time LF looks for it.

**What happens when a Wave is renamed?**
Private Wave preferences are keyed by the Wave's UUID, so renaming or moving a
Wave keeps its preferences intact, and reusing a retired Wave name starts clean.

**What stays in files vs. the database?**
YAML files hold declared preferences (agents, context, machines, pools,
rentals). SQLite (`~/.lf/loopflow.db`, `~/.lf/history.db`) holds live state,
Tasks, Sessions, turns, and usage. Desktop pane layouts live locally on each
Mac.

**Migrating older preferences?**
Run `lf config migrate` to inspect or apply legacy preference migrations, or
`lf config retained` to reconcile remote-retained profiles.

Next: [Native authentication](subscriptions.md) · [Security](security.md).
