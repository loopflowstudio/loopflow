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

- **Landing gate narrowed (assumed, reversible):** a Task run waits at a merging
  step only while a person's stop stands, not whenever unapproved. "Whenever
  unapproved" would park every `code` run at `land` after demo, since that run
  never reaches `end` with no interactive step left. The stop is the case
  LOO-687 needs.
- **Stop columns are local, not a WorkEdit:** adding a `WorkValue` variant
  changes the peer-replicated edit format released peers parse. A Machine move
  therefore does not carry a stop (`shipped_at` is already cleared by it).
- **No "Not shipping" Progress event** on a skipped route approval; status
  derives the standing stop line instead, so no per-run dedup state.
- **Starter has no Loopflow variant:** ancestry cannot tell `restart_ships`
  children from a person's chain (Graph children of `lf task run` also have
  parents). A broken chain reads "attribution unresolved". A restarted ship
  inherits the reading process's starter.
- **Steer `durable::Author` unchanged:** a Session steer keeps
  `Author::Named(requester)`; the steer text names the Session. Adding a
  variant to a persisted enum is a reader-compat change worth its own Task.
- **`ShippingSnapshot.approved_by` not added:** `TaskSnapshot.ship_approval`
  carries it, keeping roadmap fixtures unchanged.
