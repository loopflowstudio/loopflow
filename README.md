# Loopflow

```bash
curl -fsSL https://github.com/loopflowstudio/loopflow/releases/latest/download/install.sh | sh
lf init
lf debug -c
```

A software instrument. Give an agent a task, keep its conversation, and resume
captured work when a command stops. Loopflow keeps the command's result, the
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
It bundles `lf`; open it explicitly with `lf desktop`. Bare `lf` starts the
general-purpose terminal conversation. On canonical main, it first carries local
commits and uncommitted files into an author-scoped sibling worktree so the
conversation cannot dirty main.

```bash
lf                         # follow the conversation wherever it goes
lf operate                 # review and operate this repository’s work
lf repo/operate            # canonical name for the same skill
lf --wave designer wave/operate  # operate one Wave
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
External skills honor the same frontmatter as local skills, on first fetch and
when read from cache. Malformed definitions report their parse error.

Flows invoke builtin commands with `cmd:`, for example `- cmd: pr land`.
See [Authoring](docs/authoring.md) for composition and review boundaries.

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
lf --wave designer wave/operate        # one finite planning pass
lf --wave designer wave/operate "ship the button audit first"
lf --wave designer wave/operate        # one finite planning pass
```

Edit `wave/designer/MEMORY.md` directly when durable context changes; it is a
reviewed repository file, not live server state.

Delegate durable work — the same verbs whether the caller is you or the wave:

```bash
lf checkout INF-123                               # durable Task Work + worktree, no controller
lf --task INF-123 flow start                                   # start end-to-end Task automation
lf comment INF-123 "take the smaller approach"   # post a Linear comment for the Task advancer
lf interrupt INF-123                             # end this turn so fresh direction is read now
lf --task INF-123 research "write scratch/runtime.md"    # one independent Task conversation
lf restart INF-123 "reconcile all scratch first" # checkpoint and begin a new kickoff
lf task status INF-123 --json                         # inspect planning, even when sync is unavailable
lf task/operate "INF-123"                           # advance until landed or blocked; link the blocking Session
lf arm -c                                          # request exact-head auto-merge and return
lf land -c                                         # hand off delivery; complete the Task after verified merge
lf pr reconcile                                      # check recorded deliveries once and settle merges
lf ci watch                                          # watch PR checks; start a ci-fix when a landing fails
```

Task comments in Linear also reach the advancing worker. Steering never starts
an idle Task or broadcasts to independent conversations.

Turn a reviewed design into work without another planning subsystem:

```bash
lf design                                             # author and review one design
lf launch-plan                                        # keep the core here; launch independent Tasks
```

Watch this repository and the current Home:

```bash
lf wave list                  # every durable Wave and its Home/runtime evidence
lf user           # display name from Git or a personal Loopflow override
lf roadmap             # every open Task across this repository's Waves
lf roadmap --all       # every repository on this machine
lf wave status designer     # one Wave's current chapter and Tasks
lf activity            # durable work, delivery and steering history
lf session list --json # conversations on this Home
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
lf session list --interactive false --history --json
lf session connect SESSION
lf session history SESSION --json
lf session rename SESSION "Release notes"
lf session bind SESSION --task INF-123
lf ready "Ready for review"  # inside an Ask or review
lf session complete SESSION        # return its saved feedback
```

A Session keeps the conversation's identity, name, feedback and native history
across commands. Interactive and headless work use the same model. Default lists
show interactive conversations; explicit filters reveal headless or completed
ones. `--all` means all repositories.

Ready saves feedback. Complete ends an Ask or review; a Flow's following decision
chooses navigation. Closing a pane or exiting a provider does not complete a review.
Bind assigns an unbound conversation to one Task permanently, including a done Task.

## The model

| Object | What it does | Where it lives |
| --- | --- | --- |
| **Skill** | Gives the agent reusable instructions and context | `.lf/skills/*.md` |
| **Flow** | Composes agent work, mechanical operations and reviews | `.lf/flows/*.yaml` |
| **Wave** | Keeps the objective, memory, cadence, budget and instruments | `wave/<name>/` |
| **Project** | Holds one Wave's plan, Tasks, KRs, targets and default Flow | Linear |
| **Chapter** | Names the repository's current group of In Progress Projects | Linear Project names and statuses |
| **Task** | Owns concrete work, its checkout, serial PRs and managed Flow selection | Linear and local SQLite |
| **Exec** | Records one actual lf process and its observed command outcome | Home-local SQLite |
| **AgentSession** | Keeps a continuable interactive or headless conversation and native history | Home-local SQLite and provider-native storage |
| **FlowSession** | Keeps a captured Flow and consumes exact boundary completions | Home-local SQLite |
| **Home** | Places execution and scopes its store, credentials and process authority | Machine identity and local data |

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
