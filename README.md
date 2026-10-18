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
LF_INSTALL_CLI_ONLY=1 lf install # update the CLI while keeping Desktop and its clients running
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
It bundles `lf`; open it explicitly with `lf desktop open`. Bare `lf` starts the
general-purpose terminal conversation. On canonical main, it first carries local
commits and uncommitted files into an author-scoped sibling worktree so the
conversation cannot dirty main.

```bash
lf                         # follow the conversation wherever it goes
lf operate                 # review and operate this repository’s work
lf repo-operate             # canonical name for the same skill
lf --wave designer wave-operate  # operate one Wave
```

OpenCode keeps errors and retries in its native terminal UI. Loopflow prints a
diagnostic log path before launch; full stderr stays there without covering your input.

Give an external agent harness the Loopflow operating skill:

```bash
npx skills add loopflowstudio/loopflow --skill loopflow -g -y
```

The harness acts as a User over the same `lf` API. See
[The Agent API](docs/agent-api.md).

## Find a command or workflow

```bash
lf --repo loopflow list    # use ~/src/loopflow without changing directories
```

Set `repo_root: ~/projects` in `~/.lf/config.yaml` to change the search root.
Use `--repo ./path` for an explicit checkout; with `--machine`, lookup happens there.

```bash
lf list                    # commands, skills, and flows
lf help debug              # inspect a definition without launching it
lf flow pursue             # select a flow explicitly
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

## Use only what you need

```bash
lf : "fix the failing test"  # launcher; no Task or Linear required
lf flow pursue               # run a Flow in this checkout
lf help skill work          # read the Task/Wave API contract
lf --task INF-123 design     # continue an existing Task with work guidance
```

Every launch receives a small `LOOPFLOW.md` contract. Work-associated launches
also preload the `work` skill's guidance, including when continuing a Task
already associated with the checkout or resumed Session. Load it explicitly
with `lf skill work`, or read `lf help skill work` in an existing conversation.

Desktop and custom clients share the same work APIs. Choose operator skills
when you want Loopflow to keep managed work moving. Standalone native skills
retain their own guidance.

## Keep work moving

Author a Wave in the repo and open its conversation:

```markdown
<!-- .lf/wave/designer/GOAL.md -->
## Objective

