# Configuration

## Use Claude for design and Codex for implementation

```bash
lf set --global default_agent codex
lf set --global skills.design.agent claude
lf set --global skills.implement.agent codex
```

Design uses Claude, implementation uses Codex, and other work falls back to Codex.
These commands edit `~/.lf/config.yaml`; writing the same YAML has the same effect:

```yaml
# ~/.lf/config.yaml
default_agent: codex
skills:
  design:
    agent: claude
  implement:
    agent: codex
```

Use `agent` for a choice you want applied. Use `default_agent` for a fallback
that lets another applicable `agent` choose differently. The examples below are
separate setups; combine the pieces that fit your work.

## Let a project choose its exceptions

Keep just a fallback in your personal global file:

```yaml
# ~/.lf/config.yaml
default_agent: codex
```

Then give the team a design preference in the repository:

```yaml
# .lf/config.yaml
skills:
  design:
    agent: claude
```

Design uses Claude; implementation and other steps fall back to Codex. This is
useful when you want one everyday default while each project chooses exceptions.

To insist on Codex for design across your projects, make a personal choice:

```bash
lf set --global skills.design.agent codex
```

That personal choice wins over the repository's Claude choice. Change `agent` to
`default_agent` if you want the repository's choice to win again.

## Keep a preference just for this project or Wave

```bash
lf set skills.design.agent claude
lf set skills.implement.agent codex
lf --wave frontend set skills.implement.agent claude
```

The first two commands save your project preferences in `lf.config.local.yaml`.
The third makes implementation use Claude for the `frontend` Wave; the rest of
the project still uses Codex. These private settings do not change the team's files.

To set a shared Wave default instead:

```bash
lf --wave frontend set --shared default_agent claude
```

