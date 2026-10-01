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
- Parent `realign` explicitly reads child Waves' top-level Markdown; ordinary
  gathering never includes descendants. Gate added this instruction to the skill.
  Deeper descendant traversal remains unspecified; direct children are the
  minimal implementation of the accepted exception.
- Do not copy obsolete LOO-298 migrations from #1318. Parent/schema changes must
  use this branch's current parent model and its ordinary migration workflow.
- The release Initiative, plan/Task moves and live schedule transfer happen
  through installed `lf` after code lands. A file fixture cannot prove that split.
- Parent rename and arbitrary directory reparenting use the authored Wave UUID
  described below. Do not infer a moved Wave from a matching leaf name; no
  identity sidecar or metadata ledger is selected.

The 2026-09-30 implementation resumed above the emergency reserve (44.1 GiB).
All seven final reader commands passed; [read-slice evidence](subwave-read-slice.md)
retains the earlier resource failures.

Implementation choice: persist the existing Wave UUID as `id:` in GOAL.md
frontmatter when registering authored Waves. Neither leaf-name matching nor a
path-derived UUID can distinguish a move from a new Wave. This uses the existing
authored file, not an identity sidecar or rename ledger. Discovery adopts the
registered UUID at the current address when the file has no id, then carries it
through directory moves and fresh clones. SQLite owns name and parent; the full
address is derived for reads. Conflicting copies remain unresolved rather than
silently taking another Wave's identity.
