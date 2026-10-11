# Loopflow

```bash
curl -fsSL https://github.com/loopflowstudio/loopflow/releases/latest/download/install.sh | sh
lf init
lf debug -c
```

A software instrument. Give an agent a task, keep its conversation, and inspect
captured work after a command stops. Loopflow keeps the command's result, the
agent's history and the Flow's progress separate, so each answers one question.

Free and open source. Needs [Claude Code](https://docs.anthropic.com/en/docs/claude-code)
or [Codex](https://github.com/openai/codex), which have their own cost.

## More install options

```bash
lf install                  # update to the latest published release, from any directory
lf schedule         # check at login and weekly (macOS)
lf schedule daily   # also accepts weekly, hourly, 5min
```

Run `lf install` from any checkout, including a Task checkout. Promotion checks
the candidate release against the installed database before changing it.
Use `lf sync` inside a repository to update its checkout.

Requires macOS or Linux and one of
[Claude Code](https://docs.anthropic.com/en/docs/claude-code),
[Codex](https://github.com/openai/codex), or
[OpenCode](https://github.com/anomalyco/opencode). Default install location is
`~/.local/bin` (`LF_INSTALL_DIR` overrides). Or install with cargo:

```bash
cargo install --git https://github.com/loopflowstudio/loopflow --bin lf
```

The Mac app — Sessions, the roadmap, every Task's worktree — is
[`Loopflow-latest.dmg`](https://downloads.loopflow.studio/Loopflow-latest.dmg).
It bundles `lf`; open it explicitly with `lf open`. Bare `lf` starts the
general-purpose terminal conversation. On canonical main, it first carries local
commits and uncommitted files into an author-scoped sibling worktree so the
conversation cannot dirty main.

```bash
lf                         # follow the conversation wherever it goes
lf operate                 # review and operate this repository’s work
lf repo-operate             # canonical name for the same skill
lf --wave designer wave-operate  # operate one Wave
```

Give an external agent harness the Loopflow operating skill:

```bash
npx skills add loopflowstudio/loopflow --skill loopflow -g -y
```

The harness acts as a User over the same `lf` API. See
[The Agent API](docs/agent-api.md).

## Find a command or workflow

```bash
lf list                    # commands, skills, and flows
lf help debug              # inspect a definition without launching it
lf run feature             # select a flow, otherwise a skill
lf skill release-run       # explicitly select the skill
lf land --help           # inspect the landing command
lf pr-review               # build an HTML walkthrough of the important code in this PR
```

Omit owners or abbreviate commands when the result is unique. Commands take precedence over definitions; help
and list stay local.
A same-named flow takes precedence in untyped execution; invalid flows report
an error. Use `lf skill NAME` to select the skill explicitly.
Skills resolve from repository and personal Claude, Codex, and Loopflow folders.
Help names the selected source; bundled reference files stay inside their skill.
See [skill sources](docs/config.md#skill-sources) for precedence and export behavior.

Flows invoke builtin commands with `cmd:`, for example `- cmd: pr land`.
See [Authoring](docs/authoring.md) for composition and review boundaries.

## Run a Flow in chat

```bash
lf sync-skills --yes        # export personal skills and builtin Flows
lf sync-skills --repo       # export this repository’s skills and Flows locally
```

Select `/pursue` in Claude Code or `pursue` in Codex’s skill picker to follow
its generated checklist: headless commands, conversational reviews, and loops.
Flow exports yield to existing skills with the same name. See
[chat Flows](docs/authoring.md#flows-in-chat) for boundaries.

## Keep work moving

Author a Wave in the repo and open its conversation:

```markdown
<!-- wave/designer/GOAL.md -->
## Objective

Keep the design system coherent. Each wake: read the chapter plan and
Tasks, pursue the highest-leverage open KR, start a concrete
Task only after it has a Linear issue, and fold what changed into memory.
```

```bash
lf --wave designer wave-operate        # one finite pass: keep started Tasks moving
lf --wave designer wave-operate "ship the button audit first"
lf session ensure -w designer          # the Wave's ongoing conversation, applying that pass on every return
```

Edit `wave/designer/MEMORY.md` directly when durable context changes; it is a
reviewed repository file, not live server state.

Delegate durable work — the same verbs whether the caller is you or the wave:

```bash
lf checkout INF-123                               # durable Task Work + worktree, no controller
lf -b task run INF-123                            # place the Task, then run its Flow here until it ends
lf task run INF-123 pursue                        # on a workflow: take the edge that runs pursue
lf comment INF-123 "take the smaller approach"   # post a Linear comment for the Task's running Flow
lf interrupt INF-123                             # end this turn so fresh direction is read now
lf --task INF-123 research "write scratch/runtime.md"    # one independent Task conversation
lf task run INF-123 --reason "reconcile all scratch first" # publish direction, then run fresh work
lf task status INF-123 --json                         # inspect planning, even when sync is unavailable
lf task complete INF-123                             # complete without moving the Workflow
lf task reopen INF-123                               # reopen planning; keep the Workflow and PR
lf task-operate "INF-123"                           # advance through follow-through to completion, or a blocker
lf --task INF-123 skill task-session               # ongoing conversation that keeps applying task-operate
lf arm                                             # request exact-head auto-merge and return
lf land                                            # request merge; follow-through completes the Task
lf pr reconcile                                      # check recorded deliveries once and settle merges
lf ci watch                                          # watch PR checks; start a ci-fix when a landing fails
```

Task comments in Linear also reach the Task's running Flow. Steering never starts
an idle Task or broadcasts to independent conversations. A stopped Flow is
history: `task run` never continues it, so inspect `lf task status` first.

Turn a reviewed design into work without another planning subsystem:

```bash
lf design                                             # author and review one design
lf launch-plan                                        # keep the core here; launch independent Tasks
```

Watch this repository and the current Machine:

```bash
lf wave list                  # every durable Wave and its Machine/runtime evidence
lf user           # read config: display name from Git or a personal override
lf roadmap             # every open Task across this repository's Waves
lf roadmap --all       # every repository on this machine
lf wave status designer     # one Wave's current chapter and Tasks
lf activity            # durable work, delivery and steering history
lf session list --json # conversations on this Machine
lf context             # context budgets, configuration sources and current usage
lf usage --days 30      # recorded provider usage
lf usage --task LOO-265 # usage attributed to one Task
lf usage --task LOO-265 --context # each step's input by source, flagged over budget
lf ps                  # one OS-live Loopflow process snapshot
lf top                 # refresh elapsed time, process state, and call trees
lf mon prune --dry-run     # inspect dead receipts and registered orphan providers
```

## Sessions

```bash
lf session list
lf resume                         # last interactive Session in this worktree
lf resume SESSION                 # Loopflow, Claude, or Codex ID
lf -b resume SESSION "Continue"    # headless turn in the saved Session workspace
lf session list --interactive false --history --json
lf session connect SESSION
lf session history SESSION --json
lf session rename SESSION "Release notes"
lf session bind SESSION --task INF-123
```

Claude and Codex terminal Sessions load assembled context from a system instructions
file, keeping the initial command-line message short even with large Wave memory.
New Session names use a short excerpt of the request, or the invoked skill when
no request was supplied. Task conversations use the Task title when no specific
purpose was supplied. Human-assigned names survive reconnects and suggestions.
Interactive terminal titles use the Session name, with the Task identifier first
and a short purpose for Task work. In cmux, the attached workspace and tab follow
`lf session rename`; other terminals pick up a changed name on reconnect.

Published installs also add native naming hooks, preserving existing hooks. In
repositories with `.lf`, plain Claude and Codex use the first request as a short
name when unnamed. Existing names stay, including instruction-derived titles;
preservation during concurrent renames remains unproved. Review the installed
hooks with Codex's `/hooks`.
Claude naming also works with `claude -p`. Codex naming requires its shared
app-server; embedded launches and `codex exec` lack that naming endpoint.
No resume hooks repair old conversations. Native providers own their terminal output.

A Session keeps the conversation's identity, name, feedback and native history
across commands. Interactive and headless work use the same model. Default lists
show interactive conversations; explicit filters reveal headless or completed
ones, and `--waiting` keeps those waiting on you. `--all` means all repositories.

`lf resume` is short for `lf session resume`. It selects the latest human message
in this worktree, falling back per Session to its last opening when native input
history is unavailable. Assistant output and background work do not change that
order. A Flow is never resumed; `lf task run ISSUE` runs a fresh one.
Headless continuation reads the saved workspace's context and native history,
even from another directory, without re-running the original skill.

Bind assigns an unbound conversation to one Task permanently, including a done Task.

## The model

| Object | What it does | Where it lives |
| --- | --- | --- |
| **Skill** | Gives the agent reusable instructions and context | `.lf/skills/*.md` |
| **Flow** | Composes agent work and mechanical operations; a run is one lf process and the step processes it starts | `.lf/flows/*.yaml` |
| **Workflow** | Gives a Task nodes, where you take part in its conversation, joined by the Flows between them | `.lf/workflows/*.yaml` |
| **Wave** | Keeps the objective, memory, cadence, budget and instruments | `wave/<name>/` |
| **Project** | Holds one Wave's plan, Tasks, KRs, targets and the workflow its Tasks take up | Linear |
| **Chapter** | Names the repository's current group of In Progress Projects | Linear Project names and statuses |
| **Task** | Owns concrete work, its checkout, optional PR and every Flow process for it | Linear and local SQLite |
| **Process** | Records one actual lf process and its observed command outcome | Machine-local SQLite |
| **LfSession** | Keeps a continuable interactive or headless conversation and native history | Machine-local SQLite and provider-native storage |
| **Machine** | Places execution and scopes its store, credentials and process authority | Machine identity and local data |

The [execution contract and cutover status](docs/architecture-reference.md#cutover-status)
separate this model from the remaining implementation. Existing historical commands
and wire fields are documented in the CLI reference; Run is not a fourth execution
object in the model.

| Built-in | What it does |
| --- | --- |
| `testing-audit` | Finds low-value tests and redundant verification, then improves the workflow |
| `token-compress` | Fits a complete artifact to an explicit token budget without truncating it |

## Docs

Read the HTML docs at [loopflow.studio/docs](https://loopflow.studio/docs).
Their reviewed source lives in this repo; agents can request raw Markdown from
each `.md` URL, use the curated
[llms.txt](https://loopflow.studio/llms.txt), or load the full corpus from
[llms-full.txt](https://loopflow.studio/llms-full.txt).

| Page | Covers |
|------|--------|
| [Get Started](docs/getting-started.md) | Install, first commands, building features, going remote |
| [Waves](docs/waves.md) | The planning model, goals, memory, KRs, Linear, crons |
| [The Agent API](docs/agent-api.md) | How agents launch, steer, and prove control of other agents |
| [Conducting](docs/conducting.md) | Seeing what got done, what needs you, and how to step in |
| [Authoring](docs/authoring.md) | Writing skills, flows, and goals |
| [Security](docs/security.md) | Execution boundaries, permissions, credentials, and account authority |
| [`lf` reference](docs/lf.md) | Every command, PR/planning/release operations, the builtin catalog |
| [Glossary](docs/glossary.md) · [Configuration](docs/config.md) · [Troubleshooting](docs/troubleshooting.md) | Reference |

## Developing loopflow

```bash
lf install                                   # install the latest published Loopflow from anywhere
lf schedule                          # update Loopflow at login and weekly (macOS)
lf sync                                      # refresh main and integrate it into this worktree
uv run python scripts/install.py local        # build only under local-bin/
LF_HOME="$(mktemp -d)" local-bin/lf wave list --json # run a disposable experiment
```

`TESTING.md` covers the test suites; `AGENTS.md` is the governing style guide;
`RELEASE_NOTES.md` and `release/` carry the release chronology.
Loopflow maintainers should use the repository resource envelope and affected
suite runner documented in [TESTING.md](TESTING.md#bounded-and-honest).

## License

[MIT](LICENSE)
