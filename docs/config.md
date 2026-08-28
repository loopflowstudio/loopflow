# Configuration

Start with a one-run override; keep it in repo config only when the choice
should apply to everyone:

```bash
lf gate -m codex --docs docs/api.md
```

```yaml
# .lf/config.yaml
agent: codex
docs: [docs/api.md]
```

CLI flags override repo config (`.lf/config.yaml`), which overrides global
config (`~/.lf/config.yaml`). Additive lists such as `docs` combine across
config files.

## Quick Reference

| Behavior | CLI Flag | Config |
|----------|----------|--------|
| Model | `-m claude:opus` | `agent: claude:opus` |
| Human-present TUI | direct TTY or `-i` | `session.launch: tui` |
| Include docs | `--docs README.md,docs/` | `docs: [README.md, docs/]` |
| Include branch files | `--diff-files` | `diff_files: true` |
| Include raw diff | `--diff` | `diff: true` |
| Include clipboard | `-c, --clipboard` | — |
| Disable Loopflow guidance | `--no-loopflow` | — |
| Context files | — | `context: [FILE]` |
| Direction (judgment/intent) | `--direction NAME` | `direction: NAME` |
| Chrome automation | `--chrome` | `chrome: true` |
| Yolo mode (skip permissions) | — | `yolo: true` |
| Claude/Codex/OpenCode launch surface | `--tui` / `--ide` | `session.launch: tui` |
| Detached Ask terminal | `LF_EXTERNAL_TERMINAL=Ghostty` | global-only `session.terminal: Ghostty` |

## Context Assembly

Every skill gets context assembled automatically. Run any command to see the breakdown:

```
Tokens: 12,847

docs           3,842 ███
  README.md      988 █
scratch        3,050 ██
clipboard      1,234 █
```

The token breakdown shows what's included:

| Section | What it contains | Config |
|---------|------------------|--------|
| **files** | Agent doc (AGENTS.md/CLAUDE.md/STYLE.md), `LOOPFLOW.md`, `scratch/`, `wave/` | always on; `--no-loopflow` drops `LOOPFLOW.md` |
| **scratch** | `scratch/` design artifacts | always included |
| **wave** | `wave/` docs | always included |
| **docs** | Explicit docs files, globs, and directory markdown walks | `docs:` |
| **diff** | Branch diff when requested | `--diff` |
| **diff_files** | Files changed on this branch when requested | `diff_files: true` |
| **summary** | Token-limited codebase overviews | `summaries:` in config |
| **clipboard** | Pasted content (errors, context) | `-c` flag |

Defaults work well for most repos. Summaries require configuration.

## Config Files

**Global config** (`~/.lf/config.yaml`) sets user-wide defaults. **Repo config** (`.lf/config.yaml`) overrides for that repo.

For most settings, repo overrides global. For additive settings (`docs`, `context`, `exclude`, `summaries`, `supported_harnesses`), lists combine from both.

```yaml
# ~/.lf/config.yaml (global)
agent: claude:opus
direction: clarity
session:
  terminal: Ghostty       # opens `lf ask` sessions on this Home

# .lf/config.yaml (repo)
agent: codex        # overrides global
context:
  - docs/api.md           # combined with global context
```

Example repo config:

```yaml
agent: claude:opus

session:
  launch: tui

direction: clarity

context:
  - src/schema.py
  - docs/api.md

docs:
  - README.md
  - docs/

exclude:
  - "*.test.ts"
  - node_modules
```

## Flows

Flows are YAML files in `.lf/flows/`:

```yaml
# .lf/flows/ship-api.yaml
- implement
- compress
- gate
```

---

## Releases

Keep the lifecycle in Loopflow and the repository-specific work in commands the
repository owns:

```yaml
release:
  targets:
    cli:
      area: [packages/cli/]
      tag_prefix: cli/
      manifests: [packages/cli/package.json]
      verify:
        - scripts/check-release cli
      prepare:
        - scripts/prepare-release cli {version}
      workflow: .github/workflows/release-cli.yml
      completion: github-release
```

`lf release run patch --target cli` selects changes from the exact
`cli/v<previous>..HEAD` git range, prepares an isolated release PR, tags its
merged commit only after the configured workflow proves that exact candidate,
and waits for the configured completion evidence. `area` scopes the range.
`manifests` use Loopflow's built-in semantic-version adapters; omit them to
auto-detect supported manifests.

`verify` runs during `lf release run`, after Loopflow resolves the version and
exact change range but before it prepares release changes. `lf release check`
only reads that evidence; it does not execute repository hooks. `prepare` runs
after manifest bumps inside the isolated release worktree. Both hook types
accept `{target}`, `{version}`, and `{previous_tag}` placeholders. The
configured workflow runs before the version tag and owns credential-free
compilation, packaging, migration checks, and smoke tests. A configured
publisher owns host signing and candidate preparation before the tag, then
registry upload, deployment, and finalization after it. Keep those details in
repo-owned commands—not in built-in release policy.

