# `lf q` — absorb the queue, delete lfq

**Finish line:** `lf q <verb>` does everything the `lfq` queue/worker surface
does (dispatch, `worker run`) against `lfdb`, and the `lfq` binary is deleted.
One workhorse, no separate queue binary.

## Context

The `lfq` binary is already gone from `rust/loopflow/Cargo.toml` (targets are
`lf`, `lfd`, `lf-prompt`), but the queue/worker *behavior* still needs a settled
home under `lf q`. The goals wave's runtime notes still describe an `lfq wave
run` / `lfq worker run` surface — reconcile that vocabulary here: the queue verbs
live under `lf q`, reading and writing the same `lfdb` tables as `lf d`.

## Scope

- Move queue/worker APIs (dispatch, `worker run`) under `lf q`.
- Confirm no `lfq` bin target or invocation path survives anywhere (Cargo,
  scripts, Concerto, docs).
- Queue state reads/writes go through `lfdb`, not a second reach-in.

## Done when

- `lf q` dispatches work and runs workers.
- No `lfq` binary, target, or documented invocation remains.
- Queue state shares the `lfdb` backend with `lf d`.

## Depends on

[[1-lfdb-extraction]].
