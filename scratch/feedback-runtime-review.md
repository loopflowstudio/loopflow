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
