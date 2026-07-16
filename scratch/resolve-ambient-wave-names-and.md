# Resolve ambient Wave names and UUIDs uniformly in Task and Project controls

W2-238. Source: W2-151 post-merge done-when audit. Infrastructure-only.

## Problem

W2-151 shipped one shared ambient-Wave resolver (`engine::wave_context::resolve_managed_wave_name`)
and routed the CLI read/publish surfaces through it — chat, radio, memory, home, cron, `lf status`,
`lf pm show` (#979). Two control-plane entry points were missed:

- `ops::task::command_source` (task.rs:339) → `command_source_for_wave` (task.rs:373)
- `ops::project::project_command_source` (project.rs:721)

Both read `LF_WAVE_ID` and call `WaveId::parse` — UUID-only — then equality-compare against the
owning session's `wave_id`. Consequences:

- **Hand-set name breaks.** `LF_WAVE_ID=product` fails `WaveId::parse` and errors "invalid ambient
  Wave id." Every other surface honors a hand-set name; `lf task steer`/`lf project steer` reject it.
- **Stale UUID is misclassified.** A UUID the registry has no row for still parses as a valid UUID,
  fails the equality check, and reports "Wave X cannot control Task Y owned by Wave Z" — identical to
  a genuine foreign-wave violation. The user can't tell stale-context from wrong-wave, so the
  remediation is unclear (listener direction: "no clear explanation of why something is blocked").
- **Task and Project disagree with the product.** `lf status`/`lf pm show`/`lf memory` resolve a
  hand-set name; the Task/Project controls from the same env reject it.

Who benefits: anyone driving Task/Project sessions from a managed environment that carries the wave
by name (Mac workers, hand-set dev shells), and anyone triaging a stale `LF_WAVE_ID` after a wave is
re-registered. Why now: this is the last hole in the W2-151 uniform-resolution contract.

## The demo

Infrastructure-only — the demo is classified CLI behavior, not a UI. From a shell carrying the wave
by name (`LF_WAVE_ID=product`, not its UUID):

```
$ lf task steer INF-123 "drop the cache first"
accepted  live-steer  directive v3

$ lf project steer developer-efficiency "pick the rebase task next"
accepted  live-steer  directive v2
```

And from a stale env (`LF_WAVE_ID=<uuid of a wave no longer on this machine>`):

```
$ lf task steer INF-123 "..."
error: ambient wave '<uuid>' (LF_WAVE_ID) is not in this machine's registry;
       the context is stale — pass --wave <name>
```

Same classified result whether the control is `lf task …` or `lf project …`.

## Approach

Route both `command_source` and `project_command_source` through
`resolve_managed_wave_name`, then authorize the resolved wave against the owning session.
One resolver, one identity model, no `WaveId::parse` of the ambient env.

Make both functions `async` and take the store they already have in scope at every call site
(verified — see De-risking). Replace the UUID-equality helper `command_source_for_wave` with one
shared authorization helper that classifies the resolver's result against the owning wave. Both
controls call literally the same helper, so they cannot drift.

### The one classification rule (both controls)

Given the owning session's `wave_id` and the ambient env:

1. Task only: the existing `LF_PROJECT_SESSION_ID` arm runs **first** and returns
   `ChildCommandSource::Project(project_id)` — unchanged. It is the explicit-override path at this
   layer; Project has no analog (Projects aren't supervised by a Project Session).
2. Resolve the ambient wave name: read `LF_WAVE_ID` from env, call
   `resolve_managed_wave_name(Some(store), None, env_wave_id)` (mirroring `chat::resolve_target` at
   chat.rs:409, which passes `Some(&**store)` for `&SharedStore`).
3. Map the resolver outcome:
   - `Ok(name)` → `store.get_wave_by_name(&name)`:
     - `Some(row)` where `row.id() == owning.id()` → `ChildCommandSource::Wave(owning.id().clone())`
     - `Some(row)` (different id) → error: `Wave {name} cannot control {subject} owned by Wave {owning.name}`
     - `None` → error: `ambient wave '{name}' is not registered on this machine; the context is stale — re-register the wave or fix LF_WAVE_ID`
   - `Err(NoContext)` → `ChildCommandSource::Human`
   - `Err(StaleIdentity(uuid))` → error (the resolver's message, naming the stale id)
   - `Err(Registry(e))` → error (propagate)
   - `Err(UnknownExplicit(_))` → unreachable at this layer (no explicit arg is passed); document and
     treat as a loud error if it ever occurs.

`{subject}` = `Task {issue.identifier}` or `Project {project.slug}`. The owning wave (with its name)
comes from the existing `owning_wave(store, session)` helper (task.rs:331, project.rs:127).

### Classification matrix

| Ambient context | Resolver result | command_source result |
|---|---|---|
| registered name (== owning) | `Ok(owning.name)` → row match | `Wave` |
| registered UUID (== owning) | `Ok(owning.name)` → row match | `Wave` |
| explicit override (task: `LF_PROJECT_SESSION_ID`) | pre-empts step 2 | `Project` |
| stale UUID | `Err(StaleIdentity)` | loud error naming the id |
| stale name (not registered) | `Ok(name)` → no row | loud "not registered" error |
| absent | `Err(NoContext)` | `Human` |
| foreign registered wave | `Ok(name)` → row, wrong id | "cannot control" error |

Task and Project produce identical results for every row except the explicit-override row
(task-only by construction).

### Shared helper

One private async helper so both controls call the same code (placement: `ops::util`, which already
owns `normalize_wave_name`, or a new `ops::wave_source` — prefer `ops::util` to avoid a new module
for one function):

```rust
pub(crate) async fn resolve_child_command_source(
    store: &SharedStore,
    owning_wave_id: &WaveId,
    subject: &str,
) -> OpsResult<ChildCommandSource>
```

Reads `LF_WAVE_ID` from env, calls the resolver, classifies per the matrix. `command_source` and
`project_command_source` become thin wrappers:

```rust
// task.rs
async fn command_source(store: &SharedStore, session: &TaskSession) -> OpsResult<ChildCommandSource> {
    // existing LF_PROJECT_SESSION_ID arm — unchanged, returns Project
    // ...
    resolve_child_command_source(store, &session.wave_id,
        &format!("Task {}", session.launch.issue.identifier)).await
}

// project.rs
async fn project_command_source(store: &SharedStore, session: &ProjectSession) -> OpsResult<ChildCommandSource> {
    resolve_child_command_source(store, &session.wave_id,
        &format!("Project {}", session.launch.project.slug)).await
}
```

Call sites updated to `.await` and pass `&store`:
- task.rs:690 (task_run), 2730 (queue_command), 2781 (resume_task_async)
- project.rs:292 (reserve_project_session), 751 (queue_project_command), 793 (project_resume)

## De-risking

| Question | Finding | Impact on design |
|---|---|---|
| Are all call sites in async context with a store? | Yes. task 690/2730/2781 inside `block_on_task`/`resume_task_async`; project 292/751/793 inside `block_on_project`. `store` bound at each. | Async rewrite + `&store` param feasible everywhere; no scratch-thread store. |
| Nested-runtime risk from awaiting inside `block_on_task`? | `block_on_task` builds its own `Runtime` and `block_on`s the async block; awaiting inside is the normal path. `resume_task_async` is already async. | No new runtime; await directly. |
| Is an explicit `--wave` threaded to command_source? | No. `TaskCommand`/`ProjectCommand` variants take `issue`+`message`+`--json` only (mod.rs:1024+). | "Explicit override" at this layer = the `LF_PROJECT_SESSION_ID` arm. No new CLI flag. (questions.md) |
| Does the shared resolver handle stale-name? | It returns a hand-set name without membership check ("membership is each consumer's concern", wave_resolution_tests.rs:86). | command_source must add `get_wave_by_name` to split stale-name from foreign-registered. |
| What invariant must survive? | `foreign_wave_cannot_be_reclassified_as_a_human_command` (task.rs:3688): a foreign/stale wave must NEVER become `Human`. | New rule returns `Human` only on `NoContext`; every other branch errors. Re-tested. |
| Is `command_source_for_wave` used outside command_source? | No — private; one call site (command_source:370) + its test (3693-3700). | Safe to delete with its test. |
| Does `owning_wave` already fetch the owning Wave + name? | Yes — task.rs:331, project.rs:127. | Reuse for owning name in errors; no new store pattern. |
| Will removing `WaveId::parse` of env break the wire test? | `child_session.rs:836` tests `ChildCommandSource::Wave` serialization, not env parsing. | Unaffected. |
| Does `lf status` stay blocked on `release-stability`? | Yes — unrelated stale-project drift (task 5efbfd37). | Explicitly out of scope here. |

## Alternatives considered

| Approach | Tradeoff | Why not |
|---|---|---|
| `resolve_managed_wave_name_sync` (own store on scratch thread) | Keeps command_source sync; no signature change. | Every call site already has a store in async context; a second connection is waste and re-introduces the "two identity models" smell. |
| Keep `WaveId::parse` + add a name-fallback branch alongside | Minimal diff. | Violates "no compatibility parser or second identity model remains" — it IS the second model. |
| Compare resolved name to owning name without `get_wave_by_name` | One fewer store read. | Conflates stale-name with foreign-registered into one "cannot control"; the directive names stale-name as distinct, and the listener direction demands the split. |
| Thread `--wave` through Task/Project commands to command_source | True explicit-override at the wave layer. | No caller passes one; inventing an unused flag is scope creep. `LF_PROJECT_SESSION_ID` already covers override. (follow-up) |

## Key decisions

- **One shared helper, two thin wrappers.** `resolve_child_command_source` holds the whole
  classification; task and project call it so they can't drift — the structural guarantee of "same
  classified result across Task and Project."
- **`Human` only on `NoContext`.** Every other outcome is a loud error. Preserves the foreign-wave
  invariant and serves the listener direction: the blocked reason names stale vs foreign vs absent.
- **Stale-name vs foreign-wave split via `get_wave_by_name`.** Different remediation
  (re-register vs switch wave), so different error text.
- **Delete `command_source_for_wave` and its test.** No second identity model. Replace with a
  matrix test over the shared helper.
- **Async + `&store`**, not the sync scratch-thread variant — the store is already in scope.

## Scope

- In scope:
  - `ops::task::command_source` → async; delegate to shared helper; preserve `LF_PROJECT_SESSION_ID` arm.
  - `ops::project::project_command_source` → async; delegate to shared helper.
  - New shared `resolve_child_command_source` (+ `WaveResolveError` → `OpsError` mapping).
  - Delete `command_source_for_wave` and `foreign_wave_cannot_be_reclassified_as_a_human_command`.
  - Update the three task + three project call sites to `.await` and pass `&store`.
  - Focused integration tests: a matrix over both controls with real store rows, no network.
- Out of scope:
  - Threading `--wave` into Task/Project control commands (no caller; flagged as follow-up).
  - Changing `ChildCommandSource` variants or the wire shape.
  - The CLI read surfaces (already routed in W2-151/#979).
  - The `release-stability` stale-project drift blocking `lf status` (task 5efbfd37).

## Done when

- `cargo test -p loopflow` green, including a new test that drives both `task` and `project`
  command sources across the six ambient contexts (registered name, registered UUID, stale UUID,
  stale name, absent, foreign-registered) with real store rows and no network, asserting identical
  classification per the matrix.
- `rg "WaveId::parse" rust/loopflow/src/ops/task.rs rust/loopflow/src/ops/project.rs` returns
  nothing (the only two hits today are the ambient arms at task.rs:362 and project.rs:724; both
  go away. Session-id parses use `TaskSessionId::parse`/`ProjectSessionId::parse`, not `WaveId`).
- `command_source_for_wave` no longer exists.
- A hand-set `LF_WAVE_ID=<name>` and a stale `LF_WAVE_ID=<uuid>` produce the same classified result
  for `lf task steer`/`lf project steer` as for `lf status`/`lf pm show` (extend
  `wave_resolution_tests.rs` or add a paired ops test).
- `cargo clippy -- -D warnings` and `cargo fmt` clean.

## Measure

Not quantitative. The product signal is command-classification agreement: before, a hand-set name
diverged (CLI accepted, controls rejected); after, all surfaces agree. The done-when test matrix is
the proof.

## Wave alignment

Infrastructure GOAL.md: "the system is legible, local work is fast, and shipping is boring." This
closes the last identity-resolution hole so Task/Project controls agree with every read surface —
legibility. MEMORY.md "Vocabulary discipline" and the W2-151 contract: one resolver, no second
identity model. Listener steering: a blocked command must explain why (stale vs foreign vs absent),
not surface a wrong-wave error for a stale context. The change deletes a code path, so surface area
shrinks — no new risk introduced.
