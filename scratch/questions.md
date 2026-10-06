# Remaining design choices — LOO-366

Jack Heart concluded review before the October 5 implementation attempt. The
[design](keep-every-wave-ready-for.md) owns accepted decisions, implementation
status and evidence; earlier review notes remain at
`6dc8536fedce0852bd4931682c7ffb0a871cf17d:scratch/questions.md`.

- Preservation mechanics need no product decision. Exact-ID input, transition-owned
  selected issue IDs, whole-input preflight/reservation and binding switching are
  implemented. Retained creation intent prevents retry from recreating a missing existing
  destination; it is an implementation choice, not additional approval from Jack Heart.
  Operation fixtures and remaining CLI/crash acceptance are recorded in the design.
- Chapter metadata representation and projection. Cross-Wave historical inspection
  remains proposed; Wave-scoped chapter creation is implemented.
- Skill composition uses retained exact-ID JSON plus candidate notes grouped by destination
  UUID, with stable local keys and admitted issue IDs. This reversible implementation
  choice preserves candidates across separate admission; operation-backed proof remains at gate.
- Designated fixture Waves and provider write authority beyond Intelligence.
  Jack Heart selected its exact Project for autonomous repair; the design retains
  that authority and the unproved Backlog/empty-Flow acceptance boundary.

Shared local binding ownership and the bounded historical name-only cutover are
resolved. Neither decision establishes implementation or installed acceptance.

October 6: Jack Heart authorized SQLite selection, one-time YAML import and LOO-382
integration. Import at explicit ensure/binding/applied rotation retains the original
file bytes as evidence and makes the old file inert; absence is recorded once too.
This is a reversible implementation choice within the accepted cutover. No additional
provider, installation, release or managed-review authority follows from it.

October 6 reconciliation: authored memory and scratch fit. The complete supplied
Task/steer input was read; `lf context --skill realign --json` reports the generated
launch goal at 17,271/16,000 tokens at gate. Its provider-owned brief,
steers and generated workspace inventory cannot be reduced by local plan curation.
No budget was raised; the launch-input owner retains this overage.
