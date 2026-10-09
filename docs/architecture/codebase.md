# Codebase map

This is a current-source navigation map, including historical capture evidence.
The [contract and cutover status](../architecture-reference.md#cutover-status)
own the accepted model and the remaining conversion.

Start from a behavior, not a directory. Follow its request until it crosses a
named domain boundary; then switch to the next area's owner.

```bash
rg "CaptureHandle" rust/loopflow/src
rg "WorkRef" rust/loopflow/src
uv run python scripts/check_architecture.py
```

## Source territories

Physical line counts are rounded to the nearest hundred. They include inline
tests and comments, so use them to judge territory—not quality or production
complexity.

| Territory | Main paths | Approx. LOC | Owns |
| --- | --- | ---: | --- |
| CLI and presentation | `rust/loopflow/src/lf/`, `src/bin/` | 31,700 | command grammar, dispatch, status/read models, terminal output |
| Operational workflows | `rust/loopflow/src/ops/` | 25,800 | Task/Project operations, sessions, PR, Git, release, metrics, PM |
| Prompt and process engine | `rust/loopflow/src/engine/`, `src/harness/` | 29,300 | Skill/Flow discovery, prompt assembly, provider subprocess streams |
| Tracked Work | `work/`, `pm/` | — | Wave/Task facts, Task PR identity, planning-provider models |
| Storage and command journal | `store/`, `journal/` | 19,700 | SQLite, migrations, durable domain rows, outer command receipts |
| Provider authority | `provider_auth/`, `provider_account/` | 7,500 | login, encrypted tokens, account homes and routes |
| Shared root modules | top-level `src/*.rs` | 10,400 | Session captures, artifacts, repository identity, subscriptions |
| Released and draft SQL | `store/migrations/**/*.sql` | 4,900 | immutable schema history and current draft frontier |
| Swift app production | `swift/Loopflow/`, `swift/LoopflowMac/` | 18,200 | shared DTOs/services and macOS presentation |
| External tests | Rust, Python, and Swift test roots | 27,100 | cross-module, wire, migration, CLI, and app proofs |

The table is intentionally broad. The checked
[Architecture Reference](../architecture-reference.md) maps every top-level CLI
family, live table, process entrypoint, HTTP route, provider, and literal
subprocess edge to one concept.

## Core source owners

| Behavior | Begin at | Main object passed onward |
| --- | --- | --- |
| command parsing | [`lf/mod.rs`](../../rust/loopflow/src/lf/mod.rs) | command args and launch context |
| Skill/Flow discovery | [`engine/target.rs`](../../rust/loopflow/src/engine/target.rs) | selected Skill or Flow |
| prompt assembly | [`engine/prompt.rs`](../../rust/loopflow/src/engine/prompt.rs) | system/task prompt pair |
| provider routing | [`provider_account.rs`](../../rust/loopflow/src/provider_account.rs) | selected account route |
| provider streams | [`harness/`](../../rust/loopflow/src/harness/) | normalized conversation and usage |
| Session capture evidence | [`session_record.rs`](../../rust/loopflow/src/session_record.rs) | manifest, append events, terminal receipt |
| shared Work types | [`durable.rs`](../../rust/loopflow/src/durable.rs) and [`work/`](../../rust/loopflow/src/work/) | `WorkRef`, status, inputs, placement, Wave/Task facts |
| Project operation | [`ops/project.rs`](../../rust/loopflow/src/ops/project.rs) | finite attributed `wave/operate` conversation |
| Flow driver | [`lf/commands/flow.rs`](../../rust/loopflow/src/lf/commands/flow.rs) | one lf process holding the graph and cursor, starting each step as a child Process |
| Wave facts and authored context | [`work/wave/`](../../rust/loopflow/src/work/wave/) | identity, config, memory, repository scope |
| Wave facts | [`work/wave/`](../../rust/loopflow/src/work/wave/) | goals, metrics, memory, relocation |
| store abstraction | [`store/`](../../rust/loopflow/src/store/) | domain rows and transactions |
| installation | [`installation.rs`](../../rust/loopflow/src/installation.rs) | artifact set and switch receipt |
| Mac read surfaces | [`swift/Loopflow/`](../../swift/Loopflow/) | required-field DTOs from `lf --json` |

## Public process surfaces

```text
lf                         foreground command and Skill/Flow launches
lf-prompt                  prompt-oriented executable surface
lf __provider-session      provider hook that binds native conversation identity to an AgentSession
Loopflow.app               pure client over CLI/HTTP DTOs
```

Internal process names are implementation surfaces, not a second user API.
Flows may invoke the named internal operations that own their exact boundary.

| Public family | Owns |
| --- | --- |
| `lf <skill>`, `lf flow` | direct execution and composition |
| `lf wave`, `repo`, `task` | planning and Work coordination |
| `lf session` | durable Sessions and resolution |
| `lf wt`, `commit`, `sync`, `pr`, `ci` | worktree and delivery operations |
| `lf mon show`, `usage`, `activity` | durable execution/history projections |
| `lf ps`, `top`, `prune`, `doctor` | local OS and command-journal observation |
| `lf machine`, `lf --machine` | Machine identity, placement, command routing |
| `lf account` | provider credential and account authority |
| `lf install`, `release` | artifact selection and release workflow |

Argument-level behavior belongs in the [`lf` reference](../lf.md). Wire DTOs
have required fields unless their type is explicitly optional. Rust and Swift
round-trip the same fixtures under `tests/fixtures/dto/`.

`lf checkout` belongs to tracked Work and delivery: it starts no execution.
`lf task run ISSUE` places the Task and fills its defaults, then is
`lf --task ISSUE flow FLOW` in that checkout. `lf --task ... <skill>` goes directly through execution with
Task attribution and never moves a Flow's position.

## Dependency direction

```text
task launch -> execution
task launch -> work
task launch -> delivery
delivery    -> work
surface     -> execution, work, delivery

execution ⇏ work, task launch
work      ⇏ task launch
```

Keep these directions literal. Work types own Project/Task domain progression;
Task launch joins them by starting an ordinary Flow in the Task checkout.
Execution accepts preassembled Wave memory and opaque Work attribution; it does
not resolve either from the planning store.

## External transport

The CLI talks to planning and model providers. The independent Discord bridge
uses outbound REST requests. Remote execution reaches the target Machine through
`lf --machine`; see [Machines and processes](machines.md).

## Add a provider

Follow the same composition order as a provider launch:

1. Add the provider kind and credential behavior under `provider_auth/`.
2. Add account routing and health semantics under `provider_account/`.
3. Add one harness adapter that maps native output to the common stream.
4. Preserve native session ids, cumulative usage, omissions, and finality.
5. Add conformance fixtures for normal, tool, error, and retry output.
6. Map the new edge in the checked architecture reference.

Do not add a collector daemon, mandatory telemetry store, or synthetic final
usage receipt.

## Add a planning fact

1. Name the real owner and stable key.
2. Put the type in the owning domain, using shared `WorkRef` only when the fact
   truly applies to Wave, Project, and Task.
3. Add one store operation with the narrow transaction the transition needs.
4. Rebuild prompts or views from the fact at a boundary.
5. Keep provider observation and authored state distinguishable.

Do not introduce a global revision, generic active-attempt slot, or mirrored lifecycle to
coordinate facts that already have natural keys.

## Add a read surface

1. Start from existing authority or evidence.
2. Derive one DTO in Rust.
3. Make local/remote scope explicit.
4. Keep absent, unknown, stale, and unavailable distinguishable when they lead
   to different decisions.
5. Add the required-field fixture and Swift mirror if the app consumes it.

A cache may improve bounded reads. It must not become mutation or launch
authority.

## Add process control

Process control begins at spawn, not at a later lookup. Create a fresh process
scope and publish birth-validated ownership before exposing stop or steer. The
receipt must include PID plus kernel birth identity, Machine/boot identity, and
the exact group, session, or native scope. Revalidate every later signal.

Never infer ownership from a conversation ID, Work, PID alone, tmux name, telemetry, or
parentage.

## Keep the map honest

```bash
uv run python scripts/check_architecture.py
```

The checker materializes the live schema and discovers public CLI families,
process entrypoints, Machine and Wave HTTP routes, provider kinds, subprocess
edges, read projections, compatibility seams, and retired vocabulary. Every
discovered item must occur once in the checked reference.

The reference is an audit surface. These area guides remain the reading path.
When a change needs a long historical explanation to make the current model
coherent, simplify the model or move the history to review notes.

## Next

[Architecture →](../architecture.md) returns to the developer guide.
[Architecture Reference →](../architecture-reference.md) opens the checked
inventory.
