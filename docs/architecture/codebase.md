# Codebase map

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
| Operational workflows | `rust/loopflow/src/ops/` | 25,800 | Task/Project operations, Ask, PR, Git, release, metrics, PM |
| Prompt and process engine | `rust/loopflow/src/engine/`, `src/harness/` | 29,300 | Skill/Flow discovery, prompt assembly, provider subprocess streams |
| Planning runtime | `wave/`, `flowloop/`, `project/`, `task/`, `pm/`, `chat/` | 32,500 | listeners, Work loops, planning/provider models |
| Storage and command journal | `store/`, `journal/` | 19,700 | SQLite, migrations, durable domain rows, outer command receipts |
| Provider authority | `provider_auth/`, `provider_account/` | 7,500 | login, encrypted tokens, account homes, routes, leases |
| Home daemon | `lfd/` | 2,900 | Home HTTP API, webhooks, Wave and service reconciliation |
| Shared root modules | top-level `src/*.rs` | 10,400 | Run records, artifacts, repository identity, subscriptions |
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
| Skill/Flow discovery | [`lf/discovery.rs`](../../rust/loopflow/src/lf/discovery.rs) | selected Skill or Flow |
| prompt assembly | [`engine/prompt.rs`](../../rust/loopflow/src/engine/prompt.rs) | system/task prompt pair |
| provider routing | [`provider_account.rs`](../../rust/loopflow/src/provider_account.rs) | selected account route and lease |
| provider streams | [`harness/`](../../rust/loopflow/src/harness/) | normalized conversation and usage |
| Run evidence | [`run_record.rs`](../../rust/loopflow/src/run_record.rs) | manifest, append events, terminal receipt |
| shared Work types | [`durable.rs`](../../rust/loopflow/src/durable.rs) | `WorkRef`, status, inputs, placement, playhead |
| Project loop | [`project/runner.rs`](../../rust/loopflow/src/project/runner.rs) | refreshed Project plan and transition |
| Task loop | [`task/runner.rs`](../../rust/loopflow/src/task/runner.rs) | Flow boundary and delivery state |
| store abstraction | [`store/`](../../rust/loopflow/src/store/) | domain rows and transactions |
| Home daemon | [`lfd/mod.rs`](../../rust/loopflow/src/lfd/mod.rs) | Home HTTP and service reconciliation |
| machine install | [`machine_install.rs`](../../rust/loopflow/src/machine_install.rs) | artifact set and switch receipt |
| Mac read surfaces | [`swift/Loopflow/`](../../swift/Loopflow/) | required-field DTOs from `lf --json` |

## Public process surfaces

```text
lf                         foreground command and Skill/Flow launches
lf-prompt                  prompt-oriented executable surface
lfd                        one Home's service keeper and webhook receiver
lf __resident              Wave resident process
lf __work                  Project/Task controller process
lf __flow-step             one internal Flow boundary
lf __screenshot-supervisor bounded browser-capture owner
Loopflow.app               pure client over CLI/HTTP DTOs
```

Internal process names are implementation surfaces, not a second user API.
Flows may invoke the named internal operations that own their exact boundary.

| Public family | Owns |
| --- | --- |
| `lf <skill>`, `lf flow` | direct execution and composition |
| `lf wave`, `project`, `task`, `work`, `ask` | planning and communication |
| `lf wt`, `commit`, `rebase`, `pr`, `ci` | worktree and delivery operations |
| `lf runs`, `usage`, `activity` | durable execution/history projections |
| `lf ps`, `top`, `prune`, `doctor` | local OS and command-journal observation |
| `lf home`, `start`, `stop`, `pause`, `resume`, `ssh` | Home identity, placement, service routing |
| `lf auth`, `profile`, `route` | provider credential and account authority |
| `lf install`, `release` | artifact selection and release workflow |

Argument-level behavior belongs in the [`lf` reference](../lf.md). Wire DTOs
have required fields unless their type is explicitly optional. Rust and Swift
round-trip the same fixtures under `tests/fixtures/dto/`.

## HTTP boundaries

The Home daemon exposes Home-scoped health, status, Wave start/stop/reconcile,
webhook, and landing-claim routes. A Wave listener exposes only that Wave's
conversation, events, playhead, messages, observations, stop, and resident
attachment/context routes.

HTTP is a local supervision and presentation transport. It does not centralize
Run records, provider credentials, or cross-Home process control. Remote access
reaches the target Home explicitly; see [Homes and processes](homes.md).

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

Do not introduce a global revision, active-Run slot, or mirrored lifecycle to
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
receipt must include PID plus kernel birth identity, Home/boot identity, and
the exact group, session, or native scope. Revalidate every later signal.

Never infer ownership from a Run id, Work, PID alone, tmux name, telemetry, or
parentage.

## Keep the map honest

```bash
uv run python scripts/check_architecture.py
```

The checker materializes the live schema and discovers public CLI families,
process entrypoints, Home and Wave HTTP routes, provider kinds, subprocess
edges, read projections, compatibility seams, and retired vocabulary. Every
discovered item must occur once in the checked reference.

The reference is an audit surface. These area guides remain the reading path.
When a change needs a long historical explanation to make the current model
coherent, simplify the model or move the history to review notes.

## Next

[Architecture →](../architecture.md) returns to the developer guide.
[Architecture Reference →](../architecture-reference.md) opens the checked
inventory.

