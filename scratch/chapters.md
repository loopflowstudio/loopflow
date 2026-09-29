# Chapters: current Projects in Linear

LOO-298 · Jack accepted this model “for now” on 2026-09-27.

A Chapter is the set of In Progress Linear Projects, one per Wave, sharing a
name such as `2026-10`. Planned Projects describe future work; Completed Projects
retain history and Tasks. There is no Chapter table, packet, configuration line
or local switch. Existing Linear Projects must survive. Several Homes converge
by reading the same provider statuses; newest name never selects a plan.

A Wave's current Project is its one In Progress Project. Different names across
Waves or multiple current Projects are visible conflicts; rotation refuses until
resolved. Project `flow:` is required and supplies Task's default; explicit Flow
selection remains allowed. A new successor copies the predecessor's Flow only;
an existing Planned successor keeps authored Flow, KRs and prose.

`lf repo new-chapter NAME --dry-run` previews every Wave and Task disposition;
execution creates or adopts the named Planned successors, moves started unfinished
Tasks, cancels only proven untouched backlog, and flips successor/predecessor
statuses. Retry the same name after interruption. Linear operations converge;
there is no cross-Home transaction. Current installed command spellings may differ;
inspect the selected executable before operating it.

## Retain through final integration

- Task identity, worktree, PR, FlowSession/capture and Started timestamp survive
  transfer; terminal Tasks remain historical. Provider terminal conflicts, missing
  Task evidence, inaccessible checkout and unknown workflow state never authorize
  cancellation. Fresh claims and publication facts fence local retirement.
- Deterministic successor UUIDs use provider-supported v4 bits. Reconcile lost
  create/attach replies by exact identity. Enumerate fresh membership and point-read
  retained omitted IDs; absence from a list does not prove deletion.
- Archived Projects decode as historical Completed unless Canceled, even if raw
  Linear status says Started. Fresh Canceled predecessor/successor evidence must
  not be overwritten by final status flips.
- Upgrade retains existing Project identity and custom Flow. Temporary
  `legacy_current` evidence distinguishes known current, known noncurrent and
  unknown pre-upgrade Projects. Convert the legacy Flow key in place before
  rename/sync can erase prose or defaults; explicit sync confirms provider writes
  before clearing markers. Ordinary reads project pending conversion without
  provider mutation. Unknown competing plans remain unresolved.
- A copied clean local Task does not prove absence of work on another Home.
  Local SQLite/PR fencing supplies no distributed lock or filesystem transaction.
  Provider content can race an external editor. Retain these limits explicitly.
- Delete Chapter storage/packets/history APIs and obsolete recommendation readers;
  preserve released migration history and deliberate upgrade evidence. Do not
  restore a permanent `recommended:` alias or silently substitute `feature`.

## Proof still required at the finish line

Reconcile the integrated H7 code after execution-owner changes. At published
`594c7c319f`, hosted Rust job109231281571 passed all 14 Chapter tests before
stopping at the unrelated comments fixture. Supervisor inspected the rotation
matrix: two Waves, twelve interrupted mutations, recovery from either private
store, a mutation-free same-name retry, expected final Project/Task statuses,
and preserved Task identity/worktree/plan/PR/captured Flow at the transfer case.
Authored successor content, legacy response loss, ambiguous/current conflicts,
unobserved second-Home work and archived history also pass. These exercise Rust
operations against simulated Linear; they do not run the CLI or live provider.

Remaining proof is specific: second private Home adopts by public sync alone
(the matrix calls rotate); exact Started timestamp survives transfer (the matrix
does not assert that column); Project Flow drives an actual Task launch with no
override (the selector has a unit test); populated draft/canonical upgrade retains
Projects through the final execution schema. Reuse unchanged covered behavior
and rerun affected cases only where subsequent owner changes require it. See
[evidence](evidence.md). No live Linear rotation or installed acceptance follows.

Open product edge: a Wave with no In Progress Project has no accepted automatic
creation policy. The earlier “create on Task start” sentence was a proposal.