This writes `.lf/wave/frontend/config.yaml`. It applies when no applicable
`agent` choice takes precedence. [The FAQ](#what-happens-when-a-wave-is-renamed)
explains how preferences follow a renamed Wave.

## Change one step in one Graph

Suppose `release` runs implementation twice:

```yaml
# .lf/graphs/release.yaml
- step: {name: implement, id: write-code}
- gate
- step: {name: implement, id: fix-review}
```

Give implementation a personal default, then make one occurrence an exception:

```yaml
# lf.config.local.yaml
skills:
  implement:
    agent: codex
graphs:
  release:
    steps:
      fix-review:
        agent: claude
```

`write-code` uses Codex; `fix-review` uses Claude. Implementation in other Graphs
still uses Codex. Both choices are in the same personal scope, so the named step
wins over the skill. The equivalent setter is:

```bash
lf set graphs.release.steps.fix-review.agent claude
```

For a shared convention, put that `graphs:` block in `.lf/config.yaml`. A personal
`agent` choice still wins over a shared choice; use a personal `default_agent`
when you want to leave room for shared exceptions.

## Put a convention alongside the skill or Graph step

A repository-owned skill, such as `.lf/skills/design/SKILL.md`, can carry its own fallback:

```markdown
---
default_agent: claude
---
Help develop the design.
```

This is another way to author a shared skill preference. Centralize preferences
under `skills:` when you want to see them together; put them in the skill when
they belong with its instructions. Built-in skills recommend no harness.
A repository Graph can make a choice for one occurrence:

```yaml
# .lf/graphs/release.yaml
- step: {name: implement, id: fix-review, agent: claude}
```

That shared choice beats every fallback. A personal `agent` choice still wins.
For Graph authoring, see [Orchestration](orchestration.md).

## Override one launch

```bash
lf -a claude design                   # this design conversation
lf -a codex graph release             # every step in this Graph
lf -a claude resume --task LOO-123     # this Task conversation
```

The Graph override reaches its composed steps and routers. The conversation
override does not change the preferences of Tasks or Graphs you launch from it.
None of these commands saves a new preference.

Other settings use the same typed values as YAML:

```bash
lf --setting 'skills.implement.yolo=false' implement
LF_SETTINGS='{skills: {design: {agent: claude}}}' lf design
```

An explicit command-line choice wins over an environment choice. Shell-exported
settings can reach child commands, so use a command-local assignment when that is
the intended lifetime.

## Give different work different context

```yaml
# .lf/config.yaml
context:
  docs: [README.md]
  exclude: [docs/generated/**]
  diff: true
skills:
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

Design gets the README and product notes; implementation gets the README and
testing instructions. The named release step explicitly disables Chrome.
Lists such as `context.docs` add contributions; `[]` clears earlier contributions.
`false` is an explicit value, while omitting or unsetting a key reveals the next
applicable setting.

Use the same paths in setters and invocation overrides:

```bash
lf set context.exclude '[docs/generated/**]'
lf set skills.design.context.docs '[docs/product.md]'
lf --setting context.diff=false design
```

`context` also holds `diff_files`, `paste` and `budgets`. The existing `--docs`,
`--diff` and clipboard flags address those same settings.

## Choose accounts and where work runs

```yaml
# ~/.lf/config.yaml
account:
  claude: [work@example.com, personal@example.com]
  codex: [work@example.com]
```

Use your connected accounts' login emails, in preference order; an account with
no login email is named by its ID. To use only your work Claude account in this
project:

```bash
lf set account.claude '[work@example.com]'
```

The project list replaces the global list. Accounts apply to a global, repository
or Wave scope; they are not selected per skill.

Once a remote machine is registered, select its durable ID for new Tasks:

```bash
lf set machine.tasks '<machine-id>'
```

Work initiated on your laptop uses the laptop's personal preferences on the remote.
A phone connecting through the laptop uses those preferences too. Work initiated
directly on the remote uses its saved preferences. The executor supplies its own
tools and credentials. See [Remote work](#remote-work) for retaining preferences
there for later use.

## Inspect and edit

```bash
lf config catalog --json
lf config show --skill implement           # compact table of authored preferences
lf config show --skill implement --explain # candidates, files, and winning reasons
lf config show --skill implement --all     # include built-in defaults
lf config show --graph release --step write-code --json
lf context --json
```

The table shows effective values and readable sources and targets. Color follows
the terminal and respects `NO_COLOR`; piped output is plain text. Built-in-only
settings stay hidden unless you pass `--all`; the effective agent always appears.
Agents and context appear first, followed by Machines, Accounts, Desktop,
Planning, Release and Loopflow. Empty sections are hidden. These feature groups
also organize catalogue browsing; each row keeps its actual configuration path.
Use `--explain` for each candidate, its authored key, file and winning reason.
`default_agent` appears within the explanation for `agent`, never as a separate
table row, including with `--all`.
`--json` always includes every setting and candidate, unchanged by display filtering.

Inspection shows the effective value, authored key, source, scope, target and file
location. It explains why candidates won, lost, contributed or cleared earlier
values. A fallback remains identifiable as `default_agent`. CLI and Desktop read
the same resolution used to prepare a launch.

In Desktop Configuration, choose My global preferences, My project preferences or
Shared project preferences. Select a project checkout and Wave for My Wave
preferences or Shared Wave preferences. Apply shows the destination file and when
the change takes effect. Explain uses that same checkout, Wave and setting target;
Unset removes only that declaration and shows what now applies. Changing the
destination reloads its declaration.

The catalogue lists supported keys, value types, destinations and lifecycle.
A successful write reports where it was saved and whether it applies immediately,
on a later launch or only after a domain action. Saving a preference does not
itself log into an account, provision a machine or move existing work.

Setters validate before writing, preserve unrelated content and comments, and
protect against concurrent edits. Hand edits invalidate the same readers. Config
inspection and bootstrap preferences do not require a compatible database; account
or machine availability can still require runtime observations.

## Where settings live

| File | Applies to | Shared through Git? |
| --- | --- | --- |
| `~/.lf/config.yaml` | Your global preferences | No |
| `./lf.config.local.yaml` | Your repository and Wave preferences | No |
| `.lf/config.yaml` | The repository's shared preferences | Yes |
| `.lf/wave/<name>/config.yaml` | The Wave's shared preferences | Yes |

`LF_HOME` changes the personal global directory. Run `set --global` to edit the
personal global file. Bare `set` edits private repository settings; `--wave`
selects a Wave within that private file. Add `--shared` to edit the committed repository or Wave file.

## Which value wins

Resolve each setting independently. For scalar choices, use this source order:

1. Explicit command-line settings.
2. Explicit environment settings.
3. Personal configuration.
4. Repository configuration, including Wave files and repository definitions.
5. Built-in defaults.

Within one source, Wave scope beats repository scope, which beats global scope.
Within the same scope, a Graph occurrence beats a skill, which beats general
settings. A more specific target does not outrank a higher source.

Agent selection first separates **choices** from **fallbacks**: every applicable
`agent` beats every `default_agent`. Apply source, scope and target ordering within
each class. A command-line `default_agent` is still a fallback.

| Declarations | Selected agent |
| --- | --- |
| Personal `default_agent: codex`; repository implement `agent: claude` | Claude |
| Personal implement `agent: codex`; repository occurrence `agent: claude` | Codex |
| Personal implement `default_agent: codex`; repository general `agent: claude` | Claude |
| Explicit `-a codex`; repository occurrence `agent: claude` | Codex |

Lists and compound settings follow their declared merge rules. `context.docs`,
`context.exclude`, `summaries` and `supported_harnesses` add in precedence order; `[]`
clears earlier contributions. Keyed maps merge by key. Account lists preserve
order and replace as a unit. `planning` replaces its complete tagged provider
configuration. `false` is a value; omission and `unset` reveal lower declarations.

## Source, scope and target

Every declaration has three independent dimensions:

| Dimension | Answers | Values |
| --- | --- | --- |
| Source | Who supplied it? | Builtin, repository, personal, environment, command line |
| Scope | Where does it apply? | Global, repository, Wave |
| Target | Which work does it select? | General, a skill, a named occurrence in a Graph |

A personal Wave preference is **personal source, Wave scope**. The same Wave's
committed preference is **repository source, Wave scope**. A file path records
where the declaration was written; it does not create another authority tier.

A setting supports only meaningful combinations. Account order has global,
repository and Wave scopes but no skill target. Appearance has no Graph-step
target. Native provider settings remain owned by the provider. The settings
catalogue supplies the same applicability and validation to files, CLI and Desktop;
unsupported combinations report an explanation rather than being silently ignored.

## Remote work

The initiating LF instance supplies your personal preferences, including LF
settings authored alongside personal skills and Graph steps. Executor personal
skill and Graph preferences do not become extra exceptions. The executor supplies
its environment: installed tools, credentials, local paths and machine identity.
Repository configuration keeps its normal source and scope. Skill instructions
and Graph topology still come from the selected or captured definitions.

| Start work from | Personal preferences used |
| --- | --- |
| Laptop, targeting a remote machine | Laptop's |
| Phone through the laptop's LF server | Laptop's |
| Shell or phone directly on the remote | Remote's saved preferences |

Forward authored declarations with their scope, target and choice/fallback
meaning. Do not turn the effective result into command-line overrides or combine
two personal profiles into an undocumented precedence layer.

The remote can retain portable preferences for subsequent direct work while the
laptop is unavailable. Refresh an unchanged retained copy when new work arrives.
Preserve independently edited remote configuration, including deletions; expose
the difference for explicit reconciliation. Forwarded work can use the incoming
profile even when saving it would overwrite remote edits. Inspection distinguishes
“used for this launch” from “saved for future launches.” Remote edits do not
silently flow back to the laptop.

Retaining definition preferences edits only LF settings in existing personal
skills and Graphs. It preserves instruction bodies, topology, comments and native
provider declarations. Missing definitions or Graph occurrences remain conflicts
until the definitions are installed or reconciled; forwarding does not copy skill
bundles or referenced assets. Captured work retains its instructions and occurrence
identity, while host-local preferences come from the executor's current definitions.

Resolve machine selection once at the origin to a durable identity. A selection
that names the current machine runs locally. Existing assigned work does not
forward itself again. Preferences neither install tools nor transfer credentials;
provider conversation/account continuity still constrains resume.

## Your display name

```bash
lf user
lf user --json
```

Set `user.name` in personal global configuration; omit it or leave it blank to
use Git's configured name. Repository configuration cannot supply someone else's
personal display name. Explicit `LF_USER_NAME` and command-line values retain
normal precedence. Empty invocation values fall through to personal config and
Git. The initiating participant is carried with remote requests.

Names describe people, not authority. Task claims use the repository's Git email
identity. Persisted artifacts name their participant; conversation replies use
“you.” Native history and provider author IDs retain their original attribution.

## Quick Reference

| Behavior | CLI Flag | Config |
|----------|----------|--------|
| Agent | `-a claude:opus` | `agent: claude:opus` |
| [Planning synchronization](work.md#share-the-plan) | — | `planning: {provider: linear, team: "<id>"}`, `planning: {provider: asana, team: "<gid-or-url>"}`, `planning: {provider: notion, team: "<page-url-or-id>"}` or `planning: {provider: git, shared: team}` |
| Interactive terminal | direct TTY or `-i` | — |
| Include docs | `--docs README.md,docs/` | `context: {docs: [README.md, docs/]}` |
| Include branch files | `--diff files` | `context: {diff_files: true}` |
| Include raw diff | `--diff patch` | `context: {diff: true}` |
| Include clipboard | `-c, --clipboard` | — |
| Chrome automation | `--chrome` | `chrome: true` |
| Yolo mode (skip permissions) | `--yolo` | `yolo: true` |
| Account's own provider home | `--isolate` / `--shared` | `isolate: true` |
| Review GraphStep terminal | — | global-only `session.terminal: Ghostty` |
| End agents nobody is viewing | — | `session.end_detached_after: 2m` (or `off`) |

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
context:
  budgets:
    memory_tokens: 6000
    scratch_tokens: 8000
```

Override individual fields for a Wave in `.lf/wave/<name>/config.yaml`:

```yaml
context:
  budgets:
    memory_tokens: 10000
```

Each field follows the same source and scope precedence. `lf context` shows the winning source for every value.
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
| Explicit docs | Complete reference bodies in the submitted turn | `context.docs` in config or Loopflow skill frontmatter, or `--docs` |
| Branch changes | Git inspection commands and changed paths | `--diff patch`, `context: {diff_files: true}` |
| Captured references | Complete private reference files | Work direction supplied by the caller |
| Clipboard | Tagged block after the request in the first turn | `-c` |

The block refreshes at startup and after compaction. After compaction it also
includes the saved active skill. Large documents include marked start excerpts
with complete source paths and sizes.
The rendered block stays within the provider byte cap; overflow listings and skills
remain readable through complete-file pointers.

## Graphs

Graphs are YAML files in `.lf/graphs/`. Repository definitions win over
same-named personal `~/.lf/graphs/` definitions, then builtins:

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
| **Config** | `context: {docs: [PATH, PATH]}` |
| **Default** | none (empty) |

Each entry is a file (`README.md`), a glob (`'*.md'`), or a directory (`swift/`
gathers `*.md` under it). Complete UTF-8 bodies and source paths enter the submitted
turn, including absolute files outside the repository. Preview and durable capture
show those submitted bytes. Dollar signs in reference text use `&#36;` notation
to avoid invoking native skills. Empty files remain labeled references; missing,
unreadable or nontext exact selections fail with the path and reason.

Exclusions and the 100-file ceiling still apply. Explicitly selecting an ambient
file supplies it once in task text instead of the refreshed block. New `--docs`
selections on `lf -b resume ID --message TEXT` belong to that turn; prior documents
stay in native history. This doesn't restrict which files the agent can edit.
Scratch and the selected Wave's checkout Markdown are gathered automatically.
Wave context reads `.lf/wave/<name>/` and each path-segment ancestor, excluding siblings
and children; listings point to those files, not database snapshots.

### Branch Files (diff_files)

List paths modified on the current branch for the agent to inspect.

| | |
|---|---|
| **CLI** | `--diff files` / `--diff none` |
| **Config** | `context: {diff_files: true}` |
| **Default** | `false` |

Use `--diff files` to identify changed files. The reference points to Git inspection;
it does not preload file bodies or patches.

### Clipboard

Append clipboard content in its own tagged block after the request in the first turn.
It counts toward the terminal argument cap.

| | |
|---|---|
| **CLI** | `-c, --clipboard` |
| **Config** | `context: {paste: true}` (local to the host) |
| **Default** | not included |

Use `-c` when debugging: copy an error, then `lf debug -c`.

### Raw Diff

Point to Git inspection commands rather than inlining a frozen patch.

| | |
|---|---|
| **CLI** | `--diff patch` / `--diff none` |
| **Config** | `context: {diff: true}` |
| **Default** | `false` (not included) |

Use when the agent needs to inspect exact changes. `--diff both` also lists changed paths.

### Exclude Patterns

Git-ignore-style patterns that remove matching documents and changed-file contents
from the context LF supplies. They do not restrict the agent’s own file access
or filter its searches or Git commands.

| | |
|---|---|
| **Config** | `context: {exclude: ["*.test.ts", node_modules, dist]}` |

---

### Agent

Choose a harness, with an optional model. Use `default_agent` for a fallback.

| | |
|---|---|
| **CLI** | `lf gate -a codex:o3` |
| **Config** | `agent: claude:opus` (optional) |
| **Default** | unset (uses applicable authored fallbacks, then the sole installed harness) |

```yaml
agent: codex          # harness default
# agent: claude:opus  # harness plus model
skills:
  implement:
    agent: claude:opus
```

Harnesses: `claude`, `codex`, `opencode`, `pi`. Use `harness:model` for specific models.

Built-in skills recommend no harness. With no authored choice or fallback,
Loopflow selects automatically only when exactly one supported harness is installed.
With multiple harnesses, save your choice with `lf set --global default_agent claude`
or select one command with `-a claude`. Start setup with `lf -a claude init` to
choose and save a personal default. Installation order and remaining usage do not
choose a harness.
Inspection still works without a choice; a new launch explains how to select one.
Existing Sessions retain their recorded provider when resumed.
A personal `skills.implement.agent: codex` beats a repository occurrence's
`agent: claude`; changing that personal key to `default_agent` lets the repository
choice win, even if it is general. `-a` explicitly selects the invocation's agent.
Both keys work in CLI `--setting`, `LF_SETTINGS`, personal and shared files,
LF-owned skill metadata, and Graph occurrence settings.

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
change the saved history's provider. A `default_agent` can refresh the model for
that provider; switching providers requires an explicit agent choice. CLI overrides
take precedence. Connecting
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

Reserved configuration for codebase overviews. No launch currently generates or loads
these summaries; setting them does not start a paid summarization run.

```yaml
summary_tokens: 25000

summaries:
  - path: src
  - path: lib
    tokens: 5000
```

### Accounts and Profiles

Account registration uses domain commands. Ordered account preferences use the
same file-backed settings as other declarations:

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
Headless commands retain the defining Graph, authored step ID and source precedence
in invocation files beside the recipe. Skill definitions and personal, repository, environment and CLI preferences
resolve in the launch repository. Sync refreshes and prunes
these captures with their recipe, preserving unrelated bundle assets.

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
sends no method or commit-message override for queued merging. Personal preferences can override the shared preference, subject to GitHub's allowed methods. Every request retains the observed head
commit check and GitHub's protection requirements.

Keep procedure in repository instructions and Graphs: who should review, when to
run checks, and whether to stop at `publish`, `submit`, or auto-merge. Do not
replace whole lifecycle commands with scripts whose successful exit could mean
something different. Release targets already expose `verify`, `prepare`,
`workflow`, `publisher`, and `completion`; use those boundaries for repository
build and packaging tools. Current PR and release observation supports GitHub;
other hosts need publication, identity and completion support together.

## FAQ

### How do Task worktrees and independent clones share preferences?

Independent clones have independent private preferences, even when they have the
same Git remote. This includes account order and machine preferences. New clones
start without another clone's private overrides; copy or import them deliberately.
Task worktrees share the main checkout's `lf.config.local.yaml`. LF resolves that
file through Git's worktree metadata; a Task's working directory does not give it
another private preference file. The first LF write adds `/lf.config.local.yaml`
to the repository's private Git exclude file if needed. It preserves existing
ignore rules and does not edit the committed `.gitignore`. An already tracked
file needs to be removed from tracking before Git can ignore it.

### What happens when a Wave is renamed?

Renaming a Wave in Desktop uses the same operation as `lf wave rename`. Its
`.lf/wave/<name>/` directory moves with its shared `config.yaml`, while the Wave's
ID stays the same.

Private Wave declarations are keyed by the Wave's authored UUID under `waves:`.
`--wave <name>` resolves that identity from its authored files; `--wave <UUID>`
addresses it directly. Rename and relocation retain the UUID. Reusing a retired
name does not reuse its preferences.

### What stays in the database?

Files hold declared preferences, including account order, preferred machines,
pool availability, rental templates and admission limits. The database holds live state and observations: account usage,
cooldowns, machine identity, Sessions, execution history and actual saved Task
arrangements. A saved arrangement remains live workspace state.
Credentials stay with their secret/provider owner. The database is not an
additional preference override layer.

### How are skill and Graph-step settings addressed?

The skill rule applies wherever `implement` runs. The Graph rule applies to
`id: fix-review` authored in `release`. Reusing that Graph applies the rule to
each copy. Composition and loops retain the defining Graph and original step ID;
another Graph's same-named step does not match. Steps without IDs use skill and
general settings. Duplicate IDs within one definition are invalid.

```bash
lf set --global skills.implement.default_agent codex
lf set graphs.release.steps.write-code.agent claude
lf config show --graph release --step write-code --json
lf config show --graph outer --json
```

Whole-Graph inspection includes composed steps and routers. Targeting
`outer/work` does not select a step authored only in `inner/work`. Navigation
labels such as `review#1` do not change setting addresses.

Launch targets support agent selection and context/launch preferences, including
context budgets. Planning, release policy and a Task's default Graph describe
broader work; they do not acquire skill targets merely because their values are configurable.
Theme, installation stream, portfolio and terminal preferences retain their
appropriate lifecycle. Reserved summary settings do not start a summarization run.

### What can an environment override inherit?

`--setting KEY=YAML` uses the same typed values as configuration files.
`LF_SETTINGS` accepts a YAML mapping. Explicit `LF_AGENT_OVERRIDE`,
`LF_YOLO_OVERRIDE`, `LF_CHROME_OVERRIDE`, `LF_USER_NAME` and
`LF_ACCOUNT_ISOLATION` retain their invocation meanings. User-supplied environment
can flow into children; LF does not export a resolved preference as an environment
override. Internal process identity variables carry execution context, not settings.
Neither environment nor command-line choices are saved as future preferences.

### How do I migrate or recover older preferences?

```bash
lf config migrate                         # inspect the existing preference cutover
lf config migrate --accept                # accept file precedence when declarations collide
lf config import ~/.lf/settings-recovery.yaml --pointer '/repo/owner~1project'
lf config retained                        # compare incoming and retained remote profiles
lf config retained --keep                 # preserve independent remote edits
```

Migration retains before/after files in `settings-cutover.json`. Unavailable legacy
repository destinations remain in `settings-recovery.yaml`, which is never read
as configuration. Import deliberately selects the destination clone; conflicting
saved declarations require `--replace`. Future clones inherit nothing automatically.
Installer previews leave live preference files untouched; promotion applies the cutover.

### Moving from flat context settings

The new namespace requires a release that supports it. Until that release is
installed everywhere a file is read, keep its released shape. The new reader
accepts flat `docs`, `exclude`, `diff`, `diff_files`, `paste` and `context_budgets`
during rollout; setters and inspection use `context.*`. The former `context`
path list becomes `context.docs`, combined after `docs` when both were authored
in the same block. Explicit new fields win over old spellings in that block.
Task 807ef7 removes this transition reader after migration.
