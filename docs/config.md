# Configuration

Read this when a run needs different instructions, files, or a coding tool.
The Mac app uses these settings too. Make persistent changes in configuration
files; use a command's flags for an experiment that should affect only one run.
A [flag](glossary.md#borrowed-from-software-engineering) is an option such as
`--docs`. A [repository](glossary.md#borrowed-from-software-engineering) is the
project folder tracked by Git.

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

Scalar settings, such as `agent`, use the most specific value. Additive lists
keep entries from both files, so a repository can add context without losing
personal defaults. Personal settings live under `$LF_HOME` when that
[environment variable](glossary.md#borrowed-from-software-engineering) is set;
otherwise the directory is `~/.lf/`, under the current user's home folder.

## Your name

```bash
lf user name          # show the resolved display name
lf user name --json   # name as a JSON string, or null when unavailable
```

Returns a display name, such as `Jack Heart`, using Git's configured `user.name`,
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
preference. Direct interactive and batch launches use the resolved name, and `lf ssh`
carries the caller's name rather than reading the destination owner's name.
Names describe people; they do not grant authority or establish who wrote an
older request. Unattributed background Task and Wave work remains unattributed.

Persisted artifacts use the person's name; session replies use “you.” Stored
transcripts retain their conversational wording.

Opening, moving, or resuming a coding-tool Session preserves the conversation without
submitting a prompt or resetting its participant. Agents can use a name they
already know or the configured name; no reconciliation is required. Opening a
session does not approve a review.

`lf user name --json` resolves the name without contacting a coding-tool
provider or the planning system (PM).
The Mac chat composer uses this local query when sending a message; CLI chat
captures the caller's name before posting to a listener. The name travels with
the message and survives replay. Old messages without names stay anonymous.
Bare interrupts do not load a name, so a preference-read failure cannot block them.
Discord and Linear retain their provider author IDs alongside display names;
a shared publisher account never substitutes for an explicitly named requester.

## Quick Reference

A TUI is an interface inside a terminal; TTY means a terminal connection.
A diff shows changed lines, while branch files include their full contents.
A model is the AI used by the selected coding tool. `FlowStep` means a step
inside a Flow. `LF_EXTERNAL_TERMINAL` is an environment variable for choosing
an external terminal app, such as Ghostty, for a review conversation.

| Behavior | CLI Flag | Config |
|----------|----------|--------|
| Model | `-m claude:opus` | `agent: claude:opus` |
| Interactive TUI | direct TTY or `-i` | `session.launch: tui` |
| Include docs | `--docs README.md,docs/` | `docs: [README.md, docs/]` |
| Include branch files | `--diff-files` | `diff_files: true` |
| Include raw diff | `--diff` | `diff: true` |
| Include clipboard | `-c, --clipboard` | — |
| Disable Loopflow guidance | `--no-loopflow` | — |
| Context files | — | `context: [FILE]` |
| Chrome automation | `--chrome` | `chrome: true` |
| Yolo mode (skip permissions) | — | `yolo: true` |
| Claude/Codex/OpenCode launch surface | `--tui` / `--ide` | `session.launch: tui` |
| Review FlowStep terminal | `LF_EXTERNAL_TERMINAL=Ghostty` | global-only `session.terminal: Ghostty` |

## Context Assembly

[Context](glossary.md#loopflows-words) is the information given to the AI
alongside the skill's instructions. Loopflow prints a breakdown before launch.
[Tokens](glossary.md#borrowed-from-software-engineering) are the text units
counted by the model; more context uses more of its input limit:

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

The defaults give the AI operating instructions and working notes. Other
files are opt-in so every run does not pay the input cost of the entire
repository. Add the source needed for the task with `--docs`; a
[glob](glossary.md#borrowed-from-software-engineering) such as `'docs/*.md'`
selects matching filenames. A diff contains changed lines; `--diff-files`
includes whole changed files. Summaries require configuration.

## Config Files

**Global config** (`~/.lf/config.yaml`) sets user-wide defaults. **Repo config** (`.lf/config.yaml`) overrides for that repo.

For most settings, repo overrides global. For additive settings (`docs`, `context`, `exclude`, `summaries`, `supported_harnesses`), lists combine from both.

```yaml
# ~/.lf/config.yaml (global)
agent: claude:opus
session:
  terminal: Ghostty       # presents review FlowStep sessions on this Home

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

A [Flow](glossary.md#loopflows-words) lists the steps to run. Edit its
[YAML](glossary.md#borrowed-from-software-engineering) file under `.lf/flows/`:

```yaml
# .lf/flows/ship-api.yaml
- implement
- compress
- gate
```

---

## Releases

Configure a [release](glossary.md#borrowed-from-software-engineering) when
a tested version needs to be packaged and published. Loopflow runs the release
sequence; repository commands perform its particular checks and packaging.
A target is one deliverable, such as the CLI. A manifest is a package file
containing its version; a Git tag names the commit for a release.

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
`manifests` use Loopflow's support for
[semantic versions](glossary.md#borrowed-from-software-engineering), such as
`1.2.3`; omit the list to detect supported package files automatically.

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

The workflow is a GitHub job that verifies the candidate before tagging.
Hooks are repository commands run at a stated point. A publisher is the
configured command with permission to sign or upload the release.

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

`LOOPFLOW.md` tells the AI how to use Loopflow for local work, Git, and pull
requests. It is included by default so a run knows the operating rules.
Individual skills can add instructions for delegation.

| | |
|---|---|
| **CLI** | `--no-loopflow` |
| **Default** | included |

Use `--no-loopflow` when you want a leaner prompt without loopflow-specific process guidance.

### Docs

Include selected files before the AI starts. Extra docs are omitted by default to leave input space for the work itself.

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

Paste clipboard text into the prompt. A
[stack trace](glossary.md#borrowed-from-software-engineering) lists the calls
leading to an error; including it helps locate the failure. Clipboard access
is opt-in so unrelated copied text is not sent automatically.

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

Choose the default [harness](glossary.md#loopflows-words), the AI coding tool,
and optionally its model. Leaving it unset lets Loopflow use an installed tool
without requiring configuration.

| | |
|---|---|
| **CLI** | `lf gate -m codex:o3` |
| **Config** | `agent: claude:opus` (optional) |
| **Default** | unset (resolution falls back to skill defaults, then the first of `codex`, `claude`, `opencode` installed on this machine) |

```yaml
agent: codex          # harness default
# agent: claude:opus  # harness plus model
```

Harnesses: `claude`, `codex`, `opencode`. Use `harness:model` for specific models.

Four built-in skills intentionally default to Claude: `kickoff`,
`review-design`, `review-slice`, and `prompt`. Every other unconfigured
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

Direct commands open an interactive conversation when their input or output
is a terminal (TTY). [Standard input and output](glossary.md#borrowed-from-software-engineering)
are the text a command receives and prints. Automated Flow steps and `--batch`
run headlessly, without someone typing replies. Skill frontmatter—the settings
at the top of a skill file—does not change how turns are scheduled.

| | |
|---|---|
| **CLI** | `-i` (interactive), `-b` (batch/headless) |
| **Default** | interactive for a direct TTY; headless otherwise |

When unattended work needs a review, mark that exact Flow step with a stable
`id` and `human: true`. It opens a Session and waits for explicit completion;
see [Authoring](authoring.md#flows).

### Chrome

Enable browser automation for Claude Code.

| | |
|---|---|
| **CLI** | `--chrome` / `--no-chrome` |
| **Config** | `chrome: true` |
| **Default** | `false` |

Requires the [Chrome extension](https://chromewebstore.google.com/detail/claude-browser-tool/gfbkicmkbhdjacjmfjffcldkdopkfjgk) and a paid Claude plan.

### Yolo

Change which coding-tool actions can run without asking. A
[sandbox](glossary.md#borrowed-from-software-engineering) restricts access;
a permission prompt asks before an action. These are different controls.
`yolo` requests bypassing both where the tool supports it.

| | |
|---|---|
| **Config** | `yolo: true` |
| **Default** | `false` |

For ordinary launches, Loopflow supplies a minimum level of access: Codex gets
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

Managed Task turns use a separate trusted delivery setup. Before starting,
Loopflow checks write access to linked Git metadata and its control store,
and requires an eligible managed Claude or Codex account. The Task runs in its
assigned worktree, but that directory is **not a hard sandbox**: managed Codex
turns bypass approvals and the vendor sandbox, and managed Claude turns skip
permission prompts. These permissions let the agent use Loopflow's Git and
delivery commands. They do not limit access to only the checked roots.
`yolo: false` does not restore vendor prompts for managed Task turns.

### Worktree Sandboxes

Ordinary Claude and Codex CLI/TUI sessions launched from a Git worktree add the
main repository as an extra writable directory. Git needs its metadata under
`<main>/.git/worktrees/<worktree>/` to stage, commit, or rebase.

Managed Task turns use the delivery setup above instead of those extra-directory
flags. Loopflow's checks establish required access, not OS containment. Use a
separate OS user, container, or VM when broader access needs restricting; see
[Security](security.md).

### Session Launch

Choose where directly invoked interactive skills open. The default, `tui`,
keeps the conversation in the current terminal. `ide` asks the coding tool's
app to open it; it does not mean the Loopflow Mac app.

```yaml
session:
  launch: tui          # tui | ide
```

`tui` opens Claude, Codex, or OpenCode in the current terminal. `ide` opens the
Codex or Claude app by URL scheme and falls back to `tui` if no app handles the
link. OpenCode is terminal-only. The per-run flags `--tui` / `--ide` override
this default.

### Summaries

Include pre-generated overviews when the codebase is too large to send in
full. `summary_tokens` sets the overall text budget; a path's `tokens` sets
its own allowance.

```yaml
summary_tokens: 25000

summaries:
  - path: src
  - path: lib
    tokens: 5000
```

### Accounts and Profiles

An [account route](glossary.md#loopflows-words) lists coding-tool logins to
try. An access profile selects the browser profile used to sign in. Manage
these through commands rather than `config.yaml`:

```bash
lf auth connect claude primary@example.com --chrome-profile primary@example.com
lf route set claude primary@ engineering@
lf --account primary@ implement
```

See [Subscription Management](/docs/subscriptions) for account storage, browser profiles, fallback order, and using accounts on another machine. See
[Security](/docs/security) for credential forwarding and trust boundaries.

### External Skills

Run a skill from another repository with `npx/`; no config entry is needed.
`npx` downloads and runs a JavaScript package. A cached skill is a locally
saved copy. The older `rams/rams` alias reads an installed local file.

- **`npx/<owner>/<repo>`** — fetched live via [`npx skills`](https://www.npmjs.com/package/skills) and cached under `.agents/skills/`. If the skill is already cached — or `npx skills find` can resolve it — `npx/<name>` often works too. Use it for third-party Claude Skill packages.
- **`rams/rams`** — legacy single-file compatibility shim. It resolves only when `~/.claude/commands/rams.md` exists.

```bash
lf npx/vercel-labs/deep-research      # live fetch, cached on first run
lf rams/rams                          # legacy compatibility alias, if installed
```

The older `skill_sources` config block and `~/.superpowers` auto-detection have been removed. If you were pointing at a local directory of skill prompts, place the files under `.lf/skills/<namespace>/<skill>.md` (repo-local) or `~/.lf/skills/<namespace>/<skill>.md` (user-global) and invoke them as `lf <namespace>/<skill>`. Namespaced skills use `/`, not `:`.