Keep the design system coherent. Each wake: read the chapter plan and
Tasks, pursue the highest-leverage open KR, start a concrete
Task only after it has a Linear issue, and fold what changed into memory.
```

```bash
lf --wave designer wave-operate        # one finite pass: keep started Tasks moving
lf --wave designer wave-operate "ship the button audit first"
lf connect --wave designer             # the Wave's ongoing conversation, applying that pass on every return
```

Edit `.lf/wave/designer/MEMORY.md` directly when durable context changes; it is a
reviewed repository file, not live server state.

Delegate durable work — the same verbs whether the caller is you or the wave:

```bash
lf checkout INF-123                               # durable Task Work + worktree, no controller
lf -b task run INF-123                            # place the Task, then run its Flow here until it ends
lf task run INF-123 pursue                        # on a workflow: take the edge that runs pursue
lf comment INF-123 "take the smaller approach"   # save a steer for the Task's Sessions
lf interrupt INF-123                             # end the active turn
lf --task INF-123 research "write scratch/runtime.md"    # one independent Task conversation
lf task run INF-123 --reason "reconcile all scratch first" # save a steer, then run fresh work
lf task status INF-123 --json                         # inspect planning, even when sync is unavailable
lf task complete INF-123                             # complete without moving the Workflow
lf task reopen INF-123                               # reopen planning; keep the Workflow and PR
lf task-operate "INF-123"                           # advance to completion, or a blocker
lf --task INF-123 skill task-session               # ongoing conversation that keeps applying task-operate
lf arm                                             # request exact-head auto-merge and return
lf land                                            # request merge; Task completion stays separate
lf pr reconcile                                      # check recorded deliveries once and settle merges
lf ci watch                                          # watch PR checks; start a ci-fix when a landing fails
```

Subtree delegation travels with shared planning. Started Tasks retain their recorded
Machine; new Tasks reserve an exclusive execution owner before checkout.
Connection settings and execution records remain Machine-local.

Task comments in Linear reach the Task's next useful boundary: before a Flow step,
after an agent turn, when showing planning, and before a Linear write. Team checks
share ten-second freshness. Only Desktop and `--watch` keep a slow clock, backing
off while nothing changes; idle agents do not poll. Teams without live Tasks make
no acquisition requests. A rate limit pauses requests across repositories in the
same Home until reset, retaining pending writes. Acquired Task comments become steers for each associated
Session, with independent delivery at its next supported provider boundary.
Steering leaves idle Sessions idle. A stopped Flow is history: `task run` never
continues it, so inspect `lf task status` first.

Turn a reviewed design into work without another planning subsystem:

```bash
lf design                                             # author and review one design
lf launch-plan                                        # keep the core here; launch independent Tasks
```

Watch this repository and the current Machine:

```bash
lf wave list                  # every durable Wave and its Machine/runtime evidence
lf user           # read config: display name from Git or a personal override
lf wave show             # every open Task across this repository's Waves
lf wave show --all       # every repository on this machine
lf wave status designer     # one Wave's current chapter and Tasks
lf history             # durable work, delivery and steering history
lf session list --json # conversations on this Machine
lf context             # context budgets, configuration sources and current usage
lf task run LOO-427 --explain --json # intended action and impediments; no launch
lf task edit LOO-427 --title "Retain drafts" --explain # validate without saving
lf --machine mini --repo project debug --context # read remote input without launching
lf flow pursue --context # graph and initial input; future inputs stay unavailable
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
lf connect                        # this scope's ongoing conversation
lf connect SESSION                # Loopflow, Claude, or Codex ID
lf session attach SESSION         # existing execution; Ctrl+] detaches
lf -b connect SESSION --message "Continue"    # headless turn in the saved Session workspace
lf session list --interactive false --history --json
lf session history SESSION --json
lf session rename SESSION "Release notes"
lf session bind SESSION --task INF-123
```

Sessions, Flows and Task shells survive closing their terminal. Reattach to a
Session with `lf session attach SESSION`;
`--observe` leaves input control unchanged. `lf resume` opens the retained native
terminal, or explicitly transitions a structured Run to native UI without replaying
its input. [Transport details and restoration limits](rust/loopflow/src/transport/README.md).

Codex terminal Sessions receive context and the first message through the provider
protocol. Claude loads context from a system instructions file and the first
message from stdin. Large requests stay out of terminal command-line arguments.
Startup, resume and compaction hooks refresh reference context from the Session's
saved launch sources. Hook declarations stay identical across Tasks and Sessions
for the same lf executable and Machine. Each native launch gets its own context
file through `LF_CONTEXT_DELIVERY`; Codex app-server threads use separate native
thread bindings because hooks inherit the server's environment.

Codex owns hook approval. Review new or changed declarations with `/hooks`;
Loopflow supplies no trust hashes or trust bypass. Approval is scoped to the
provider's config home, so another account/profile or a changed executable path
may need review again. Codex hashes declarations, not executable contents.
Headless runs skip untrusted hooks until reviewed in that provider profile.
Claude interactive hooks also require workspace trust, potentially once per
folder; `claude -p` treats the workspace as trusted. Loopflow follows the providers’ [Codex hook trust](https://learn.chatgpt.com/docs/hooks#review-and-trust-hooks)
and [Claude workspace trust](https://code.claude.com/docs/en/hooks#workspace-trust)
policies. Unchanged declarations cannot remove Claude’s folder trust requirement.

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
hooks with Codex's `/hooks`. Naming uses bounded store reads and process recording;
if ownership cannot be read promptly, the hook leaves the name unchanged.
Claude naming also works with `claude -p`. Codex naming requires its shared
app-server; embedded launches and `codex exec` lack that naming endpoint.
No resume hooks repair old conversations. Native providers own their terminal output.

A Session keeps the conversation's identity, name, feedback and native history
across commands. Interactive and headless work use the same model. Default lists
show interactive conversations; explicit filters reveal headless or completed
ones, and `--waiting` keeps those waiting on you. `--all` means all repositories.

`lf connect` opens this scope's ongoing conversation, creating one when absent.
From the canonical checkout or the repository's persistent checkout it opens the
repository primary; Task and persistent Wave checkouts open their own primary.
`lf resume` is an exact alias, including all flags. An unassociated linked checkout
requires an explicit target; ordinary `lf` starts a new conversation there.

`lf --repo NAME connect`, `lf connect --wave NAME`, and `lf connect --task ISSUE`
select an explicit scope. `lf connect SESSION` opens the exact retained conversation.
`--fresh` creates a new primary without stopping the previous client or losing its
history. Ordinary connect recovers the current primary after a lost reply; each
explicit `--fresh` creates another. `--prepare` returns connection data without
launching or taking over a client; `--json` changes only the output format.
First creation uses the matching repository, Wave or Task session skill;
reconnection retains its instructions and history.

Headless continuation reads the saved workspace's context and native history,
even from another directory, without re-running the original skill.

Bind assigns an unbound conversation to one Task permanently, including a done Task.

## The model

| Object | What it does | Where it lives |
| --- | --- | --- |
| **Skill** | Gives the agent reusable instructions and context | `.lf/skills/*.md` |
| **Flow** | Composes agent work and mechanical operations; a run is one lf process and the step processes it starts | `.lf/flows/*.yaml` |
| **Workflow** | Gives a Task nodes, where you take part in its conversation, joined by the Flows between them | `.lf/workflows/*.yaml` |
| **Wave** | Keeps the objective, memory, cadence, budget and instruments | `.lf/wave/<name>/` |
| **Project** | Holds one Wave's plan, Tasks, KRs and targets | Linear |
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
