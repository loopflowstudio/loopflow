# Loopflow

Loopflow creates and runs **Waves** — persistent agents that work toward an
outcome. Write a Wave's goal once; it coordinates Linear-backed Projects and
Tasks, remembers what it learns, and keeps one steerable conversation beside
the live work map.

Everything is one binary. `lf` is the command humans type *and* the API agents
call to launch, steer, and observe other agents. There is no centralized server: state
lives in a local SQLite store and append-only journals, shared truth lives in
Linear and GitHub, and remote machines are reached over plain SSH.

## Install

```bash
curl -fsSL https://github.com/loopflowstudio/loopflow/releases/latest/download/install.sh | sh
lf init
```

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
It bundles `lf`; `lf` (bare) opens it.

Give an external agent harness the Loopflow operating skill:

```bash
npx skills add loopflowstudio/loopflow --skill loopflow -g -y
```

The harness acts as a User over the same `lf` API. See
[The Agent API](docs/agent-api.md).

## A taste

Fix a bug from the clipboard:

```bash
lf debug -c            # paste an error, watch it fix
```

Author a wave — two files in your repo — and run it:

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
lf stop designer
```

Edit `wave/designer/MEMORY.md` directly when durable context changes; it is a
reviewed repository file, not live server state.

Delegate durable work — the same verbs whether the caller is you or the wave:

```bash
lf task run INF-123                                   # durable Task Work, own worktree
lf task steer INF-123 "take the smaller approach"     # redirect the active turn
lf task status INF-123 --json                         # inspect durable state
lf pr land -c                                         # merge the PR, complete the Task
```

Watch the whole machine:

```bash
lf ls                  # every durable Wave and its Home/runtime evidence
lf roadmap             # every open Task across every wave, bucketed by need
lf status designer     # one wave's live Project → Task hierarchy
lf activity            # durable Work changes with exact Run, PR, and Steer proof
lf trace <exec-id>     # what one agent did — and exactly what it was told
lf ps                  # rank live call trees by cumulative completed output
lf top                 # refresh call-tree rates, age, idle time, and health
lf prune --dry-run     # inspect dead receipts and registered orphan providers
lf usage               # subscription state and spend, per account and repo
lf performance         # 14-day latency, verification, and spend scorecard
```

## The model

| Atom | What it does | Where it lives |
|------|--------------|----------------|
| **Skill** | Runs a prompt with assembled context | `.lf/skills/*.md` |
| **Flow** | Chains skills together | `.lf/flows/*.yaml` |
| **Wave** | Durable operating context: memory, cadence, chat, project selection | `wave/<name>/` |
| **Project** | Measured bet inside exactly one wave | Linear, via `lf pm` |
| **Task** | Concrete work; its Work owns the only delivery worktree | Linear, via `lf pm` |
| **Home** | Stable execution authority; its route may move without changing identity | local SQLite |

| Built-in | What it does |
|----------|--------------|
| `testing-audit` | Finds low-value tests and redundant verification, then improves the workflow |
| `token-compress` | Fits a complete artifact to an explicit token budget without truncating it |

## Docs

Rendered at [loopflow.studio/docs](https://loopflow.studio/docs), served from
this repo. For agents: every page is raw markdown at its `.md` URL, indexed
at [loopflow.studio/llms.txt](https://loopflow.studio/llms.txt) with the full
corpus at [/llms-full.txt](https://loopflow.studio/llms-full.txt).

| Page | Covers |
|------|--------|
| [Get Started](docs/getting-started.md) | Install, first commands, building features, going remote |
| [Waves](docs/waves.md) | The planning model, goals, memory, KRs, Linear, crons |
| [The Agent API](docs/agent-api.md) | How agents launch, steer, and prove control of other agents |
| [Conducting](docs/conducting.md) | Monitoring and steering many agents; the Mac podium |
| [Authoring](docs/authoring.md) | Writing skills, flows, directions, and goals |
| [Architecture](docs/architecture.md) | Concepts, truth owners, persistence, processes, public APIs, and provider edges |
| [Security](docs/security.md) | Execution boundaries, permissions, credentials, and account authority |
| [`lf` reference](docs/lf.md) | Every command, PR/planning/release operations, the builtin catalog |
| [Configuration](docs/config.md) · [Troubleshooting](docs/troubleshooting.md) | Reference |

## Developing loopflow

```bash
uv run python scripts/install.py local --use   # build lf + lfd + Loopflow.app from this checkout, make active
uv run python scripts/install.py refresh       # fast control-plane rebuild from the default branch
```

`TESTING.md` covers the test suites; `STYLE.md` is the governing style guide;
`RELEASE_NOTES.md` and `release/` carry the release chronology.

## License

MIT
