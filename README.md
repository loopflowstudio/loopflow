# Loopflow

Run one useful prompt first:

```bash
curl -fsSL https://github.com/loopflowstudio/loopflow/releases/latest/download/install.sh | sh
lf init
lf debug -c        # copy an error to the clipboard, watch it fix
```

That command assembles the skill, repository guidance, scratch notes, and
clipboard into one prompt. It launches the configured provider and writes one
Home-local Run record. The Run records what happened; it does not reserve the
repository, control a Wave, or become planning state.

The same building block runs **Waves**: persistent agents that coordinate
Linear-backed Projects and Tasks, remember what they learn, and stay steerable.
There is no Loopflow server at the center. Repo files hold authored behavior;
Linear and GitHub hold shared coordination and delivery facts; each Home keeps
its local execution records. `lf ssh` runs the same local commands on another
Home.

## More install options

Requires macOS or Linux and one of
[Claude Code](https://docs.anthropic.com/en/docs/claude-code),
[Codex](https://github.com/openai/codex), or
[OpenCode](https://github.com/anomalyco/opencode). Default install location is
`~/.local/bin` (`LF_INSTALL_DIR` overrides). Or install with cargo:

```bash
cargo install --git https://github.com/loopflowstudio/loopflow --bin lf
```

The Mac app — wave chat, the machine-wide roadmap, every task's worktree — is
[`Loopflow-latest.dmg`](https://downloads.loopflow.studio/Loopflow-latest.dmg).
It bundles `lf`; open it explicitly with `lf desktop`. Bare `lf` starts the
terminal control conversation. On canonical main, it first carries local
commits and uncommitted files into an author-scoped sibling worktree so the
conversation cannot dirty main.

Give an external agent harness the Loopflow operating skill:

```bash
npx skills add loopflowstudio/loopflow --skill loopflow -g -y
```

The harness acts as a User over the same `lf` API. See
[The Agent API](docs/agent-api.md).

## Keep work moving

Author a Wave in the repo and run it:

```markdown
<!-- wave/designer/GOAL.md -->
## Objective

Keep the design system coherent. Each wake: read the Linear Projects and
Tasks, direct the Project with the highest-leverage open KR, start a concrete
Task only after it has a Linear issue, and fold what changed into memory.
```

```bash
lf start designer                           # serve it from this Home's one keeper
lf chat --steer "ship the button audit first"
lf pause designer                           # keep listening; queue new turn starts
lf resume designer
lf stop designer                            # stay off across Home restarts
lf start designer                           # turn it back on
```

Edit `wave/designer/MEMORY.md` directly when durable context changes; it is a
reviewed repository file, not live server state.

Delegate durable work — the same verbs whether the caller is you or the wave:

```bash
lf task prepare INF-123                               # durable Task Work + worktree, no controller
lf project prepare runtime-model                      # durable Project Work, no controller
lf task run INF-123                                   # start end-to-end Task automation
lf task steer INF-123 "take the smaller approach"     # queue direction; wake its controller if installed
lf --task INF-123 research "write scratch/runtime.md"    # one independent Task-bound Run
lf task restart INF-123 "reconcile all scratch first" # checkpoint and begin a new kickoff
lf task status INF-123 --json                         # inspect durable state
lf pr arm -c                                          # request exact-head auto-merge and return
lf pr land -c                                         # watch, repair CI, merge, then complete the Task
```

Turn a reviewed design into work without another planning subsystem:

```bash
lf design                                             # author and review one design
lf launch-plan                                        # keep the core here; launch independent Tasks
```

Watch this repository and the current Home:

```bash
lf ls                  # every durable Wave and its Home/runtime evidence
lf roadmap             # every open Task across this repository's Waves
lf roadmap --all       # every repository on this machine
lf status designer     # one wave's live Project → Task hierarchy
lf activity            # durable Work changes with exact Run, PR, and Steer proof
lf runs                # recent Home-local Run records
lf runs run_ab12 --events # inspect one Run's append-only evidence
lf replay run_ab12     # repeat its recorded provider request as a child Run
lf usage --days 30     # direct provider-authored usage for those Runs
lf ps                  # one OS-live Loopflow process snapshot
lf top                 # refresh elapsed time, process state, and call trees
lf prune --dry-run     # inspect dead receipts and registered orphan providers
```

## The model

| Atom | What it does | Where it lives |
|------|--------------|----------------|
| **Skill** | Runs a prompt with assembled context | `.lf/skills/*.md` |
| **Flow** | Chains skills together | `.lf/flows/*.yaml` |
| **Wave** | Durable operating context: memory, cadence, chat, project selection | `wave/<name>/` |
| **Project** | Measured bet inside exactly one wave | Linear, via `lf pm` |
| **Task** | Concrete work; its Work owns the only delivery worktree | Linear, via `lf pm` |
| **Run** | Append-only evidence from one harness launch | `$LF_HOME/runs/` on the executing Home |
| **Home** | Stable machine identity; its SSH route may move | local SQLite |

| Built-in | What it does |
|----------|--------------|
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
| [Conducting](docs/conducting.md) | Monitoring and steering many agents; the Mac podium |
| [Authoring](docs/authoring.md) | Writing skills, flows, directions, and goals |
| [Security](docs/security.md) | Execution boundaries, permissions, credentials, and account authority |
| [`lf` reference](docs/lf.md) | Every command, PR/planning/release operations, the builtin catalog |
| [Configuration](docs/config.md) · [Troubleshooting](docs/troubleshooting.md) | Reference |

## Developing loopflow

```bash
uv run python scripts/install.py local --use  # build and pin this checkout against a disposable Home
uv run python scripts/install.py refresh      # return to the latest published release and reliable Home
uv run python scripts/install.py local        # build only under local-bin/
```

`TESTING.md` covers the test suites; `STYLE.md` is the governing style guide;
`RELEASE_NOTES.md` and `release/` carry the release chronology.
Loopflow maintainers should use the repository resource envelope and affected
suite runner documented in [TESTING.md](TESTING.md#bounded-and-honest).

## License

MIT
