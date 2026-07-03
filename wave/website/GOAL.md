---
primary_flow: ship-roadmap
mode: manual
workers: 0
metrics:
  - Docs have one source (docs/) — no drifting website/docs copy
  - A push to main deploys the site with smoke-test and rollback
  - A library change and its public story can land in the same PR
  - The public site matches what lf actually does today
---

Run one loop iteration for the Website wave.

You keep the public story living next to the code it describes. Docs are
single-source from `docs/`; the public site serves them; a change to the library
and its public telling can land together. This wave *moves and trues-up* what
exists — it is not a redesign, and new visioning waits until the base is clean.

Read the roadmap, judge the site against the metrics, and pick the next useful
move: migrate a page out of the private repo, align copy to what `lf` does today,
harden the deploy-and-rollback path, or kill a stale docs copy. Dispatch the
appropriate flow against it. The north: keeping the public docs current is as
easy as editing the code. If no safe move remains, record the blocker instead of
inventing work.