Completion is explicit:

- `tag` — pushing the tag completes the release.
- `workflow` — the configured pre-tag candidate workflow must succeed.
- `github-release` — after candidate proof and tagging, a GitHub Release for
  the tag must exist.

Without `completion`, targets with `workflow` use `workflow`; other targets use
`tag`. The first release requires an explicit `X.Y.Z`; bump keywords require a
previous target tag.

---

## Options Reference

### Loopflow Guidance

Ambient operating guidance for inline execution and mechanical git/PR operations. Injected by default; tier skills add scoped delegation.

| | |
|---|---|
| **CLI** | `--no-loopflow` |
| **Default** | included |

Use `--no-loopflow` when you want a leaner prompt without loopflow-specific process guidance.

### Docs

Prefetch specific files, globs, or directories into context. Not included by default.

| | |
|---|---|
| **CLI** | `--docs PATH[,PATH...]` |
| **Config** | `docs: [PATH, PATH]` |
| **Default** | none (empty) |

Each entry is a file (`README.md`), a glob (`'*.md'`), or a directory (`swift/`
gathers `*.md` under it). Use this to pull in reference docs relevant to the
task—it doesn't restrict which files the agent can edit. `scratch/` and
`wave/` are always included automatically; you don't need `--docs` for them.

### Branch Files (diff_files)

Full content of files modified on the current branch.

| | |
|---|---|
| **CLI** | `--diff-files` / `--no-diff-files` |
| **Config** | `diff_files: true` |
| **Default** | `false` |

Use `--diff-files` when the agent needs complete file bodies, not just line changes. Combine with `--diff` when the exact patch also matters.

### Clipboard

Paste content (errors, stack traces, context) into the prompt.

| | |
|---|---|
| **CLI** | `-c, --clipboard` |
| **Default** | not included |

Use `-c` when debugging: copy an error, then `lf debug -c`.

### Raw Diff

Include `git diff main...HEAD` output showing exact line changes.

| | |
|---|---|
| **CLI** | `--diff` / `--no-diff` |
| **Config** | `diff: true` |
| **Default** | `false` (not included) |

Use when you want the agent to see precisely what changed. Can combine with `--diff-files`.

### Context Files

Additional files always included in every skill.

| | |
|---|---|
| **Config** | `context: [src/schema.py, docs/api.md]` |

Config sets baseline files for all skills.

### Exclude Patterns

Glob patterns to exclude from file listings.

| | |
|---|---|
| **Config** | `exclude: ["*.test.ts", node_modules, dist]` |

---

### Agent

Set the default harness, with an optional model.

| | |
|---|---|
| **CLI** | `lf gate -m codex:o3` |
| **Config** | `agent: claude:opus` (optional) |
| **Default** | unset (resolution falls back to skill defaults, then `codex`) |

```yaml
agent: codex          # harness default
# agent: claude:opus  # harness plus model
```

Harnesses: `claude`, `codex`, `gemini`, `opencode`. Use `harness:model` for specific models.

Four built-in skills intentionally default to Claude: `kickoff`,
`review-design`, `review-slice`, and `prompt`. Every other unconfigured
built-in skill defaults to Codex. A CLI `-m` or authored `agent:` config remains
an explicit override.

Loopflow starts every Codex CLI and interactive run on the standard service tier,
even when the user's Codex config selects Fast mode. In an interactive Codex
TUI, run `/fast` to opt into Fast mode for that session.

Gemini is supported for direct `lf` commands. Wave, Project, and Tasks
require `claude`, `codex`, or `opencode`.

OpenCode model strings use `provider/model` form. Bare `opencode` resolves to
the Loopflow-owned `opencode/glm-5.2` default, which Loopflow sends explicitly
so OpenCode config cannot silently fall back to a lower-capability model:

```yaml
agent: opencode                          # Loopflow default: opencode/glm-5.2
agent: opencode:opencode/glm-5.2         # same default, explicit
agent: opencode:moonshotai/kimi-k2       # explicit provider/model
```

### Supported Harnesses

Optional list of harnesses exposed in Loopflow's model picker and settings.

```yaml
supported_harnesses:
  - claude
  - codex
  - gemini
```

This list is additive across global and repo config.

### Run Mode

Direct named invocations use a present-human session when stdin or stdout is a
TTY. Automated flow nodes and `--batch` invocations run headlessly. Skill
frontmatter never changes scheduling.

| | |
|---|---|
| **CLI** | `-i` (interactive), `-b` (batch/headless) |
| **Default** | present-human for a direct TTY; headless otherwise |

