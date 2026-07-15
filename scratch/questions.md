# Assumptions

- W2-135 process leases are already on `main`. This PR records the referenced
  body generation but deliberately adds no second lease or process owner.
- `lf handoff` is the durable primitive name. Presentations may wrap it, but
  Wave, Project, and Task remain the only parent lifecycle nouns.
- The next migration ordinal on this branch is `0.11.008`; concurrent branches
  may require an ordinal-only rebase before landing.
