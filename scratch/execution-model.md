# Loopflow execution model

Loopflow execution has three independent axes:

1. **Lifecycle** — one direct run or repeated passes to a termination bit.
2. **Placement** — the current worktree or a fresh/stacked worktree.
3. **Ownership** — the caller waits, or a served wave owns the background work.

An OS child process is not delegation. `lf commit`, `lf pr land`, direct skills,
direct flows, tests, and bounded helper agents can all be children of the current
execution while the current task retains ownership.

## Vocabulary

| Term | Meaning |
| --- | --- |
| Execute inline | Work the current lifecycle in the current worktree. Nested operations are allowed. |
| Invoke an operation | Run `lf commit`, `lf pr land`, `lf rebase`, or another mechanical command. Inline, no delegated lifecycle. |
| Run a skill once | `lf skill <name> ...` or the bare target form. Inline and blocking. |
| Run a flow once | `lf flow <name> ...`. Runs its authored steps once, inline and blocking. |
| Place a one-shot run | `--fork` or `--stack`. New worktree and registry run; caller still waits. |
| Inhabit a loop | Repeated flow in a placed worktree; caller owns it and blocks. |
| Detach a loop | Transfer an already-justified loop to an already-served wave so the parent can do other useful work. |
| Serve a wave | Start residency, thread, cadence, and the detached-loop door. Not ordinary delegation. |
| Promote a project | Exceptional project-to-wave migration owned by the dedicated promotion skill. |

There is no `--delegate` flag. `--detach` changes ownership after the decision to
create a loop has already been made.

## Decision tree

```text
Can this be completed in the current lifecycle and worktree?
├─ yes → execute inline
│        ├─ one skill → lf skill <name> ...
│        ├─ one authored flow → lf flow <name> ...
│        └─ mechanical step → lf commit / lf pr land / lf rebase / ...
└─ no: does it need a separate worktree and repeated completion condition?
         ├─ no → keep it inline; placement bureaucracy buys nothing
         └─ yes → create a loop with room for at least two passes
                  ├─ parent needs the result next → foreground loop
                  └─ parent has another useful move now
                           ├─ exact wave already served → --detach
                           └─ no live server → foreground loop
```

Never start a wave server merely to make `--detach` available. Never use
`--max-passes 1`; run the flow directly.

Canonical loop syntax puts global scope before the subcommand:

```bash
lf --wave <wave> loop <flow> "<whole handoff>"
lf --wave <wave> loop <flow> "<whole handoff>" --detach
```

The CLI also accepts a global flag after a built-in command when that command
does not define the same spelling. It normalizes `lf loop task "…" --wave
designer` to the canonical form before parsing. A subcommand-local spelling
wins on collision (`lf commit -m "message"`; loop-local `--max-turns`), and
`--` ends normalization so later flag-shaped text stays literal.

## Delegation depth

The tier controls `lf loop`, not all child processes.

| Current tier | Inline work | May launch |
| --- | --- | --- |
| One-off command | Assigned seed, direct skills/flows, mechanical ops | No loop |
| Task | Its own task, scoped PM reads/updates, related-work inspection, follow-up filing, mechanical ops | No loop |
| Project | Sole blocking task inline, project PM/KR work | Task loop only |
| Wave | Sole blocking move inline, selection/sequencing, wave PM/worker state | Project or task loop |
| Promotion/split skill | Its explicit lifecycle operation | A new served wave when the skill requires it |

A task may read and close its own PM item, inspect related work, and file another
task for later. Filing work is not launching it.

This bounds normal recursive delegation:

```text
served wave
├─ inline operations / skills / flows
├─ task loop                         (task launches no loop)
└─ project loop
   ├─ inline project work
   └─ task loop                      (task launches no loop)
```

The maximum normal loop depth is therefore wave → project → task. A child seed
must be a strict subset, never the parent's whole objective restated.

## Wave/server matrix