Flows declare a required User gate on the exact skill occurrence with a stable
`id` and `human: true`; see [Authoring](authoring.md#flows).

### Direction

Directions shape judgment and intent—how the coding agent approaches work.

| | |
|---|---|
| **CLI** | `--direction ux` or `--direction ux,clarity` |
| **Config** | `direction: clarity` or `direction: [ux, clarity]` |

Direction files live in `.lf/directions/` as markdown. Built-in direction groups:
`infra`, `ux`, `craft`, `creativity`, `ceo`. Group members are also
available directly (for example `security`, `feedback`, `clarity`, `alive`).

### Chrome

Enable browser automation for Claude Code.

| | |
|---|---|
| **CLI** | `--chrome` / `--no-chrome` |
| **Config** | `chrome: true` |
| **Default** | `false` |

Requires the [Chrome extension](https://chromewebstore.google.com/detail/claude-browser-tool/gfbkicmkbhdjacjmfjffcldkdopkfjgk) and a paid Claude plan.

### Yolo

Skip vendor permission prompts and sandboxes.

| | |
|---|---|
| **Config** | `yolo: true` |
| **Default** | `false` |

Loopflow's normal floor is conservative automation: Codex gets
`workspace-write` and non-interactive Codex runs get `approval_policy = "never"`;
non-interactive Claude runs skip permission prompts. User and repo-level vendor
configs that are already more permissive are not downgraded. For example, Codex
`sandbox_mode = "danger-full-access"` or Claude
`permissions.defaultMode = "bypassPermissions"` are left to the vendor config.
If vendor config is less permissive, Loopflow warns and supplies its default.

`yolo: true` is the explicit Loopflow bypass: Claude uses
`--dangerously-skip-permissions`, Codex uses
`--dangerously-bypass-approvals-and-sandbox`, Gemini uses `--yolo`, and OpenCode
uses `permission: "allow"` via `OPENCODE_CONFIG_CONTENT`.

Durable Task provider turns are the exception. Their assigned worktree is a
hard write boundary, so `yolo` and a more permissive vendor config cannot widen
it. Codex runs with `workspace-write`, Claude uses its strict fail-closed Bash
sandbox, and OpenCode denies external-directory tools.

### Worktree Sandboxes

Claude and Codex CLI/TUI sessions launched from a Git worktree automatically add
the main repo as an extra writable directory. This keeps normal agent
permissions, but lets Git write the linked worktree index under
`<main>/.git/worktrees/<worktree>/` when the agent stages, commits, rebases, or
runs mechanical `lf` commands. Durable Task provider turns do not add the main
repo. Loopflow owns their Git mutations after the provider edits and tests the
assigned files.

### Session Launch

Pick where directly invoked human-present skills open.

```yaml
session:
  launch: tui          # tui | ide
```

`tui` opens Claude, Codex, or OpenCode in the current terminal. `ide` opens the
Codex or Claude app by URL scheme and falls back to `tui` if no app handles the
link. OpenCode is terminal-only. The per-run flags `--tui` / `--ide` override
this default.

### Summaries

Pre-generated codebase overviews for large repos.

```yaml
summary_tokens: 25000

summaries:
  - path: src
  - path: lib
    tokens: 5000
```

### Accounts and Profiles

Account state and repository routes are managed through CLI commands rather
than `config.yaml`:

```bash
lf auth connect claude primary@example.com --chrome-profile primary@example.com
lf route set claude primary@ engineering@
lf --account primary@ implement
```

See [Subscription Management](/docs/subscriptions) for identity storage,
access profiles, routing, health, selectors, and remote development. See
[Security](/docs/security) for credential forwarding and trust boundaries.

### External Skills

Loopflow has one external skill channel plus one compatibility shim. No config needed.

- **`npx/<owner>/<repo>`** — fetched live via [`npx skills`](https://www.npmjs.com/package/skills) and cached under `.agents/skills/`. If the skill is already cached — or `npx skills find` can resolve it — `npx/<name>` often works too. This is the general escape hatch for third-party Claude Skill packages.
- **`rams/rams`** — legacy single-file compatibility shim. It resolves only when `~/.claude/commands/rams.md` exists.

```bash
lf npx/vercel-labs/deep-research      # live fetch, cached on first run
lf rams/rams                          # legacy compatibility alias, if installed
```

The older `skill_sources` config block and `~/.superpowers` auto-detection have been removed. If you were pointing at a local directory of skill prompts, place the files under `.lf/skills/<namespace>/<skill>.md` (repo-local) or `~/.lf/skills/<namespace>/<skill>.md` (user-global) and invoke them as `lf <namespace>/<skill>`. Namespaced skills use `/`, not `:`.
