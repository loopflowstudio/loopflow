# The Agent API

`lf` is the API agents call to launch, steer, and observe other agents. There
is no SDK and no server. The verbs are the same binary humans type; every read
surface takes `--json`; and every launched agent receives the operating
contract (`LOOPFLOW.md`) in its context, so it already knows these verbs when
it starts.

A wave directing a task looks like this — and the caller here is an agent, not
a person:

```bash
lf task run INF-123                                  # start a durable Task Session
lf task steer INF-123 "take the smaller approach"    # redirect its active turn
lf task receipt CMD_ID --until incorporated --json   # prove the steer landed
lf task wait INF-123 --until terminal                # block until it settles
```

## The nouns

Delegation follows one supervision path: **Wave → Project Session → Task
Session**. A Wave coordinates and remembers; a Project pursues measurable KRs;
only a Task Session owns a worktree, and every file-writing change happens
there, advancing through serial PRs to `main`. Tasks are Linear issues —
durable delegated work starts from an existing issue, so the roadmap is the
queue.

## Delegate

```bash
lf task run INF-123                          # run an existing Linear issue
lf task start "add passkeys" -p <project-id> # create the issue, then run it
lf task run INF-124 --stack-on INF-123       # dependent work before the parent PR merges
lf task run INF-125 --headless               # no interactive surface
lf project run <project-id>                  # start the supervising Project Session
```

The contract every agent runs under: **delegation must make the problem
smaller**. Delegate only a strict subset that can finish independently; never
hand off the whole seed, and never delegate the one blocker between you and
completion — resolve that inline. The current process and worktree are the
default execution surface.

`--stack-on` forks the child's worktree from the parent Task's active PR and
records the fork commit; the child's PR targets the parent's branch until
merge, then replays only child-authored commits onto `main`.

## Steer

Direction to a running Session is a protocol, not a hope. Directives are
versioned, and receipts prove incorporation:

```bash
lf task follow-up INF-123 "also audit retry callers"   # queue for the next turn
lf task steer INF-123 "support passkeys too"           # redirect the active turn
lf task interrupt INF-123 --message "stop; smaller PR" # interrupt now
lf task receipt CMD_ID --until incorporated --timeout 30s --json
lf task acknowledge INF-123 --directive 2 --summary "smaller approach active"
```

Decisions flow the same way in both directions:

```bash
lf task request-decision INF-123 --option approve --option revise --wait
lf task decide INF-123 DECISION_ID approve
```

A Session survives its process. `lf task resume INF-123 --model codex` leases
the next body generation to another provider without losing the directive,
worktree, or PR chain; `lf task recover` starts a linked successor
on the same worktree.

`lf project` carries the same verbs one level up: `follow-up`, `steer`,
`interrupt`, `receipt`, `acknowledge`, `decide`, `wait`, `resume`, `attach`.

## Speak: the radio

Agents talk to each other on the bus; chat belongs to humans.

```bash
lf radio pub --channel goals.build "parser lands green; starting docs"
lf radio sub goals --json          # NDJSON stream of a channel family
```

Publish is an INSERT into the shared local store — no broker, no server in the
path, so it works even with zero loopflow processes running. A listening Wave
folds messages from its channel family into its journal with attribution.
Outside any wave, a publish prints a drop note and exits 0, so the verb is safe
in every prompt. Never guess a channel: publish only where the prompt or skill
names one.

`lf chat` is reserved for humans; agents never post there.

## Remember, with receipts

```bash
lf memory add "workers report via stream" --receipt chat_turn:turn-3
lf receipt show chat_turn:turn-3 --json
lf receipt show pr:acme/app#42 --json
```

`lf memory add` records a durable wave learning bound to evidence. A receipt
token (`kind:reference` — `chat_turn:`, `run:`, `pr:`, …) resolves to the
canonical local record, so a claim in memory can always be drilled to what
actually happened. `wave/<name>/MEMORY.md` is server-owned; agents write
through the API, never the file.

## Ship

Three commitment levels, all headless — pick by how done the work is and who
lands it:

```bash
lf pr publish    # make work visible mid-stream; the agent's default verb
lf pr submit     # done, a human clicks merge
lf pr land       # done, loopflow lands it hands-off; -c completes the Task
```

`lf pr open` is the one presenting verb — it opens a browser. Agents reach for
it only when a human asked to see the PR.

## Observe

Every read the fleet surfaces offer is `--json`:

```bash
lf status <wave> --json     # live Project → Task hierarchy
lf roadmap --json           # every open Task across every wave
lf runs --task INF-123 --json
lf trace <exec-id> --json
```

[The Fleet →](fleet.md) covers the full monitoring surface. The Mac app is
built on exactly these calls — it keeps no second database.

## The contract

Every launched agent gets `LOOPFLOW.md` — the operating contract — in context
(opt out with `--no-loopflow`). Its spine:

- Route git, worktrees, and PRs through `lf`; never raw `git worktree`.
- Execute here first; delegation must make the problem smaller.
- Checkpoint and proceed: don't ask permission for reversible work.
- Answer humans in turn text; report proactively on the radio only when a
  channel is established.
- Write repo-specific learnings into `.lf/` and commit them with the work.

Source: `rust/loopflow/src/engine/builtins/LOOPFLOW.md`.

## Next

[The Fleet →](fleet.md) · [Waves →](waves.md) · [`lf` reference](lf.md)
