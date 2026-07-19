# Feedback runtime implementation review

Each slice records its accepted boundary, corrections, and proof here. A flow
wrapper exit is never accepted as evidence without source deletion and
behavioral tests.

## Slice 1 — explicit PR state

Accepted. `ReviewGateState` and its requested/active/approved/change-requested
branches are deleted rather than renamed. `OpenPr` is only a recommended action
when an open PR has passing checks; it owns no Wait, Run, WorkStatus, Feedback,
or completion transition. A merged `ContinueTask` PR recommends the next serial
PR immediately. Only an explicit `CompleteTask` disposition enters Task
completion checks.

Migration `0.11.037_after_merge_continue_task.sql` maps historical `review` to
`continue_task`, rebuilds the current constraint around the two honest values,
and rejects a new `review` write. Missing disposition on a merged PR is now an
invariant violation instead of silently selecting completion.

Proof: exact deleted-symbol/dead-field searches are empty; the action behavior
tests and migration test pass; `cargo fmt --check` passes. The automated
`lf code` wrapper made no source change and was rejected as evidence.

## Slice 2 — Radio and channel identity

Accepted. The Radio CLI and hidden subscription compatibility surface, bus
store/runtime/listener/retention, channel-family identity, `LF_CHANNEL`, and
current bus tables are deleted. `MessageOp::Say`, machine byline fields, and
the replay-only `ChannelOpened` event are deleted with their Rust and Swift
wire consumers. Human Wave conversation remains one plain message/steer
surface; ordinary Rust channels and prompt system/task channels are unrelated.

Migration `0.12.001_drop_agent_bus.sql` drops both bus tables. Historical
schema fixtures retain their old table definitions so the current migration
is exercised. Project promotion now reports in its typed child-Wave thread and
does not publish a second lossy copy.

Proof: exact current-source Radio/channel-identity search is empty; all 36
migration tests, 7 Wave journal tests, and 3 Wave resolution-matrix tests pass;
Swift contract tests and `cargo check -p loopflow` passed during implementation;
`cargo fmt --check` passes. The first migration proof caught and corrected a
missing registration and a current probe that still queried the dropped table.

## Slice 3 — file-only memory

Accepted. `wave/<name>/MEMORY.md` is the only memory truth. Prompt assembly
reads applicable ancestor files oldest-first. The journal has no memory event,
the runtime has no memory state or broadcast, the listener has no memory route
or SSE frame, Swift has no memory stream state, and `lf memory` exposes exactly
the required `show` subcommand. The read path does not require a server.

The export-memory builtin and all add/log/update, evidence, Doctor, cron,
golden, and server-owned-curation guidance are deleted. Historical migrations
remain historical; generic mutation result types named `*Receipt` are not part
of this slice.

Proof: journal 12/12, runtime 24/24, Wave context 17/17, thread 9/9, CLI memory
3/3, parser 1/1, Doctor 15/15, resolution matrix 3/3, builtins 9/9, prompt
81/81, cron 7/7, golden parity 1/1, Swift Wave chat 24/24, and Swift DTO fixture
7/7 pass. Exact current-source searches are empty; `cargo fmt --check` and
all-target Clippy with warnings denied pass.

## Slice 4 — explicit prompt context

Accepted. `PromptComponents` and launch seeding have no recent-Wave-chat field,
tag, budget, renderer, gatherer, trace asset, or debug projection. Skill launch
seeding accepts one optional Wave memory section rather than an arbitrary list
of ambient Wave sections. Wave conversation, journal, `ChatTurn`, `lf chat`,
and Mac chat UI remain unchanged as product surfaces.

Separate Project and Task integration proofs seed a complete real Wave journal
turn, build `project_pursue` and `task_pursue` prompts, and prove both sides of
the unrelated conversation are absent while `MEMORY.md` is present.

Proof: Wave context 7/7, prompt 81/81, run 35/35, and context integration 22/22
pass. Exact forbidden-symbol/tag search is empty; `cargo fmt --check` and
all-target Clippy with warnings denied pass.

## Slice 5 — planning truth

