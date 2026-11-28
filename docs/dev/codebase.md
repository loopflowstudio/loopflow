# Codebase map

This is a current-source navigation map, including historical capture evidence.
The [contract and cutover status](reference-cutover.md#cutover-status)
own the accepted model and the remaining conversion.

Start from a behavior, not a directory. Follow its request until it crosses a
named domain boundary; then switch to the next area's owner.

```bash
rg "LfProcessRuntime" rust/loopflow-execution/src
rg "WorkRef" rust/loopflow-execution/src
uv run python scripts/check_architecture.py
```

## Source territories

| Territory | Main paths | Owns |
| --- | --- | --- |
| CLI and presentation | `rust/loopflow/src/{lf,bin}/` | Command grammar, dispatch and text/JSON output |
| Shared operations | `rust/loopflow-execution/src/{engine,ops}/` | Typed Work operations, execution launches, delivery and planning |
| Client access | `rust/loopflow/src/server/`, `rust/loopflow-execution/src/machine/` | Server admission and dispatch; shared Machine clients and wire types |
| Execution | `rust/loopflow-execution/src/{agent,graph,prompt,skills,harness}/` | Skill/Graph discovery, prompts, provider processes and recording |
| Harness formats | `rust/loopflow-harness/src/` | What each native client writes: stream decoders, transcript readers, file locations |
| Tracked Work | `rust/loopflow-execution/src/work/`, `rust/loopflow-storage/src/work/` | Wave, Project and Task facts, delivery identity and tracker models |
| Persistence | `rust/loopflow-storage/src/`, `rust/loopflow-execution/src/{store,journal}/` | Synchronous records and SQL; tracker fetches and command/conversation history |
| Authentication and history | `rust/loopflow-execution/src/{provider_auth,transcript}/` | Service authentication, native homes, transcript adoption and a Session's conversation |
| Installation | `rust/loopflow-execution/src/installation{.rs,/}` | Selected artifacts, server registration, update and rollback |
| Native app | `swift/Loopflow/`, `swift/LoopflowMac/` | Shared DTOs/services and macOS presentation |
| Tests | Rust, Python and Swift test roots | Cross-module, wire, migration, CLI and app checks |

The table is intentionally broad. The checked
[Architecture Reference](reference.md) maps every top-level CLI
family, live table, process entrypoint, HTTP route, provider, and literal
subprocess edge to one concept.

## Core source owners

| Behavior | Begin at | Main object passed onward |
| --- | --- | --- |
| command parsing | [`lf/mod.rs`](../../rust/loopflow/src/lf/mod.rs) | command args and launch context |
| Skill/Graph discovery | [`definition/mod.rs`](../../rust/loopflow-execution/src/definition/mod.rs) | selected Skill or Graph |
| prompt assembly | [`prompt/mod.rs`](../../rust/loopflow-execution/src/prompt/mod.rs) | system/task prompt pair |
| model/harness fallback | [`agent/`](../../rust/loopflow-execution/src/agent) | configured headless continuation order |
| invocation resources | [`process/runtime.rs`](../../rust/loopflow-execution/src/process/runtime.rs) | current attachment, retry state and cleanup under the existing `LfProcess` |
| provider launch and recording | [`harness/`](../../rust/loopflow-execution/src/harness) | `Launch`, recorded conversation and usage |
| provider output | [`loopflow-harness`](../../rust/loopflow-harness/README.md) | decoded facts and transcript `Turn`s |
| Session capture evidence | [`session_record.rs`](../../rust/loopflow-execution/src/session_record.rs) | manifest, append events, terminal receipt |
| shared Work types | [`durable.rs`](../../rust/loopflow-execution/src/durable.rs) and [`work/`](../../rust/loopflow-execution/src/work) | `WorkRef`, status, inputs, Task ownership, Wave/Task facts |
| Wave operation | [`ops/project.rs`](../../rust/loopflow-execution/src/ops/project.rs) | finite attributed `wave-operate` conversation |
| Graph process | [`lf/commands/graph.rs`](../../rust/loopflow/src/lf/commands/graph.rs) | one lf process holding the graph and cursor, starting each step as a child Process |
| Wave facts and authored context | [`work/wave/`](../../rust/loopflow-execution/src/work/wave) | identity, config, memory, repository scope |
| store abstraction | [`store/`](../../rust/loopflow-execution/src/store) | domain rows and transactions |
| installation | [`installation.rs`](../../rust/loopflow-execution/src/installation.rs) | artifact set and switch receipt |
| Client server | [`server/`](../../rust/loopflow/src/server) | Typed requests and responses shared with CLI operations |
| Creation recovery | [`engine/creation.rs`](../../rust/loopflow-execution/src/engine/creation.rs) | Exact submitted request and server-owned receipt |
| Mac Work surfaces | [`swift/Loopflow/`](../../swift/Loopflow) | Required-field DTOs over the shared server protocol |

## Public process surfaces

```text
lf                         foreground command and Skill/Graph launches
lf-prompt                  prompt-oriented executable surface
lf __context-block         provider turn-boundary hook: renders context at start/compaction and asks the Machine server for turn recording and steers (`session.boundary`); opens no store
Loopflow.app               client over shared server/CLI DTOs
```

Internal process names are implementation surfaces, not a second user API.
Graphs may invoke the named internal operations that own their exact boundary.

| Public family | Owns |
| --- | --- |
| `lf <skill>`, `lf graph` | direct execution and composition |
| `lf wave`, `repo`, `task` | planning and Work coordination |
| `lf session` | durable Sessions and resolution |
| `lf wt`, `commit`, `sync`, `pr`, `repo ci` | worktree and delivery operations |
| `lf session history`, `session history show`, `session usage` | durable execution/history projections |
| `lf machine doctor` | local OS observation: strays, live lf binaries, store contention |
| `lf machine`, `lf --machine` | Machine identity and execution-destination routing |
| `lf server` | Client API serving and observational readiness |
| `lf machine login\|logins\|logout` | each provider's own sign-in on any Machine, approved here |
| `lf install`, `release` | Artifact selection, server registration/update and release workflow |

Argument-level behavior belongs in the [`lf` reference](../lf-reference.md). Wire DTOs
have required fields unless their type is explicitly optional. Rust and Swift
round-trip the same fixtures under `tests/fixtures/dto/`.

`lf checkout` belongs to tracked Work and delivery: it starts no execution.
`lf task run ISSUE` places the Task and fills its defaults, then is
`lf --task ISSUE graph GRAPH` in that checkout. `lf --task ... <skill>` goes directly through execution with
Task attribution and never moves a Graph's position.

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

Keep these directions literal. Work types own Wave/Task domain progression;
Task launch joins them by starting an ordinary Graph in the Task checkout.
Execution accepts preassembled Wave memory and opaque Work attribution; it does
not resolve either from the planning store.

## External transport

The CLI talks to trackers and model providers. Remote execution reaches the target Machine through
`lf --machine`; see [Machines and processes](machines.md).

## Add a provider

A provider is one unit struct with two implementations: what it writes, then
how lf launches it. Launch and transcript decisions branch on what the harness
declares.

```sh
cargo test -p loopflow-harness <name>_decode                         # after steps 1-2
cargo test -p loopflow-execution --lib harness::launch_matrix        # after step 4
cargo test -p loopflow-execution --lib harness::conformance_tests::  # after step 4
cargo test -p loopflow --lib server::wake::tests::                   # after step 4
```

1. In [`loopflow-harness`](../../rust/loopflow-harness/README.md), add the unit
   struct and its `HarnessKind` variant. Include it in `HarnessKind::ALL` and
   `HarnessKind::harness()`.
2. Implement `Harness` on it, including its caller-session markers, driver
   recognition and sign-in advice. Map native output to the common stream in
   `<name>_decode.rs` and implement `transcript_reader` in
   `transcript/<name>.rs`. `transcript_home` has no default: name the client's
   own home variable and directory. Supply the provider's saved-history layout
   and retention policy when it keeps transcripts. Keep native session ids, usage
   omissions and finality. Captured output for normal, tool, error and retry
   turns goes in `src/testdata/` and `src/transcript/testdata/`.
3. In execution, add `harness/<name>.rs` implementing `Launcher` on the same
   struct: its `Capabilities`, `open` for the native
   interactive client and `driver` for the headless stream, and `recognizes`
   when a running process is its client by more than the command name. Add its
   line in `harness::launcher`. Implement `idle_reach` for the endpoint that
   starts a turn, and declare how a reopened work session receives its steers.
4. Exercise the provider through the shared launch, worker and wake fixtures.
   `harness::launch_matrix`, `harness::conformance_tests`, `server::wake::tests`,
   and `task_graph_launch_tests` iterate `HarnessKind::ALL`. The Antigravity
   reopen fixture exercises the native launch through `lf resume --task`.
5. Map the new edge in the checked architecture reference.

Do not add a collector daemon, mandatory telemetry store, or synthetic final
usage receipt.

## Add a planning fact

1. Name the real owner and stable key.
2. Put the type in the owning domain, using shared `WorkRef` only when the fact
   truly applies to Wave and Task.
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

[Architecture →](index.md) returns to the developer guide.
[Architecture Reference →](reference.md) opens the checked
inventory.
