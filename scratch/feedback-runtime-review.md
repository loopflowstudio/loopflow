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

Reopened proof audit, July 19: the required `lf code` loop again made no source
change and was rejected. The named integration proof
`merged_continue_task_rotates_to_a_working_pr_without_review_state` now starts
from a durable PR N, reconciles GitHub's merged reading with authored
`ContinueTask`, and crosses the production `pr_next` /
`ensure_working_pr_with_authority` store-and-worktree rotation. It observes a
pushed current branch and durable working PR N+1 while Work stays open and both
the Task gate proposal and Feedback checkpoint remain absent.

`observed_merge_completes_a_pr_marked_to_complete_the_task` separately proves
that `CompleteTask` reaches Done with only the one merged PR. Generic “review
gate” comments and wait reasons now name the explicit Task Gate Feedback or
the exact completion facts they enforce; no approval state was added. Both PR
proofs pass, as do Task actions 2/2, Task model 15/15, the historical
`review`-to-`continue_task` migration proof, `cargo fmt --check`, and all-target
Clippy with warnings denied.

## Slice 2 — Radio and channel identity

Accepted. The Radio CLI and hidden subscription compatibility surface, bus
store/runtime/listener/retention, channel-family identity, `LF_CHANNEL`, and
current bus tables are deleted. `MessageOp::Say`, machine byline fields, and
the replay-only `ChannelOpened` event are deleted with their Rust and Swift
wire consumers. Human Wave conversation remains one plain message/steer
surface; ordinary Rust channels and prompt system/task channels are unrelated.

Migration `0.12.001_drop_agent_bus.sql` drops both bus tables. Historical
schema fixtures retain their old table definitions so the current migration
is exercised. Project promotion never enters that human thread: an explicit
promotion nudge asks the existing observer to verify the durable parent link,
then deliver one typed `PromotionWake`. It is journaled under a deterministic
parent-link id and replayed through the existing resident inbox. Background
child-observation polling never infers a fresh promotion from old ancestry. The
scheduler consumes the wake once; repeated nudges do not append another input
or start an overlapping pass.

Reopened promotion repair, July 19: `complete_promotion` nudges the existing
`/observations` door after the child listener is live. It cannot invoke `lf
chat` or `/messages`; the observer refuses a nudge that does not match registry
truth. Once ancestry and residency are durable, transport/refusal failure is a
best-effort warning rather than a false promotion failure; heartbeat remains
the fallback. The required `lf code -b -m opencode --max-turns 12 --docs
scratch/promotion-wake.md` loop exited successfully but made no source change
and was rejected as evidence.

Proof: the full-wire promotion test proves one typed wake starts exactly one
three-step child-Wave flow, records one `PromotionObserved`, answers its
deterministic id, and records no `UserMessage`. The reopen proof consumes that
id, closes and reopens the journal, and proves a repeated nudge cannot restore
or duplicate it. The registry proof shows background polling over existing
ancestry records nothing, while two explicit verified nudges still journal one
wake. The best-effort transport proof shows a dead observer endpoint cannot
turn completed ancestry/residency into promotion failure. The human door rejection/acceptance and
fresh-schema Agent Bus absence tests pass. `Cli::command()` has no Radio
subcommand, source module, or API; unknown first verbs remain owned by external
skill/flow discovery, with no Radio tombstone parser branch. Wave journal tests
pass 12/12, and `cargo check -p loopflow --tests` plus `cargo fmt --check` pass.
Swift contract tests passed during the original implementation. The first
migration proof caught and corrected a missing registration and a current
probe that still queried the dropped table.

Focused repair rerun: typed full-wire pass 1/1, explicit registry verification
1/1, consumed close/reopen dedupe 1/1, best-effort dead-endpoint nudge 1/1,
human-door contract 1/1, current-schema absence 1/1, and structural Radio
absence 1/1. `PromotionWake` and its delivery method remain crate-private; the
repair adds no public Radio, bus, Message, Session, or generic server API.

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

Reopened documentation audit, July 19: the required `lf code` loop made no
changes and was rejected as evidence. The Wave runtime README no longer
advertises deleted memory SSE events or `/memory` routes, and the Swift chat
client comment now names only the surviving SSE frames. The file-only runtime
and direct-read CLI remain accepted.

One current-context defect remains blocked rather than hidden:
`wave/intelligence/MEMORY.md` still describes the deleted stream and write API.
This repair did not edit it because the operating contract calls Wave memory
server-owned and the repair explicitly prohibited a direct file write. No
owned alternative exists: `lf memory` is read-only and the server has no memory
route, while `update-wave` still prescribes the forbidden ordinary file edit.
The exact authority/tooling contradiction is recorded in `scratch/questions.md`;
Slice 3's implementation is complete, but its active Intelligence context is
not curated until that ownership decision is resolved.

Repair proof: the live-memory claim search is empty across current Rust, Swift,
docs, skills, and tests outside the explicitly blocked Wave memory; CLI memory
tests pass 3/3, the one-subcommand parser proof passes 1/1, Swift Wave chat
streaming passes 24/24, and `cargo fmt --check` passes.

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

Proof: `evidence_receipt_command_is_absent` shows that the first-class command
is absent and unknown `receipt` input remains eligible for external skill
discovery. Task behavior 15/15 and the Wave resolution matrix 3/3 pass, and the
exact evidence API/docs search is empty. `cargo fmt --check` and all-target
Clippy with warnings denied pass. No tombstone command or compatibility alias
exists. The earlier claim that an authored flow rejects `receipt` was removed:
there is no such named fixture or test, and command absence is the relevant
contract.

## Slice 6D1 — honest Work and Launch identity

