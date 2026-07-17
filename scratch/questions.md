# Open questions — W2-308

Nothing blocking. Assumptions made and proceeding:

1. **A failed `harness.interrupt()` fails the body.** Chose `?` over
   `let _ = ...`. It matches the control path (`runner.rs:3876-3889`), and
   swallowing it would leave `review_preempted` unset and re-interrupt every
   200ms against a harness that cannot be interrupted. Cost: a broken interrupt
   now kills a body holding an open review. The review record survives (durable,
   `Active`), so a successor generation resumes it — the same recovery every
   other body failure uses.

2. **One interrupt per provider turn, not per body.** Clearing `review_preempted`
   at `TurnCompleted` is one line and strictly better than never clearing: a
   review that resumes and later meets a *new* failing head can be preempted
   again. Argued in the design that this cannot loop, since after `TurnCompleted`
   either the wake armed or it is stale — both close the guard.

3. **The stale-wake fallback is the pre-existing idle state.** If the wake goes
   stale between the currency read and `TurnCompleted`, nothing arms and the body
   idles at `runner.rs:729` with the review open. Not adding compensating logic
   for a window the currency check already makes rare; the fallback is a state the
   runner already reaches today.

4. **Not touching the fresh-Project-review path.** It sets
   `provider_turn_active = false` and already arms through the idle poll. The
   directive's framing implies all reviews park; measured, they do not. Narrowing
   here rather than widening the patch to a path with no defect.
