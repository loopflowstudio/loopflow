# Configuration

```bash
lf user                                # show the resolved display name
lf user --json                         # name as a JSON string, or null when unavailable
```

`user` is the unique shortcut for `lf config user`. It returns a display name,
such as `Jack Heart`, using Git's configured `user.name`,
including repository overrides. Agents can use a familiar name such as `Jack`
in prose. No first name or username is derived from this value. Override it
in personal Loopflow configuration (`$LF_HOME/config.yaml`, or
`~/.lf/config.yaml` when `LF_HOME` is unset):

```yaml
user:
  name: Jack
```

Edit `user.name` to correct it; remove it or leave it blank to use Git again.
Keep other settings in the file. Repo `.lf/config.yaml` cannot override this
preference. Direct interactive and batch launches use the resolved name, and `lf --machine`
carries the caller's name rather than reading the destination owner's name.
A non-empty `LF_USER_NAME` overrides these sources for a launched request; an
empty or whitespace-only value falls through to personal configuration and Git.
Steering comments prefer their recorded requester, then Linear's author name.
Names describe people; they do not grant authority or establish who wrote an
older request. Unattributed background Task and Wave work remains unattributed.

Persisted artifacts use the person's name; session replies use “you.” Stored
transcripts retain their conversational wording.

Opening, moving, or resuming a native session preserves the conversation without
submitting a prompt or resetting its participant. Agents can use a name they
already know or the configured name; no reconciliation is required. Opening a
session does not approve a review.

`lf user --json` resolves the name without provider or PM access.
Session prompts carry the selected participant name. Stored transcripts retain
their original wording; unnamed historical messages remain anonymous.

Discord and Linear retain their provider author IDs alongside display names;
a shared publisher account never substitutes for an explicitly named requester.

Start with a one-run override; keep it in repo config only when the choice
should apply to everyone:

```bash
lf gate -a codex --docs docs/api.md
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
| Agent | `-a claude:opus` | `agent: claude:opus` |
| Interactive terminal | direct TTY or `-i` | — |
| Include docs | `--docs README.md,docs/` | `docs: [README.md, docs/]` |
| Include branch files | `--diff files` | `diff_files: true` |
| Include raw diff | `--diff patch` | `diff: true` |
| Include clipboard | `-c, --clipboard` | — |
| Disable Loopflow guidance | `--no-loopflow` | — |
| Context files | — | `context: [FILE]` |
| Chrome automation | `--chrome` | `chrome: true` |
| Yolo mode (skip permissions) | — | `yolo: true` |
| Account's own provider home | `--isolate` / `--shared` | `isolate: true` |
| Review FlowStep terminal | — | global-only `session.terminal: Ghostty` |

## Context budgets

```bash
lf context                         # size targets, sources and current usage
lf context --wave intelligence     # local Wave memory and scratch
lf context --task LOO-303 --json    # Task checkout, including Wave memory
```

Edit the existing repo `.lf/config.yaml`:

```yaml
context_budgets:
  memory_tokens: 6000
  scratch_tokens: 8000
```

Override individual fields for a Wave in `wave/<name>/GOAL.md` frontmatter:

```yaml
---
context_budgets:
  memory_tokens: 10000
---
```

Each field resolves from Wave frontmatter, repo config, personal config, then
the compiled default. `lf context` shows the winning source for every value.
The same block supports `memory_bytes` and `scratch_bytes`. Values must be positive
integers. These are authoring targets, not launch limits: sources are never excerpted
or refused for exceeding them. The retired `goal_*` and `input_*` keys are removed.

`lf context` measures recursive scratch Markdown and checkout Wave/ancestor memory,
without contacting a provider or reading the clipboard. Defaults are 16,000 tokens /
128 KiB for memory and 12,000 tokens / 96 KiB for scratch. Curate stale notes gradually,
preserving live decisions and evidence; re-query after writing.

The first conversation turn contains the skill followed by the request. Terminal
launches pass it as an argument; if its UTF-8 size reaches the 122,880-byte argument
cap, launch fails before starting the provider and reports the size and cap. Headless
message streams have no terminal-argument cap.

## Context Assembly

Launch headers inventory gathered sources; those counts are not the bytes sent
in a provider request. The harness loads its repo guide natively.

| Content | Delivery | Selection |
|---------|----------|-----------|
| Operating and surface instructions, participant, reply guidance | Fixed added-instructions slot | `--no-loopflow` omits the operating guide |
| Skill and request | First turn | Selected skill and launch message |
| Scratch and checkout Wave/ancestor documents | Refreshed conversation block; whole files or a complete listing | Current checkout and selected Wave |
| Explicit docs | File paths in the listing | `docs:` or `--docs` |
| Branch changes | Git inspection commands and changed paths | `--diff patch`, `diff_files: true` |
| Codebase summaries and clipboard | Complete private reference files | `summaries:`, `-c` |

The block refreshes at startup and after compaction. After compaction it also
includes the saved active skill. Overflow points to complete files, never excerpts.

## Config Files

**Global config** (`~/.lf/config.yaml`) sets user-wide defaults. **Repo config** (`.lf/config.yaml`) overrides for that repo.

For most settings, repo overrides global. For additive settings (`docs`, `context`, `exclude`, `summaries`, `supported_harnesses`), lists combine from both.

```yaml
# ~/.lf/config.yaml (global)
agent: claude:opus
session:
  terminal: Ghostty       # presents review FlowStep sessions on this Machine

