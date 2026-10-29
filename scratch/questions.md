# LOO-687 open questions

- **Ship intent (assumed, reversible):** kept Jack's #1741 single approval (a
  route into `end` approves), plus a durable person stop that blocks only
  implicit re-approval, plus "an unapproved run waits at `land`". The
  alternative was explicit-only approval (`lf ship` alone ships). It's simpler,
  but it reverses #1741. Jack decides whether to go explicit-only instead.
- **Halt is not a stop (assumed):** `ship_halt` after repeated identical failures
  doesn't block a later route into `end` from approving again.
- **Agent `lf ship` (assumed):** a Task Session calling `lf ship` explicitly
  still clears a person's stop. The skills say not to do that unless the person
  asks. There's no code guard.

## Explore pass 1 (2026-10-29, headless)

`scratch/explore.html` shows three visions: A durable stop + #1741 (the plan),
B explicit-only `lf ship`, C an append-only ship ledger read as state. Headless
recommendation: A, with halt-is-not-a-stop, Session `lf ship` allowed, and an
unapproved run waiting at `land` (page answers 1a 2a 3a 4a). Not accepted;
Jack chooses. B reverses #1741 two days after Jack restored it; C rewrites
every `shipped_at` reader for history this Task doesn't need.

## Implement pass (2026-10-29, headless)

The assumptions made while implementing are now in the plan: the landing gate
covers only a standing stop, stop columns are local rather than a WorkEdit, no
"Not shipping" event, no Loopflow starter, `durable::Author` unchanged, and no
`ShippingSnapshot.approved_by`. Each one is reversible, and Jack decides.
