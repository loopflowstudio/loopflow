# Remaining design choices — LOO-366

Jack Heart concluded review before the October 5 implementation attempt. The
[design](keep-every-wave-ready-for.md) owns accepted decisions, implementation
status and evidence; earlier review notes remain at
`6dc8536fedce0852bd4931682c7ffb0a871cf17d:scratch/questions.md`.

- Preservation mechanics: transactional ingestion alone leaves Task restart's
  independent Project writer able to reverse Completed to Started (new failing
  public-store regression). Remove execution consumers' captured planning writes
  alongside accepted-observation projection, including Task-update reconciliation’s
  separate `update_task_plan`; retain local execution choices.
  Cover mutation readbacks, detail ingestion, partial Task updates and held Wave
  locks, then registration/relocation and checkout admission. The new entity-age
  regression also fails; preserve each entity's acquisition time and historical
  binding. No new product decision is needed.
- Chapter metadata representation and projection. Cross-Wave historical inspection
  remains proposed; Wave-scoped chapter creation is requested.
- Skill/Flow composition and the retained Task-candidate format between KR planning,
  chapter creation and separate Task admission.
- Designated fixture Waves and provider write authority beyond Intelligence.
  Jack Heart selected its exact Project for autonomous repair; the design retains
  that authority and the unproved Backlog/empty-Flow acceptance boundary.

Shared local binding ownership and the bounded historical name-only cutover are
resolved. Neither decision establishes implementation or installed acceptance.
