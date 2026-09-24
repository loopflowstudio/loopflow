# Open design questions

## Task terminology assumption

“Open” means started and unfinished, including paused, blocked, or review-waiting Tasks. “Unopened” means unstarted backlog, not every issue whose Linear status is nonterminal. Filing or preparing a Task alone does not establish that execution began. The design now specifies a deterministic evidence classifier shared by preview, application, and review; ambiguous historical state must not be auto-abandoned.

The human resolved the prior execution question: open Tasks move automatically with identity and progress intact; unopened Tasks are abandoned/closed. The earlier proposal to stop active work is superseded.

## Placement

No exact owning Wave or existing implementation Project was named. Placement remains unresolved; no live PM reads or writes are needed to settle the product model.

## Implementation choices

- Wave remains the persistent conversation and sole next-work decision surface.
- `lf wave new-chapter` calls one deterministic rotation API; start/review chapter use its preview, historical snapshots, and receipts. This supersedes the initial `lf project new-chapter` spelling following the human's Wave-only UI requirement.
- Convert the existing listener form `lf wave <name>` to `lf wave serve <name>` while retaining convenient top-level start/status/chat commands. Implemented alongside the internal listener launchers.
- Public Desktop selection is Wave or Task; chapter history is a view/filter. Internal Project and Run identities remain diagnostic provenance. The shared current read models, cached plan, Sessions, Activity, and metrics must all use this same scope.
- One resumable Wave/chapter binding controls current resolution; Linear owns authored content and the Git archive preserves accepted chapter history.
- Existing multi-Project portfolios migrate through a fresh chapter, moving started Tasks and abandoning unopened backlog.
- New Waves receive an initial Project; empty or completed current Projects remain current until chapter rotation.
- Durable metric instruments survive on the Wave; chapter-specific proof does not inherit old verdicts.
- Human clarification: the objective belongs to the Wave; Tasks, KRs, and metric targets belong to the chapter Project. Present both ownership scopes through the Wave UI. This supersedes the prior blanket statement that metrics belong to the Wave.

The subsequent `$implement` instruction authorized these reversible implementation choices. See `scratch/projects.md` for the complete design.

Implementation resumed following the subsequent `$implement` instruction. The findings are in `scratch/research-wave-ui-collapse-5db824ef.md`. Native rendering has been inspected with offline fixtures. Live portfolio migration remains unapplied; applying it requires acceptance of its concrete preview.

## Ownership correction — implementation gap

Observed: `ProjectContent` already owns KRs, and Tasks have a Project parent.
`MetricContractDefinition`/`MetricContract` still own `target` in the Wave metric
contract, including it in the contract revision hash. Project `definition`
is also displayed as chapter-level objective prose. These parts need to be
reconciled with the clarified design. The prior test passes do not establish
Project-owned metric targets. Preserve measurement identity/history while
moving target authorship and dated evaluation into the chapter plan.