| Invocation | No exact/ambient wave | Exact wave, server stopped | Exact served wave |
| --- | --- | --- | --- |
| Direct skill / flow / prompt | Runs without wave context | Runs locally with that wave's stored context | Runs locally with live wave context; server does not own it |
| Mechanical `lf` operation | Runs | Runs | Runs |
| `--fork` / `--stack` one-shot placement | Fails wave placement resolution | Runs; server not required | Runs identically |
| Foreground `lf loop` | Fails wave resolution | Runs; caller owns it | Runs; caller still owns it |
| `lf loop --detach` | Fails wave resolution | Fails: no live server | Server owns it and caller returns after launch |
| `lf serve <wave>` | Explicit name required; may register it | Starts residency | Refuses a duplicate unless forced |
| `lfq exec ...` | Fails | Fails | Runs only allowlisted verbs with an injected capability token |
| Project promotion | Parent must be exact/ambient | Works from registry and starts child residency | Parent liveness does not change it |

A stopped server blocks detachment and server-owned memory mutation. It does not
block direct execution, mechanical operations, foreground loops, or scoped PM
work. Those paths must not detour into starting a server.

## Context and attribution

An explicit top-level `--wave <wave>` is one identity choice. It controls:

- the GOAL/MEMORY/project/thread context assembled for the run;
- session and journal wave attribution;
- loop placement and the channel used by the work.

The branch implementation resolves the explicit name to a registered wave and
rejects unknown names instead of creating synthetic empty wave framing. An
explicit wave overrides a different ambient wave; cross-wave parent session
links are dropped.

Generic skills never infer a wave because a nearby directory appears related.
They read wave/PM state only when the prompt or seed gives an exact wave,
task/project reference, or concrete coordination question.

## Registry runs, traces, and spans

- `run_id` is the trace.
- `process_id` is one process span and is always freshly minted.
- Nested operational `lf` commands inherit their owning trace.
- A placed one-shot run or loop mints a new trace.
- The placed registry `Run.id` is the same value as that trace's `run_id`.
- Every pass of a loop and every nested operation in that pass inherits the
  loop's registry/trace id.
- A nested loop mints another registry/trace id; two sibling loops launched
  under one parent never collide or merge evidence.
- Detached launch carries its minted id through the server exactly once, so its
  background child reports under the same id shown by `lf status`.

```text
parent trace A
├─ inline op span A.1
├─ foreground loop registry/trace B
│  ├─ pass span B.1
│  └─ nested op span B.2
└─ detached loop registry/trace C
   ├─ loop-owner span C.1
   └─ pass span C.2
```

`lf status`, prompt logs, ledger readers, and `lf trace <run-id>` can therefore
name the same placed work.

## PM failure rule

- Wave/project: report a failed PM reader once, then continue from GOAL, MEMORY,
  project KRs, and the seed. Repair PM only when PM is the assigned work.
- Task: read/update its own item and related state when useful. If unavailable,
  continue from a computable seed; do not turn the task into auth repair.
- One-off: do not browse PM unless the seed or active skill makes it relevant.

## Durable output

A loop does not create a Loopflow transcript artifact. Its vendor conversation
is private. Durable evidence is:

- the PR and branch changes;
- `lf radio` progress/completion/failure reports;
- `lf memory add` durable learnings when a live wave exists;
- registry and `run_events` evidence.

A read-only tmux attach primarily exposes outer loop status; it is not a durable
record of successful pass output.

## Remaining CLI gaps

- `--fork` and `--stack` parse on commands that do not honor them (`loop`,
  `serve`, inline prompt, and ordinary ops). Unsupported combinations should
  reject instead of silently doing nothing.
- Placement resolution still has several entrypoints (top-level wave scope,
  worktree/branch inference, and subcommand-local PM/chat flags). The normal
  model is exact explicit scope first; further consolidation is separate work.
- The installed binary must be refreshed before local behavior reflects source
  changes in this branch.
