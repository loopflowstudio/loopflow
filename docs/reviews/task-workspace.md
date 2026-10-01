# Task workspace review — October 1, 2026

Jack Heart approved landing PR #1369 and requested that LOO-353 keep going.
Approval authorizes this delivery checkpoint; unexecuted proof below remains
unexecuted. This Session is independent, not a waiting managed Flow boundary.

## Delivered behavior

Sessions group by resolved checkout and owning Home, independently from recorded
Run attribution and Flow membership. Retained workspace owners keep shell processes,
Session surfaces, file drafts and layout while switching Tasks. Directory browsing
and revisioned file saving do not require a Project or PR. Readable symlinks are
read-only; access changes precede content-revision shortcuts and preserve drafts.

Interactive stages and participation indicators refer to exact captured boundaries;
Ask retry keys remain caller-scoped. Closing or hiding a pane does not complete a
Session, Ask or Flow. The separate Task terminal owner is removed.

Jack rejected the original left-side button stack. The revision uses compact
creation and Files controls, a collapsible Sessions sidebar, and Task details for
the full Flow. Terminal presentation stays unchanged. Lowercase Wave names receive
readable title labels; names containing uppercase characters remain authored.

After IDE research, Jack requested that split layout drive Session selection.
Ordinary clicks focus an existing pane or replace the active Session pane; other
splits retain geometry. Command-click toggles visibility, while Option-click or
Open alongside reveals a retained pane or splits right. Hidden panes retain ratios.
Native surfaces and running shells survive. Visible rows and focused row are distinct.
One existing Session defaults to hidden sidebar; multiple Sessions show it. Later
arrivals and manual choices are retained. No second selection registry or tab model.

## Observed evidence and remaining limits

Focused Rust/Swift behavior, shared DTO fixtures, Clippy and native build paths
passed during implementation. The last layout revision passed 44 focused Swift tests
and Xcode fallback build-for-testing. Earlier native input tests use fixture shells;
provider transport fixtures are simulated. Build success is not provider proof.

The configured demo used a migrated private copy of Jack's main Home and a matching
test-materialized CLI. It did not upgrade the live main database. Native captures
showed LOO-330, compact controls and restored planning. Growth, Product and then
Infrastructure each had one existing Project marked Backlog; Jack authorized repair
to In Progress with the existing feature default and preserved KRs. Historical
foreign-Team adoption now uses the same filter as ordinary planning reads.

Jack's screenshots also exposed Restore checkout reporting a conversation error for
an abandoned Task. The UI now explains existing terminal-state restrictions and
links to Task details; checkout failures use their own title. Historical checkout
restoration remains a product question, not implemented recovery.

The final reopened Task had no open Session in the copied inventory. Its empty view
does not prove populated Session selection. SnapshotService requires a key window;
“No window to snapshot” alone does not prove a crash or closed window.

Still unmeasured/unexecuted: configured provider Ask/Flow continuation and exact
caller release, remote owning-Home association, cross-Task focus/draft/process
retention, full real-file symlink/draft transitions, and at least twenty retained
layout actions with p95 below the proposed 100 ms target plus idle CPU/process
counts. The existing signpost ends at a main callback, not compositor presentation.
Jack approved landing without claiming these results. Carry them into LOO-353.

## Remaining ownership

- LOO-353 remains open: finish configured workspace proof and UX iteration; improve
  default Flow selection and direct source-file editing; update the website to the
  shipped focus-on-your-own-work story and remove obsolete resident/chat claims.
- LOO-364 owns primary repo/Wave/Task Sessions, safe native wake delivery and durable
  Flow switching. Do not duplicate that runtime work in LOO-353. Its older plan must
  be restated on the current AgentSession/FlowSession/Exec model before implementation.
- LOO-366 owns reliably available ordinary Projects with optional chapter coordination.
- LOO-367 owns Task admission/completion without unrelated workflow prerequisites.

Jack accepted Projects independent of chapters; chapters coordinate optional global
resets. Task contributes purpose/history/continuity to ordinary Flows, not a second
execution engine. Lower operations retain identity, authority, safe retries and
recovery. Broader candidate features remain proposals, not implementation approval.

## Retained design and research

The full accepted design and detailed evidence before scratch cleanup are retained
at commit `bc78c27c017bc93099c06fd342b51bc6110beb5d`:

- `scratch/growth-thoughts.md`: Unit 1 implementation, Unit 2 constraints, Unit 3 plan.
- `scratch/task-workspace-demo.md`: dated feedback, test commands and native limits.
- `scratch/kickoff-evidence.md`, `scratch/ask-evidence.md`: source and proof findings.
- `scratch/questions.md`: unresolved product choices.

Read them with `git show bc78c27c017bc93099c06fd342b51bc6110beb5d:scratch/growth-thoughts.md` (or the other exact path).
[Independent-operation research](independent-operations.md) preserves primary sources
and ranked follow-up candidates. Public behavior is documented in `swift/README.md`.
