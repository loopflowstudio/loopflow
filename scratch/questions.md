# Open questions — W2-141

## Backlog decay policy (wave-level, not a view decision)

18 of 26 Product tasks are filed-and-unstarted. That is not yet a crisis, but it
is the direction of travel, and the Now/Available design makes the pile *more*
visible, not less.

The wave already resolved "no backlog" in Linear's favor, with the explicit note
that what it bought — *"the open runs ARE the wave's open tasks"* — is gone.
Nothing now prevents the tracker filling with intent nobody does.

The prior art disagrees sharply, and both answers are coherent:

- **Linear**: manage the backlog by *decay*, not discipline — auto-close and
  auto-archive on staleness. *"Important ones will resurface; low priority ones
  will never get fixed."*
- **Basecamp**: delete the concept. *"Backlogs are a big weight we don't need to
  carry."* Bet per cycle; let unbet ideas die.
- **Height**: automate the pruning with an AI that triages and closes stale items
  unprompted. Shipped it, then shut the company down (24 Sep 2025). The only
  natural experiment available, and it argues for machine *suggestion* over
  machine *decision*.

Not decided here. It is a wave-level judgment about how Product carries intent,
not a property of a view, and picking one silently inside a rendering task would
be the wrong place to make it.

**Assumption taken for the design:** the roadmap renders everything filed, with
no automatic pruning and no hidden staleness filter. If decay is wanted later, it
is a PM-side policy, not a view predicate.