# .lf/config.yaml (repo)
agent: codex        # overrides global
context:
  - docs/api.md           # combined with global context
```

Example repo config:

```yaml
agent: claude:opus

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
exact change range but before it prepares release changes. `lf check`
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

List specific files, globs, or directories for the agent to read. Not included by default.

| | |
|---|---|
| **CLI** | `--docs PATH[,PATH...]` |
| **Config** | `docs: [PATH, PATH]` |
| **Default** | none (empty) |

Each entry is a file (`README.md`), a glob (`'*.md'`), or a directory (`swift/`
gathers `*.md` under it). Resolved paths appear in the context listing; file
bodies are not inlined. This doesn't restrict which files the agent can edit.
Scratch and the selected Wave's checkout Markdown are gathered automatically.
Wave context reads `wave/<name>/` and each path-segment ancestor, excluding siblings
and children; listings point to those files, not database snapshots.

### Branch Files (diff_files)

List paths modified on the current branch for the agent to inspect.

| | |
|---|---|
| **CLI** | `--diff files` / `--diff none` |
| **Config** | `diff_files: true` |
| **Default** | `false` |

Use `--diff files` to identify changed files. The reference points to Git inspection;
it does not preload file bodies or patches.

### Clipboard

Save clipboard content in a complete private file referenced by the context listing.

| | |
|---|---|
| **CLI** | `-c, --clipboard` |
| **Default** | not included |

Use `-c` when debugging: copy an error, then `lf debug -c`.

### Raw Diff

Point to Git inspection commands rather than inlining a frozen patch.

| | |
|---|---|
| **CLI** | `--diff patch` / `--diff none` |
| **Config** | `diff: true` |
| **Default** | `false` (not included) |

Use when the agent needs to inspect exact changes. `--diff both` also lists changed paths.

### Context Files

Additional file paths listed for every skill.

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
| **CLI** | `lf gate -a codex:o3` |
| **Config** | `agent: claude:opus` (optional) |
| **Default** | unset (resolution falls back to skill defaults, then the first of `codex`, `claude`, `opencode` installed on this machine) |

```yaml
agent: codex          # harness default
# agent: claude:opus  # harness plus model
```

Harnesses: `claude`, `codex`, `opencode`. Use `harness:model` for specific models.

Four built-in skills intentionally default to Claude: `kickoff`,
`review-design`, `realign`, and `prompt`. Every other unconfigured
built-in skill uses Codex when it is installed, then Claude, then OpenCode. A
CLI `-m` or authored `agent:` config remains an explicit override.

Loopflow starts every Codex CLI and interactive run on the standard service tier,
even when the user's Codex config selects Fast mode. In an interactive Codex
TUI, run `/fast` to opt into Fast mode for that session.

Bare harness names use the model selected by that provider account. Add
`harness:model` only when the invocation must pin a specific model. OpenCode
model strings use `provider/model` form:

```yaml
agent: opencode                          # user's OpenCode default
agent: opencode:opencode/glm-5.2         # explicit model
agent: opencode:moonshotai/kimi-k2       # explicit provider/model
```

### Supported Harnesses

Optional list of harnesses exposed in Loopflow's model picker and settings.

```yaml
supported_harnesses:
  - claude
  - codex
  - opencode
```

This list is additive across global and repo config.

### Run Mode

Direct named invocations use an interactive session when stdin or stdout is a
TTY. Automated flow nodes and `-b` invocations run headlessly. Skill
frontmatter never changes scheduling.

| | |
|---|---|
| **CLI** | `-i` (interactive), `-b` (batch/headless) |
| **Default** | interactive for a direct TTY; headless otherwise |

