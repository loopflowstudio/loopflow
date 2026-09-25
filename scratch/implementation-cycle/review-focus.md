# Cycle 1 review focus

The parent recorded the pre-cycle Task change inventory at
`/tmp/loo291-cycle-start-changes.json` and existing non-scratch source copies at
`/tmp/loo291-cycle01-before/`. The full `lf task diff --json` response is truncated;
never represent that artifact as complete. Read `lf task diff LOO-291 <path>` for
changed source paths and inspect named files/earlier receipts for wider context.

Review actual value, not merely command acceptance:
- Raw and skill-invoked Sessions receive useful names before the first user reply.
- List and open agree; reads do not write or invent new identities.
- Human rename survives subsequent generated suggestions, including concurrent
  writers; no provider restart, Task rename, branch rename or Run replacement.
- Interactive, Ask and Flow boundaries all preserve their independent action
  semantics. Required Session → Run identity is not a title lookup heuristic.
- Failure/blank input leaves the previous name usable; a missing target is explicit.
- Swift fixtures require new wire fields, without independently defaulting them.
- The operating instruction names a real command and cannot silently overwrite a
  user name. No second Session registry, competing title writer or duplicated store.
- Historical generator recovery is attributed accurately: distinguish recovered
  words/algorithm from any newly selected additions.

Name this slice's remaining obligations separately from complete native design
acceptance. A passing naming CLI does not establish the native frame, Flow graph,
comment reads, exact Flow membership or configured terminal preservation.
- Check Home routing: remote Flow/Ask records must not silently rename or seed a
  different local Run. Respect the existing authority/routing path or surface the
  precise unsupported operation; do not create a local title merely to hide it.
- A Session boundary ID may survive replacement of its linked Run during open/
  recovery. Check renamed Ask/Flow Sessions through that actual replacement path:
  storing a name under the old Run must not silently lose a human title when the
  Session's new Run is published. Session identity and Run identity have different
  lifetimes; this is a concrete ownership boundary to exercise, not a generic test.
- The candidate operating text invokes `lf session rename "$LF_RUN_ID" ...
  --suggest`. Verify that exact invocation inside Interactive, Ask and Flow
  Sessions. Ask/Flow boundary IDs differ from linked Run IDs; accepting a Run ID
  must resolve to that exact boundary or guidance must select its real Session ID.
  A standalone rename by boundary ID does not prove this agent-facing instruction.
