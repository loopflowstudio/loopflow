# Subwave decisions and assumptions · LOO-354

Jack Heart · 2026-09-30. [Accepted design](define-durable-subwave-identity-and.md).
The original LOO-329 notes remain at
`backup/define-durable-subwave-identity-and-20260930:scratch/subwave-questions.md`.

- No design review. Complete local implementation and focused proof, then stop
  at the supervising Flow's interactive demo. LOO-298's autonomous landing
  authorization is not a replacement for this Task's demo boundary.
- Keep the read slice's short curation hint and README-first ordering within
  each ancestor directory. These are reversible implementation choices, not
  a claim that Jack approved their exact wording.
- Parent `realign` explicitly reads children's files; ordinary gathering never
  includes descendants. Descendant depth remains unspecified, so this pass does
  not change realign's reading policy.
- Do not copy obsolete LOO-298 migrations from #1318. Parent/schema changes must
  use this branch's current parent model and its ordinary migration workflow.
- The release Initiative, plan/Task moves and live schedule transfer happen
  through installed `lf` after code lands. A file fixture cannot prove that split.
- Parent rename and arbitrary directory reparenting need stable identity evidence.
  Current discovery has neither parent assignment nor a directory-move identity
  contract. Do not infer a moved Wave from a coincidentally matching leaf name.
  Resolve that implementation question within the accepted id/name/parent model
  when building step 2; no new identity sidecar or metadata ledger is selected.

Verification is resource-blocked; see [read-slice evidence](subwave-read-slice.md).
No new user decision is needed to resume after capacity recovers.
