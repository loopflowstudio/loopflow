---
head: 615729570782d730d2ea3b196e34779db9f63555
status: bootstrap
---

# Continuity Map

## Purpose

Find chains of work that are only partially complete. Reduce cares because
partial integration creates compensating complexity: docs say one thing, UI
shows another, backend supports a third, and future agents add glue instead of
removing the mismatch.

## Feature chain model

For each product change, track the expected chain:

```text
design intent
  -> backend/runtime behavior
  -> CLI/API surface
  -> UI surface
  -> docs/examples
  -> tests/verification
  -> release note or decision record
```

Not every change touches every link. The reduce question is whether omitted
links are intentional or just forgotten.

## Standing drift patterns

### Backend exists, UI missing

Signals:

- Rust lfd or engine capability exists.
- `lfq` or Swift model has no corresponding field/action.
- Docs explain command-line use but Concerto cannot drive or display it.

Likely places to inspect:

- `rust/loopflow/src/lfd/http/dto.rs`
- `python/loopflow/models.py`
- `swift/LoopflowCore/Models/`
- `swift/LoopflowCore/State/`
- `swift/Concerto/Views/`

### UI changed, docs stale

Signals:

- Swift views expose concepts absent from `README.md` or `docs/`.
- Screenshots or UI tests show flows not documented in the journey docs.
- Release notes mention a product behavior without a stable docs page.

Likely places to inspect:

- `swift/Concerto/Views/`
- `docs/index.md`
- `docs/getting-started.md`
- `docs/waves.md`
- `README.md`

### Docs lead implementation

Signals:

- README examples mention commands or flags that do not appear in `lf --help`.
- Wave-authoring docs describe fields not accepted by lfd config parsing.
- Prompt/flow docs mention built-ins that are no longer resolvable.

Likely places to inspect:

- `rust/loopflow/src/lf/commands/`
- `rust/loopflow/src/engine/flow.rs`
- `rust/loopflow/src/lfd/types/wave.rs`
- `.lf/steps/`
- `.lf/flows/`

### DTO mirror drift

Signals:

- A Rust wire field has no Python or Swift peer.
- A fixture update tests only one language.
- A client supplies defaults for fields that should be required.

Likely places to inspect:

- `tests/fixtures/dto/`
- `tests/parity/`
- `python/tests/`
- `swift/ConcertoTests/DTOFixtureTests.swift`

### Wave/runtime drift

Signals:

- Wave docs use terms not represented in runtime state.
- lfd schedules a concept that wave authoring cannot express cleanly.
- PM sync changes lifecycle state but roadmap docs do not preserve why.

Likely places to inspect:

- `wave/`
- `docs/wave-authoring.md`
- `rust/loopflow/src/lfd/triggers/`
- `rust/loopflow/src/lfd/scheduler.rs`
- `rust/loopflow/src/ops/pm.rs`

## First continuity audit

Start with session lifecycle because it crosses every surface:

- Rust daemon session/run/event types.
- Python lfq models and commands.
- Swift session/run/wave/attention stores.
- README/docs examples for `lfq sessions`, `lfq attach`, `lfq logs`.
- Tests that prove live update behavior.

Deliverable: a table of lifecycle states, source files, UI labels, docs labels,
and mismatches. Only then propose renames or deletions.
