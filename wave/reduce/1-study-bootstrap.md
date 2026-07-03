---
priority: high
---

# Study bootstrap

**Finish line:** Reduce has a current map of loopflow's architecture, reference
systems, leverage points, and unfinished work chains. One assess pass can read
that state and name the next move without re-discovering the basics.

## Context

Reduce does not start by deleting code. It starts by making the system legible:
what exists, what outside systems are worth learning from, where one
improvement compounds, and where shipped work is only half-integrated.

The first bootstrap maps are:

- `analysis/architecture-map.md` - the internal system model and state
  boundaries
- `analysis/reference-map.md` - external systems to study for each loopflow
  surface
- `analysis/leverage-map.md` - areas where one improvement makes every session
  better
- `analysis/continuity-map.md` - backend/UI/docs/runtime chains that can drift

This is study output, not a proposal. Proposals start when a concrete design
change survives the maps.

## Done when

- The four bootstrap analyses exist with a HEAD freshness marker.
- `docs/architecture.md` explains the whole system in one place.
- The next reduce assess pass can choose between refreshing stale analysis,
  drafting a proposal, or filling a specific missing map.