Flows hold autonomous steps only; a `human: true` step is rejected at launch.
See [Authoring](authoring.md#flows).

### Chrome

Enable browser automation for Claude Code.

| | |
|---|---|
| **CLI** | `--chrome on` / `--chrome off` |
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
`--dangerously-bypass-approvals-and-sandbox`, and OpenCode uses
`permission: "allow"` via `OPENCODE_CONFIG_CONTENT`.

Durable Task provider turns are the exception. Their assigned worktree is a
hard write boundary, so `yolo` and a more permissive vendor config cannot widen
it. Codex runs with `workspace-write`, Claude uses its strict fail-closed Bash
sandbox, and OpenCode denies external-directory tools.

### Worktree Sandboxes

Claude and Codex CLI/TUI sessions launched from a Git worktree automatically add
the main repo as an extra writable directory. This keeps normal agent
permissions, but lets Git write the linked worktree index under
`<main>/.git/worktrees/<worktree>/` when the agent stages, commits, syncs, or
runs mechanical `lf` commands. Durable Task provider turns do not add the main
repo. Loopflow owns their Git mutations after the provider edits and tests the
assigned files.

### Interactive sessions

```bash
lf -a claude audit
```

Interactive skills run in the current terminal. A direct TTY invocation is
interactive by default; `-i` keeps interactive execution when input is piped.
Use `-b` for headless execution.

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
lf account connect claude primary@example.com --chrome-profile primary@example.com
lf account route set claude primary@ engineering@
lf --account primary@ implement
```

See [Subscription Management](/docs/subscriptions) for identity storage,
access profiles, routing, health, selectors, and remote development. See
[Security](/docs/security) for machine logins and trust boundaries.

### Skill sources

```bash
lf help audit                       # inspect the selected file
lf list skill                       # list names and sources
```

Put `audit/SKILL.md` in `.claude/skills`, `.agents/skills`, or `.codex/skills`.
A bundle wins over a same-named Markdown file in the same folder.
Single Markdown files also work in `.lf/skills`, `.claude/commands`, and
`.codex/prompts`. Supporting files inside a skill bundle are not separate skills.
Namespaces use `/`, such as `lf help team/audit`.

Repository sources win over personal sources, then embedded builtins. Within each
scope, precedence is `.lf/skills`, `.claude/skills`, `.claude/commands`,
`.agents/skills`, `.codex/skills`, `.codex/prompts`. Personal Claude and Codex
folders respect `CLAUDE_CONFIG_DIR` and `CODEX_HOME`. Generated Loopflow exports
never override the current embedded builtin. Help, listing, execution lookup,
and retained Flow definitions use the same selection. Listings prefer the authored
`description`, falling back to the first prose line.

Install third-party skills with their own installer, then use their installed
names. The `npx/` fetch path and `rams/rams` alias are removed; an installed
`rams.md` is named `rams`.

`lf -b -a claude audit` invokes a Claude bundle directly from its original
folder through the native command parser, with gathered context separate from
command arguments. Terminal Claude uses a captured native plugin with the same
declarations and exact arguments; gathered context uses the refreshed conversation block.
Codex sources use explicit native skill links on both surfaces, including sources
outside Codex's discovered catalog. Cross-harness launches translate argument
and tool-name instructions, retain declarations and identify the original asset
directory. Codex custom prompts expand one-based positions and `NAME=value`
arguments. A warning names declarations the launch does not enforce.

Third-party and builtin launches use the same fixed additions and conversation
context. `--no-loopflow` explicitly omits the operating guide. Memory and scratch
budgets are authoring targets, not launch gates. LF builtins remain inline in
the first turn. Captured Flow definitions survive
source-file changes or removal: Claude uses a captured native definition, while
Codex uses captured instructions. Both retain the original resource directory;
resources removed with the bundle are not preserved.

```bash
lf sync-skills --yes       # personal skills and builtin skills/Flows → home
lf sync-skills --repo      # repository skills/Flows → this checkout
```

Global exports go to `~/.claude/skills` and `~/.agents/skills`; repository exports
go to the corresponding checkout directories, never home. Release promotion’s
skill sync includes builtin Flows. Recipes capture Flow topology at sync time;
sync again after adding, editing, removing or overriding repository definitions.
Headless commands load their skills when launched.

Exact names win: `.lf/skills/wave/session.md` and `.lf/skills/wave-session.md`
are distinct definitions. `lf skill wave/session` selects the first; `lf skill
wave-session` selects the second. When no exact name exists, `/` → `-` provides a
fallback. Multiple fallback candidates produce an ambiguity naming the candidates.
The same rules apply to Flows. Repository, personal and provider-root precedence
apply to identical literal names. Existing unique bare shortcuts remain; hyphen
suffixes do not create new shortcuts. Untyped lookup checks exact names before
fallbacks, with Flow-first precedence for the same literal name.

Codex exports retain slash and dash names, plus dashed fallbacks where unoccupied.
Claude exports are flat: a literal dashed name wins over a flattened hierarchical
name, with unrepresentable exports reported as skipped. Flow exports have no
prefix and yield to skills at each provider's projected name. Each recipe retains
its original exact Flow name. Invalid provider names are reported, never silently
rewritten. Third-party files and bundles are preserved. Sync prunes only generated
exports after replacements are writable; desired nested Codex exports survive.
Original native declarations and resource paths remain attached to each source;
cross-harness exports do not claim to enforce or translate native controls.
