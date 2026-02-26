# Open Questions

## Wave selection ambiguity

No `<lf:wave>` tag in prompt and branch is `main` (not `<wave>.main`). Seven waves exist: agentapi, chords, infra, living, loop, mobile, remote.

**Assumption made**: picked `chords/02-chord-crud` based on highest combined urgency/importance/readiness score. Phase 01 shipped, 02 is foundational for phases 03-04, and scope is well-defined.

If a different wave was intended, re-run ingest with an explicit `<lf:wave name="...">` tag.
