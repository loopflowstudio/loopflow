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
older request. Task claims compare names, so give each person on a repository a
distinct one: a claim with no name, or under yours, reads as yours. Unattributed background Task and Wave work remains unattributed.

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
| [Planning synchronization](work.md#share-the-plan) | — | `planning: {provider: linear, team: "<id>"}`, `planning: {provider: asana, team: "<gid-or-url>"}`, `planning: {provider: notion, team: "<page-url-or-id>"}` or `planning: {provider: git, shared: team}` |
| Interactive terminal | direct TTY or `-i` | — |
| Include docs | `--docs README.md,docs/` | `docs: [README.md, docs/]` |
| Include branch files | `--diff files` | `diff_files: true` |
| Include raw diff | `--diff patch` | `diff: true` |
| Include clipboard | `-c, --clipboard` | — |
| Context files | — | `context: [FILE]` |
| Chrome automation | `--chrome` | `chrome: true` |
| Yolo mode (skip permissions) | `--yolo` | `yolo: true` |
| Account's own provider home | `--isolate` / `--shared` | `isolate: true` |
| Review GraphStep terminal | — | global-only `session.terminal: Ghostty` |
| End agents nobody is viewing | — | `session.end_detached_after: 24h` (or `off`) |

## Scratch lifecycle

```bash
lf scratch clear   # remove checkpointed scratch additions; retain existing files and edits
```

Skills write plans and notes to `scratch/` without setup, and recursive Markdown
there enters agent context automatically. Scratch follows ordinary Git behavior
in every checkout. No scratch setting is required.

`lf pr submit` and `lf pr land` invoke this cleanup during preparation, so default
Graphs need no extra step. It is also available on its own. Publishing a draft or
ready PR does not run general scratch cleanup; PR-copy handoff artifacts retain
their existing lifecycle. Cleanup uses the existing Task diff base
or the branch's merge base with main; it removes only added files, preserving
existing files, edits, detected renames and `.gitkeep`. Untracked files outside
the checkpoint remain. Repeating cleanup makes no empty commit.

## Context budgets

Native Codex Sessions restore their context binding on start and reconnect,
including when the provider is still running. A missing delivery file reports its
path; reconnect the Loopflow Session to restore the binding from its saved input.
Missing referenced files remain errors and must be restored at their reported paths.
Each Codex thread trusts exactly Loopflow's own hook declarations, after any
wrapper hooks; host declarations and saved Codex trust decisions remain unchanged.

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

Override individual fields for a Wave in `.lf/wave/<name>/config.yaml`:

```yaml
context_budgets:
  memory_tokens: 10000
```

Each field resolves from Wave config, repo config, personal config, then
the compiled default. `lf context` shows the winning source for every value.
The same block supports `memory_bytes` and `scratch_bytes`. Values must be positive
integers. These are authoring targets, not launch limits; exceeding them does not
refuse a launch or select its context. The retired `goal_*` and `input_*` keys are removed.

`lf context` measures recursive scratch Markdown and checkout Wave/ancestor memory,
without contacting a provider or reading the clipboard. Defaults are 16,000 tokens /
128 KiB for memory and 12,000 tokens / 96 KiB for scratch. Curate stale notes gradually,
preserving live decisions and evidence; re-query after writing.

The first conversation turn contains the skill, request and explicitly selected
documents. Codex uses app-server input and Claude uses streamed or file input.
Pi and OpenCode terminal launches retain their 122,880-byte argument cap and
report oversized input before launch. Provider context-window limits still apply;
Loopflow never retries with shortened documents.

## Context Assembly

Launch headers inventory gathered sources; those counts are not the bytes sent
in a provider request. The harness loads its repo guide natively.

| Content | Delivery | Selection |
|---------|----------|-----------|
| Operating and surface instructions, participant, reply guidance | Fixed added-instructions slot | Standalone native skills retain their own guidance |
| Skill and request | First turn | Selected skill and launch message |
| Scratch and checkout Wave/ancestor documents | Refreshed conversation block; whole files, marked start excerpts and a complete listing | Current checkout and selected Wave |
| Explicit docs | Complete reference bodies in the submitted turn | `docs:` in config or Loopflow skill frontmatter, or `--docs` |
| Branch changes | Git inspection commands and changed paths | `--diff patch`, `diff_files: true` |
| Codebase summaries | Complete private reference files | `summaries:` |
| Clipboard | Tagged block after the request in the first turn | `-c` |

The block refreshes at startup and after compaction. After compaction it also
includes the saved active skill. Large documents include marked start excerpts
with complete source paths and sizes.
The rendered block stays within the provider byte cap; overflow listings and skills
remain readable through complete-file pointers.

## Config Files

**Global config** (`~/.lf/config.yaml`) sets user-wide defaults. **Repo config** (`.lf/config.yaml`) overrides for that repo. **Wave config** (`.lf/wave/<name>/config.yaml`) overrides for work in that Wave; repository-only keys (`planning`, `release`, `exclude`, `merge_method`, `land`, `pr`) are an error there. See [Set it up once](work.md#set-it-up-once).

For most settings, the nearer file wins; `context_budgets` and `skill_agents` merge per key. For additive settings (`docs`, `context`, `exclude`, `summaries`, `supported_harnesses`), lists combine from both.

```yaml
# ~/.lf/config.yaml (global)
agent: claude:opus
session:
  terminal: Ghostty       # presents review GraphStep sessions on this Machine
  end_detached_after: 24h # stop an unviewed Session's agent; `lf resume` reopens it

# .lf/config.yaml (repo)
agent: codex        # overrides global
context:
  - docs/api.md           # combined with global context
```

Scheduled commands are personal and never configured here; see
[Schedule it yourself](ops.md#schedule-it-yourself).

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

## Graphs

Graphs are YAML files in `.lf/graphs/`:

```yaml
# .lf/graphs/ship-api.yaml
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

Launches receive the small `LOOPFLOW.md` contract. Task- and Wave-associated
launches also preload Work guidance. Read `lf help skill work` when entering
Work APIs from an existing conversation. Standalone native skills retain their
own guidance. Association selects context; a requested skill or Graph selects
the procedure.

### Docs

List specific files, globs, or directories for the agent to read. Not included by default.

| | |
|---|---|
| **CLI** | `--docs PATH[,PATH...]` |
| **Config** | `docs: [PATH, PATH]` |
| **Default** | none (empty) |

Each entry is a file (`README.md`), a glob (`'*.md'`), or a directory (`swift/`
gathers `*.md` under it). Complete UTF-8 bodies and source paths enter the submitted
turn, including absolute files outside the repository. Preview and durable capture
show those submitted bytes. Dollar signs in reference text use `&#36;` notation
to avoid invoking native skills. Empty files remain labeled references; missing,
unreadable or nontext exact selections fail with the path and reason.

Exclusions and the 100-file ceiling still apply. Explicitly selecting an ambient
file supplies it once in task text instead of the refreshed block. New `--docs`
selections on `lf -b connect ID --message TEXT` belong to that turn; prior documents
stay in native history. This doesn't restrict which files the agent can edit.
Scratch and the selected Wave's checkout Markdown are gathered automatically.
Wave context reads `.lf/wave/<name>/` and each path-segment ancestor, excluding siblings
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

Append clipboard content in its own tagged block after the request in the first turn.
It counts toward the terminal argument cap.

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
| **Default** | unset (resolution falls back to skill defaults, then the first of `codex`, `claude`, `opencode`, `pi` installed on this machine) |

```yaml
agent: codex          # harness default
# agent: claude:opus  # harness plus model
skill_agents:
  implement: claude:opus  # this skill only, ahead of the skill's own agent
```

Harnesses: `claude`, `codex`, `opencode`, `pi`. Use `harness:model` for specific models.

Four built-in skills intentionally default to Claude: `kickoff`,
`explore`, `realign`, and `prompt`. Every other unconfigured
built-in skill uses Codex when it is installed, then Claude, then OpenCode, then Pi. A
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

### Pi

```bash
npm install -g --ignore-scripts @earendil-works/pi-coding-agent@1.3.0
pi                                      # configure a model and /login
lf -a pi implement                     # native interactive Session
lf -b -a pi:anthropic/claude-sonnet-4-6 implement
lf resume <session-id>                  # reopen its exact Pi conversation
```

Pi 1.3.0 is the protocol baseline. Bare `pi` uses Pi's own model selection;
`pi:provider/model` passes the model to Pi. Pi owns credentials and its native
session files. API keys supplied to the process (for example through Doppler)
remain available to Pi; Loopflow does not copy Claude/Codex subscription logins
into it. Configure Pi on each Machine where it runs.

Headless work uses Pi RPC; native Sessions retain Pi's terminal interface.
Loopflow supplies context through a launch-local extension, including after
compaction, and translates selected skills with their arguments. Resume requires
the recorded session file; deleting it produces an error instead of a new chat.
Keep that file when moving or archiving work. A native `/new` or session switch
changes the conversation outside Loopflow's recorded identity; start another
Loopflow Session to select a new conversation.

Text, tool events and reported token/cost usage enter capture/history. Pi's final
`agent_settled` event ends a turn, including Pi's internal retries and queued
steering. Loopflow preserves a final failure; it does not send an automatic
continuation prompt. Resume with explicit input to continue. A blank terminal
answer fails headless execution. Native usage checkpoints survive a crash before
settlement; missing usage stays unknown.

`lf session steer <session-id> "direction"` queues guidance during an active Pi
run through the same saved-steer receipts used by other harnesses. An emitted
receipt means local delivery, not proof the model incorporated it. Idle Sessions
stay idle. Direct RPC steering reports queued, rejected or uncertain delivery
independently of the initiating turn's result.
Managed account switching, subscription limits, native response schemas,
turn limits and transferring Pi history to another harness are unavailable.
Headless extension dialogs fail with an instruction
to open the Session interactively. Pi's tools use the OS user's permissions;
project trust and the working directory do not sandbox them. Provider-only
worktree confinement is unsupported; managed Tasks use the caller's OS boundary.

### Supported Harnesses

Optional list of harnesses exposed in Loopflow's model picker and settings.

```yaml
supported_harnesses:
  - claude
  - codex
  - opencode
  - pi
```

This list is additive across global and repo config.

### Run Mode

Direct named invocations use an interactive session when stdin or stdout is a
TTY. Automated graph nodes and `-b` invocations run headlessly. Skill
frontmatter never changes scheduling.

| | |
|---|---|
| **CLI** | `-i` (interactive), `-b` (batch/headless) |
| **Default** | interactive for a direct TTY; headless otherwise |

A Graph's steps run headless except its `interactive:` steps, which launch the way
a direct skill does; `-i` and `-b` choose for every step. `lf task run` stops
before each interactive step instead. See [Orchestration](orchestration.md#where-a-person-takes-part).

### Chrome

Enable browser automation for Claude Code.

| | |
|---|---|
| **CLI** | `--chrome on` / `--chrome off` |
| **Config** | `chrome: true` |
| **Default** | `false` |

Applies to fresh and resumed Claude Sessions, both terminal and headless.
`--chrome off` explicitly disables the vendor integration. Other providers do
not use this Claude-specific setting.

Requires the [Chrome extension](https://chromewebstore.google.com/detail/claude-browser-tool/gfbkicmkbhdjacjmfjffcldkdopkfjgk) and a paid Claude plan.

### Yolo

Skip vendor permission prompts and sandboxes.

| | |
|---|---|
| **CLI** | `--yolo` |
| **Config** | `yolo: true` |
| **Default** | `false` |

Loopflow's normal floor is conservative automation: Codex gets
`workspace-write` and non-interactive Codex runs get `approval_policy = "never"`;
non-interactive Claude runs skip permission prompts. User and repo-level vendor
configs that are already more permissive are not downgraded. For example, Codex
`sandbox_mode = "danger-full-access"` or Claude
`permissions.defaultMode = "bypassPermissions"` are left to the vendor config.
If vendor config is less permissive, Loopflow supplies its default.

`--yolo` or `yolo: true` is the explicit Loopflow bypass: Claude uses
`--dangerously-skip-permissions`, Codex sends `sandbox: "danger-full-access"`
and `approvalPolicy: "never"` through its app-server protocol, and OpenCode uses
`permission: {"*": "allow"}` via `OPENCODE_CONFIG_CONTENT`.

Managed Task turns require an execution boundary independently of vendor
permission settings. Claude and Codex bypass their own permission prompts within
managed execution; `yolo` does not remove the required boundary. Pi uses the same
caller-owned OS environment and its own credentials; no vendor sandbox is implied.
OpenCode cannot currently satisfy that managed execution requirement. For
provider-only worktree scope, Codex uses `workspace-write`, Claude uses its strict fail-closed Bash
sandbox, and OpenCode denies external-directory tools, including with `yolo`.

Stopped-session resume reloads Loopflow settings at the saved checkout. The
provider's conversation identity and account stay attached; a current model
setting for that provider applies. A configured different provider does not
change the saved history's provider. CLI overrides take precedence. Connecting
to an already running provider keeps its existing launch settings. Historical
replay uses captured inputs. Resuming native history from an isolated account
keeps that account's provider home, including when `--shared` is requested.

Claude turn limits and native JSON schemas require headless mode; terminal
launches reject them. Other headless providers report unsupported turn limits
or native schemas and continue with ordinary output; this also preserves
historical replay. Graph output validation still applies to every provider.

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
lf set account.claude primary@,engineering@
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

Repository sources win over personal sources, then embedded builtins. A team's
repository Skill changes everyone's process; to try your own variant, save it
under a new name in `~/.lf/skills`. Within each
scope, precedence is `.lf/skills`, `.claude/skills`, `.claude/commands`,
`.agents/skills`, `.codex/skills`, `.codex/prompts`. Personal Claude and Codex
folders respect `CLAUDE_CONFIG_DIR` and `CODEX_HOME`. Generated Loopflow exports
never override the current embedded builtin. Help, listing, execution lookup,
and retained Graph definitions use the same selection. Listings prefer the authored
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

Standalone native skills use their own instructions with gathered conversation
context; builtin launches receive the launcher contract and associated Work
guidance. Memory and scratch
budgets are authoring targets, not launch gates. LF builtins remain inline in
the first turn. Captured Graph definitions survive
source-file changes or removal: Claude uses a captured native definition, while
Codex uses captured instructions. Both retain the original resource directory;
resources removed with the bundle are not preserved.

```bash
lf self sync-skills --yes   # personal skills and builtin skills/Graphs → home
lf self sync-skills --repo  # repository skills/Graphs → this checkout
```

Global exports go to `~/.claude/skills` and `~/.agents/skills`; repository exports
go to the corresponding checkout directories, never home. Release promotion’s
skill sync includes builtin Graphs. Recipes capture Graph topology at sync time;
sync again after adding, editing, removing or overriding repository definitions.
Headless commands load their skills when launched.

Exact names win: `.lf/skills/wave/session.md` and `.lf/skills/wave-session.md`
are distinct definitions. `lf skill wave/session` selects the first; `lf skill
wave-session` selects the second. When no exact name exists, `/` → `-` provides a
fallback. Multiple fallback candidates produce an ambiguity naming the candidates.
The same rules apply to Graphs. Repository, personal and provider-root precedence
apply to identical literal names. Existing unique bare shortcuts remain; hyphen
suffixes do not create new shortcuts. Untyped lookup checks exact names before
fallbacks, with Graph-first precedence for the same literal name.

Exports use one portable name per skill in both directories: lowercase letters,
digits and separating hyphens, at most 64 characters. `repo/operate` exports as
`repo-operate`. Long names use their final component plus a stable suffix, such as
`pdf-a1b2c3d4`. Exact literal names keep priority; collisions receive distinct
suffixes so neither skill is lost. Original `lf` names remain unchanged; native
slash aliases are replaced by these portable names. Graph exports yield to skills
at the same projected name and retain their original exact Graph identity.
Third-party files and bundles are preserved. Sync prunes only generated exports
after replacements are writable.
Original native declarations and resource paths remain attached to each source;
cross-harness exports do not claim to enforce or translate native controls.

## Scratch cleanup during delivery

Delivery clears committed scratch additions relative to the Task's base (or the
branch's integration base). Existing files, edits and renames remain repository
content. `scratch/.gitkeep` is preserved. Use `lf scratch clear` for the same
explicit cleanup; checkpoint intended notes first. Uncommitted files are preserved
by this command. Delivery checkpoints the intended change before cleanup.

## Merge policy

```yaml
# .lf/config.yaml — only needed to prefer a particular allowed method
merge_method: merge  # merge, squash, or rebase
```

`arm` and `land` read the repository's allowed GitHub merge methods. Without a
preference, choose squash, then merge, then rebase from the allowed methods.
A repository preference disabled by GitHub returns an actionable error before
replacing an existing auto-merge request. A merge queue owns its method, so lf
sends no method or commit-message override for queued merging. Personal config
cannot override this repository policy. Every request retains the observed head
commit check and GitHub's protection requirements.

Keep procedure in repository instructions and Graphs: who should review, when to
run checks, and whether to stop at `publish`, `submit`, or auto-merge. Do not
replace whole lifecycle commands with scripts whose successful exit could mean
something different. Release targets already expose `verify`, `prepare`,
`workflow`, `publisher`, and `completion`; use those boundaries for repository
build and packaging tools. Current PR and release observation supports GitHub;
other hosts need publication, identity and completion support together.
