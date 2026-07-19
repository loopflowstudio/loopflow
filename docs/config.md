# Configuration

Configure loopflow via CLI flags, global config (`~/.lf/config.yaml`), or repo config (`.lf/config.yaml`). Precedence: CLI flags > repo config > global config.

## Quick Reference

| Behavior | CLI Flag | Config |
|----------|----------|--------|
| Model | `-m claude:opus` | `agent: claude:opus` |
| Interactive mode | `-i` | frontmatter: `interactive: true` |
| Include docs | `--docs README.md,docs/` | `docs: [README.md, docs/]` |
| Include branch files | `--diff-files` | `diff_files: true` |
| Include raw diff | `--diff` | `diff: true` |
| Include clipboard | `-c, --clipboard` | — |
| Disable Loopflow guidance | `--no-loopflow` | — |
| Context files | — | `context: [FILE]` |
| Direction (judgment/intent) | `--direction NAME` | `direction: NAME` |
| Chrome automation | `--chrome` | `chrome: true` |
| Yolo mode (skip permissions) | — | `yolo: true` |
| Interactive launch surface | `--tui` / `--ide` | `session.launch: tui` |

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

Five built-in skills intentionally default to Claude: `kickoff`,
`review-design`, `demo`, `code-review`, and `prompt`. Every other unconfigured
built-in skill defaults to Codex. A CLI `-m` or authored `agent:` config remains
an explicit override.

Loopflow starts every Codex CLI and Session run on the standard service tier,
even when the user's Codex config selects Fast mode. In an interactive Codex
TUI, run `/fast` to opt into Fast mode for that session.

Gemini is supported for direct `lf` commands. Wave, Project, and Task Sessions
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

Batch mode runs to completion. Interactive mode allows interruption and chat.

| | |
|---|---|
| **CLI** | `-i` (interactive), `-b` (batch/headless) |
| **Default** | batch for all skills |

Set a skill's default mode in its frontmatter:

```yaml
---
interactive: true
---
# Your skill prompt here
```

CLI flags override the frontmatter default.

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

### Worktree Sandboxes

Claude and Codex CLI/TUI sessions launched from a Git worktree automatically add
the main repo as an extra writable directory. This keeps normal agent
permissions, but lets Git write the linked worktree index under
`<main>/.git/worktrees/<worktree>/` when the agent stages, commits, rebases, or
runs mechanical `lf` commands.

### Session Launch

Pick where directly invoked interactive skills open.

```yaml
session:
  launch: tui          # tui | ide
```

`tui` opens the vendor CLI/TUI in the current terminal. `ide` opens the Codex or
Claude app by URL scheme and falls back to `tui` if no app handles the link.
OpenCode is terminal-only. The per-run flags `--tui` / `--ide` override this default.

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

Connect providers once, then route each repository through managed accounts:

```bash
lf auth status                   # GitHub / Claude / Codex / OpenCode Zen / Linear
lf auth github                   # connect a provider in your browser
lf auth connect claude primary@example.com --chrome-profile primary@example.com
lf auth accounts claude          # cached account state; no network request
lf auth accounts --verify        # compare cached state with every provider now

lf profile create --chrome-profile primary@example.com --as primary
lf auth access set claude primary@ --profile primary
lf route set claude primary@ engineering@
lf route show
```

Each provider login keeps independent auth and session state under
`~/.lf/accounts/`. Commands identify a login by its full email or an unambiguous
email prefix; path-safe internal IDs remain private storage keys. Access profiles
record the Chrome venues that can authenticate a login, and repository routes
record the ordered logins a provider may use. A provider child stays pinned to
its selected login for its lifetime. Shared logins are tried once and share one
cooldown. Profiles, bindings, and repo routes live in the local database —
Loopflow sends no account topology to a central service.

`auth connect <provider> <email-prefix> --chrome-profile` opens the selected
Chrome directory and binds the verified login. Codex
verifies the login email from its ID token; Claude uses the selected Chrome
profile email. `auth import` adopts an existing isolated credential — or the
current macOS Keychain login when the account home is empty — without another
OAuth flow.

`auth accounts --verify` labels cached and live state separately. A revoked
credential is recorded as missing and prints the exact `lf auth connect`
recovery command; agent routes skip it and continue through the next healthy
account.

Routing controls per account:

```bash
lf auth set claude <email-prefix> --paid-through 2026-08-14
lf auth set claude <email-prefix> --routing explicit-only
lf auth reset claude <email-prefix>
lf auth disconnect claude --email <email-prefix>
```

Once `paid-through` passes, automatic routing treats the account as
`explicit-only` until cleared.

Prefer accounts for one invocation, or restrict the process tree to exactly the
selected accounts:

```bash
lf -m codex --account eng@example.com : "fix the tests"
lf --account claude=jack@ --account codex=loopflow-eng@ implement
lf --only-account codex=manabot-eng@ review
```

`--account` keeps the normal provider route as fallback; `--only-account`
removes every unselected account and provider. Both accept repeatable
`claude=<selector>` and `codex=<selector>` forms.

`lf ssh <host> -- <cmd>` resolves the selected route locally and forwards an
opaque handle to a foreground credential broker. It writes no managed provider
credential files on the remote host. Detached remote sessions are rejected —
their lease would vanish when SSH exits — so authenticate on the remote host
for long-running work.

`lf ssh <HomeId> -- <cmd>` resolves the Home's current SSH route from the
durable store and makes the target prove that identity. Add `--remote-native`
to forward no provider, GitHub, PM, or secret authority. Use that mode for Home
lifecycle commands that rely on credentials installed on the remote machine
and outlive the SSH process.

### External Skills

Loopflow has two primary skill channels plus one compatibility shim. No config needed.

- **`gstack/<skill>`** — bundled in the binary. Upstream is [garrytan/gstack](https://github.com/garrytan/gstack). Maintainers run the `refresh-gstack` skill inside the loopflow repo to resync the bundled catalog; users just get the version their `lf` was built with.
- **`npx/<owner>/<repo>`** — fetched live via [`npx skills`](https://www.npmjs.com/package/skills) and cached under `.agents/skills/`. If the skill is already cached — or `npx skills find` can resolve it — `npx/<name>` often works too. This is the general escape hatch for third-party Claude Skill packages.
- **`rams/rams`** — legacy single-file compatibility shim. It resolves only when `~/.claude/commands/rams.md` exists.

```bash
lf gstack/office-hours                # bundled
lf npx/vercel-labs/deep-research      # live fetch, cached on first run
lf rams/rams                          # legacy compatibility alias, if installed
```

The older `skill_sources` config block and `~/.superpowers` auto-detection have been removed. If you were pointing at a local directory of skill prompts, place the files under `.lf/skills/<namespace>/<skill>.md` (repo-local) or `~/.lf/skills/<namespace>/<skill>.md` (user-global) and invoke them as `lf <namespace>/<skill>`. Namespaced skills use `/`, not `:`.
