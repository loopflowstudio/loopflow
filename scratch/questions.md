# Questions

- LOO-579 liveness: the receipt treats any open, not-dead Process in the Task's
  work (outside the caller's lineage) as live, so an unrelated interactive
  Session in the checkout also makes `lf task interrupt` record an interrupt and
  name that Process. Chosen over argv parsing for one detector. Concept review (2026-10-24): that
  receipt says "interrupt recorded", not "stopped" (implemented; see design,
  Receipt repair).
