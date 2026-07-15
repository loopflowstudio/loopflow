# PR2 — replaceable body lease and provider handoff

## User model

A Task or Project Session is durable intent. Its directive, status, event
history, Task worktree, Task PR chain, and supervision ownership survive the
process acting for it.

The acting process is one replaceable body generation. Its monotonically
increasing generation number is the write lease/fencing token. The selected
agent, provider process, and provider transcript handle belong to that lease;
they are not Session identity.

```text
lf task resume W2-135                  # next generation keeps agent + history
lf task resume W2-135 --model codex    # atomically hand off the next generation
lf project resume loopflow-api --model codex
```

## Shared API

Add one shared handoff request and audit payload:

```rust
struct ChildBodyHandoffRequest {
    agent: String,
    reason: String,
}

struct ChildBodyHandoff {
    from_agent: String,
    to_agent: String,
    from_provider: String,
    to_provider: String,
    reason: String,
}
```

`ops::child` owns the common resume/handoff operation. Task and Project resolve
their public noun, reconcile observed liveness, then pass the same shared
operation their typed Session. The store performs each noun's row update and
typed event insert in one `IMMEDIATE` transaction.

## Atomic transition

For an explicit resume with a different `--model`:

| Existing state | Result |
|---|---|
| Completed / Abandoned | reject; terminal Sessions never restart |
| Abandon requested | reject; recorded terminal intent dominates |
| Starting / Running with a live body | reject; never create writer two |
| Failed / Waiting / Blocked / Created | atomically select the new agent, set its provider, clear an incompatible provider transcript handle, append `BodyHandedOff`, then queue the ordinary Resume command |
| Open Task PR | allow only because this is an explicit operator resume; preserve the same active PR/worktree; supervisor restart remains barred |

Default resume does not run the handoff transaction. It queues Resume exactly
as today, so the next generation receives the existing provider transcript
handle.

The existing compare-and-swap process reservation advances the generation once.
Task and Project runners already reject a generation that is no longer current
and command claims are fenced by generation. PR2 keeps that mechanism as the
single writer lease rather than adding a second lock.

## Status

Human Task and Project status output names the current/latest body as
`generation <n>, agent <agent>, provider <provider>` and says `none` before the
first generation. JSON retains the structured Session fields and latest process
receipt.

## Proof

- CLI parsing accepts `lf task resume W2-135 --model codex` and the identical
  Project option.
- A failed Claude Task with an initial/current directive and an active PR hands
  off through the Store/child-session API: Session id, worktree, directive, and
  active PR are unchanged; the Claude transcript handle is gone; agent/provider
  become Codex; the next generation increments; its seed still contains the
  directive and active PR branch; and a typed from/to/reason event exists.
- Project handoff proves the same shared state transition.
- A live writer rejects handoff and keeps its agent/history untouched.
- A default/same-agent resume keeps provider history.

## Done when

Focused Rust tests, `cargo fmt`, and `cargo clippy -- -D warnings` pass; PR2 is
opened without completing W2-135.