Accepted. `launch_context.rs`, Project/Task launch receipts, and Linear snapshot
wrappers are replaced by `planning.rs` with `ProjectDefinition` and
`TaskDirective`. Project exposes `definition`; Task exposes `directive` and
`project_id`. Task SQL selects no parent Project PM metadata. Prompt, status,
roadmap, journal, and diagnostic paths resolve the current parent deliberately.

No migration or compatibility layer was added: existing normalized columns
already express the facts, and the change removes Rust-level duplication.
Runtime agent/provider/provider-session, abandon intent, and handoff fields are
preserved for the server-topology slice.

Proof: focused Task seed, status, store-normalization, and parent-update tests
pass; they show separate Task/Project snapshot timestamps and a changed Project
definition affecting the next Task view without rewriting its directive. Exact
retired-type/copied-parent and Task SQL projection searches are empty;
`cargo check --all-targets`, `cargo fmt --check`, and all-target Clippy with
warnings denied pass.

## Slice 6A — explicit Feedback continuation

Accepted. `lf work feedback` resolves User-attention Feedback and presents its
recorded Launch. Presentation success, failure, process exit, and signal
handling cannot mutate the Feedback or advance the flow. `lf work continue` is
the only command that closes the current checkpoint, under User or immediate
parent Run authority and fenced by the checkpoint Basis.

The continuation-on-exit flags, hidden guard process, exit policy, retry/lock
state, conditional presentation callback, and Feedback escalation command,
transition, and receipt are deleted. A Feedback route is chosen when the
checkpoint opens and cannot be changed afterward.

Proof: the parser accepts the surviving presentation and continuation commands
and rejects both retired flags and `work escalate`; exact retired-symbol
searches are empty. The focused parser and durable-store suites and Swift local
launcher tests pass from implementation; `cargo fmt --check` and all-target
Clippy with warnings denied pass. The automated `lf code` wrapper again made no
source change and was rejected as evidence.

## Slice 6B — explicit Task Feedback reviewer

Accepted. `FeedbackReviewer::{User, Parent}` names the actual authority on each
`TaskPhasePlan`. The standard lifecycle routes kickoff clarification to User,
iteration to the immediate parent Project, and gate mutation to User. `lf task
run|start --reviewer user|parent` can deliberately override future checkpoints;
the overloaded Task `--headless` spelling is rejected.

Migration `0.12.002_task_feedback_reviewers.sql` renames all three stored phase
columns and maps `require|defer` to `user|parent`. Current readers accept only
the new vocabulary. Changing a Task plan does not mutate Launch attention, so
already-open Feedback stays routed to the peer that was chosen when it opened.

`InteractionPolicy`, feedback-only `FlowAction` variants, and their dead policy
evaluator are deleted. Provider launch may still be headless; that transport
surface no longer decides who reviews Task Feedback.

Proof: parser and Task launch tests, default/override model tests, store
round-trip, migration mapping, and two reviewer integration tests pass. The
integration proof changes an existing Task to Parent while its open Feedback
remains routed to User. Exact current-reader and retired-policy searches are
empty; `cargo fmt --check` and all-target Clippy with warnings denied pass.

## Slice 6C — delete producerless evidence Receipts

Accepted. The evidence `Receipt` model, five evidence kinds, parser, resolver
DTOs, and `lf receipt` command are deleted. File-only Wave memory has no
structural receipt authoring, and no other current feature produced these
pointers; keeping a universal local resolver created API without a writer.

Receipt-only PR reference and identity types, `TaskPr::pr_identity`, and the
repository-wide `all_task_prs` query are also deleted. CI incident identity
uses the current Task PR's GitHub record directly and keeps its small private
URL-to-repository helper. Generic typed outcomes such as `SteerReceipt`,
`AdvanceReceipt`, and `TaskStartReceipt` remain because they report completed
operations rather than pretending to be evidence links.

Proof: the first-class command is absent, an authored flow rejects `receipt`,
Task behavior 15/15 and the Wave resolution matrix 3/3 pass, and the exact
evidence API/docs search is empty. `cargo fmt --check` and all-target Clippy
with warnings denied pass. No tombstone command or compatibility alias exists.