Accepted. Task workspace snapshots carry `task_id`; child activity carries
`work_id`; Swift Launch presentation consumers use `launchId` for the existing
`launch.id`. Rust/Swift DTO mirrors, fixtures, tests, and Mac consumers change
together with no fallback key or compatibility property.

Production Project and Task code now names stable records and ids Project,
Task, or Work rather than Session. User docs likewise describe Task Work,
Project Work, and Launches. Historical migrations retain the vocabulary of the
schema they migrate, and explicit provider session ids plus real tmux, Ghostty,
browser, and human sessions remain.

Proof: Rust chat-turn behavior 6/6 and Wave journal 7/7 pass. Swift DTO,
registry, local launcher, and Wave transcript coverage passes 53/53, including
workspace JSON, child activity, and Launch presentation. Exact false-Session
and retired wire-key searches are empty outside historical migrations and the
explicit substrate allowlist; `cargo fmt --check` and all-target Clippy with
warnings denied pass. Clippy caught and removed one redundant field spelling.

## Slice 6D2 — honest Mac projections

Accepted. The Mac runtime projection is `WorkCensus` containing
`WaveActivity` and `WorkActivity`; its types, files, views, copy, and tests no
longer call heterogeneous Work, Run, and Launch rows Sessions. The one-value
`SessionAction` and every row action array are deleted. A row is openable
exactly when it carries the existing optional `launchId`.

Context Lab now mirrors its source data: `LaunchSetQuery`, `LaunchSetTotals`,
`LaunchLane`, `launches`, `LaunchOutcome`, launch counts, and a `launch_set`
flame root. Rust, JSON fixtures, Swift, and UI copy change together with no old
keys, aliases, or defaults. A final broad audit also renamed false stable-Work
locals in webhook, lfd, PM/store tests, Wave observation, pruning, and promotion
paths; the surviving Session allowlist is provider resume state, provider
configuration, usage-window labels, URLSession, tmux/Ghostty, and human prose.

Proof: Rust Context Lab 20/20 and parser 1/1 pass; Swift Context Lab 10/10, DTO
fixtures 7/7, and RegistryQuery 17/17 pass with the full Mac target compiled.
The follow-up Task 15/15, store persistence 2/2, and reteam 12/12 proofs pass.
Exact old projection/wire-key and false stable-Work Session searches are empty;
`cargo fmt --check` and all-target Clippy with warnings denied pass.

### Slice 6D3 repair — completion audit

Accepted after a second completion audit found residue that still made the
broad-deletion statement false. `RegisteredTask.task` has no compatibility
property; Task values are now `parent_task`, the Wave resident receives
`resident_env`, and prompt fixtures name their repository `repo`. Stable Task
identity in the Mac terminal workspace remains `taskId`. The surviving Session
names refer only to provider continuation/configuration, usage windows,
URLSession, tmux/Ghostty/browser surfaces, human work periods, or explicit
historical migration/architecture/release text.

`WorkCensusTests.projectionAssignsLaunchIdentityOnlyToUserAttention` now builds
the real projection from a decoded roadmap plus User- and parent-routed
Launches. The emitted User-attention Launch row carries the exact Launch id and
is openable; the parent-routed Launch and every emitted Project and Task row
carry no Launch id and remain view-only. `WorkActivity` still has no action enum
or array, and the exact
`ActiveSession(s)|SessionAction|SessionSet*|SessionLane` search is empty. The
neighboring Mac action proof now follows the shared `OpenPr` reason, "checks
passed; open the PR," instead of reviving an implicit Review gate in test copy.
The Context Lab `LaunchSet` contract and DTO fixture are unchanged.

Focused repair proof: Swift Work Census passes 1/1 and Roadmap controls pass
2/2; the Rust parent-Task persistence proof passes 1/1 and prompt-document
coverage passes 9/9. Exact cited false-Session and retired-projection searches
are empty. `cargo fmt --check` and library Clippy with warnings denied pass;
the final broad gates remain the completion pass's responsibility.

## Final architecture review

Accepted with one intentionally open boundary. The implemented system now maps
its public nouns to real things: stable Work, bounded Run, provider Launch,
observed Turn, authored Steer, exact Wait, and an explicit flow Feedback
checkpoint. The independent contraction does not introduce aliases, tombstone
commands, dual readers, or a generic actor abstraction before process ownership
is known.

The architecture now distinguishes current behavior from target behavior. A
Project runner can answer child Feedback only while it is alive; there is no
Project server and no Home-owned Ready mechanism to wake a stopped parent. The
current clean-canonical-checkout gate is also named as a failure mode, not a
retained invariant. The server follow-up has deterministic done-whens for both:
durable input must wake exactly one parent Run, and a dirty checkout must not
strand read-only parent control.

At 2 a.m. the present failure is therefore diagnosable but not yet recovered:
a best-effort child nudge can fail and no durable owner notices the Ready
Project. The follow-up must make status name the exact Wait or Ready fact and
give one Home process responsibility for recovery. Implementing a speculative
Project daemon in this slice would have created a second lifecycle beside
Epoch/Run/Wait, so it remains out of scope by design.

The useful independent safety ideas from closed PR #1052 are retained in the
handoff: exact prepared-head auto-merge pinning, refusal to land across an open
authored Feedback checkpoint, completion from durable merged-PR evidence, and
guards that agree with legal actions. Its blanket managed-submit ban and
approval state are rejected because they would restore implicit blockage.

Final proof: `cargo fmt --check`, the Context Lab contract fixture, and all 37
migration tests pass. Exact searches for the deleted review, Radio, memory,
ambient-chat, evidence-receipt, escalation, policy, and false Session
projection symbols are empty. The previously recorded all-target Clippy and
focused Rust/Swift behavioral suites apply to the same source content.
