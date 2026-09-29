# Import preservation obligations

LOO-298 · 2026-09-28 · Source review consolidated; these are required assertions,
not a claim that the current importer satisfies them. Original path/line/hash
map is archived as `reviews/parallel-import-review.md`; [archive](parallel-work.md).

Exec is an actual lf process. AgentSession and FlowSession own subordinate history.
A historical Run does not establish another Exec. Preserve old selectors without
retaining Run as a competing product owner.

| Input | Destination and distinguishing proof |
| --- | --- |
| Interactive manifest/name/resolution/native reference | Stable AgentSession selector, human-title precedence, repo, caller, native/account identity; closure and failed terminal outcome remain distinct. |
| Completed keyed Ask and waiting Ask without prepared Run | Preserve exact key, request, answer, feedback and caller. Keyed retry returns the saved answer without provider launch. No invented Exec or successful turn; file mtime is inferred chronology. |
| Released Task positions and SQL captures | FlowSession graph, nested/flat cursor, return counts, version/claim/failure/review association survive. Keep unmapped historical fields until accounted for; current-API seeding is not a released upgrade. |
| SQL Session with multiple Run members | Retain every failed/replaced/successful member, ordering, outcome and attribution; current selection is not successful completion. Stable title/feedback, stale completion refusal and exact successful Flow reference. |
| Taskless position files | Import autonomous pending, failed, completed and human-review Flows, even without a Run or an active review. Preserve capture/source independence, boundary feedback, completion and failure. |
| Headless and earlier review manifests | Preserve native conversation and known Flow/node/iteration association even after cursor moves. Absent/partial membership stays unknown, not Independent; never null all membership just because old cursor differs. |
| Provider attempt/turn/usage events | Preserve account/native changes, failed/successful continuations, namespace local attempt keys by original owner. Missing, zero, partial, final and cumulative counters remain distinct; totals and coverage agree without extra Execs. |
| Terminal receipts versus SQL end | Preserve conflicting/missing evidence explicitly, including terminal write succeeding before SQL end fails. Session closure, provider completion and command exit are independent facts. |
| Mechanical operations | Flow history holds each start/outcome, including multiple Ops in one actual Exec and effect without receipt. Retain uncertainty/retry guard; no artificial agent conversation. |
| Command journal | One process can have many work boundaries; keep actual child processes separately. Completion-only input does not prove start time/exit/signal/via-agent. Preserve observed timestamp meaning and outer command result independently. |
| Native PID/start, boot, endpoint and driver/provider generations | Historical identity grants no operational authority. Imported/copied stores cannot claim a live driver/engine from old receipts. Unknown/reused PID and late generation never acquire control. |
| Work ancestry, Started and usage | Preserve original Task/Wave/source per historical member/event, existing Started timestamps and Started-only/controller evidence. Validate conflicting ancestry; done/landed and renamed identities do not require launch eligibility. Bind time is assignment, not old launch time. |

## Required failure cases

1. Session-ID existence alone cannot make import idempotent. Identical replay must
   be unchanged; conflicting answer/member/capture must fail with preserved input.
   A failed Ask/review import cannot mark its Run claimed and suppress later import.
2. No launch resolver errors converted to null Work. Validate separately recorded
   Task and Wave, renamed Wave identity and done/landed Task attribution.
3. Interrupt between capture creation, review reservation and member import;
   rerun preserves original IDs, feedback and complete history without duplicates.
4. One old process with several agent/mechanical Runs maps to one Exec plus history.
   A Run lacking process evidence retains that missingness. No synthesized exact
   start from earliest completion event and no fabricated successful outcome.
5. Prove pre-bind/post-bind/active-turn history and usage according to recorded
   evidence. Existing bulk-bound bytes cannot establish their original owners.
   Preserve `tasks.started_at` even when original Runs were unbound; import must
   use the existing Started fact, not infer it solely from Run inventory.
6. After successful import, make every retired product-state input unavailable
   in the fixture. Public list/detail/rename/keyed retry/Flow recovery still work
   and recreate no mutable sidecars. Keep immutable payload/native history.

## Migration and proof order

Inventory released SQL, filesystem evidence and already-applied development
frontiers separately. Reconcile exact identity/conflicts; label inferred timestamps,
unknown membership/process and unavailable payload. Preserve all captures and
conversation histories before replacing references and switching readers.

Source audit distinguishes the starting schemas: released `0.12.15` already drops
the former `runs` table; released `0.12.20` introduces `task_flow_positions`, and
the released frontier has no `sessions` table. SQL Sessions with multiple Run
members belong to this branch's applied development drafts. Prove released Task
positions plus filesystem origins separately from populated pre-admission SQL
members through the forward draft. A final-schema/current-API seed proves neither
upgrade. Verified direction: Task comment `e6276089-bbf2-4aea-a171-e4fe3d5cf7a9`.
Evidence previously discarded by a released migration cannot be silently recreated.

The 2026-09-29 populated proofs now distinguish those frontiers. The released
position fixture exposed an autonomous Task losing Started despite its retained
Started event (`released-started-evidence-red.log`). The forward draft restores
assignment presence at explicitly inferred conversion time; the old event keeps
its timestamp. Source proof passes; `materialized-sql-upgrade.log` also passes
the released-position and admission cases while failing development-prefix
adoption. The retained NULL-title review seed fails, but Supervisor's released-writer
audit found no supported path producing that combination. Preserve its synthetic
red receipt separately from the corrected populated fixture; no runtime title
fallback or applied-migration rewrite follows. See
`.lf/tmp/historical-title-review/classification.md`. Broader import proof remains owed.

`materialized-sql-upgrade-rollback.log` passes four focused cases after the
adoption repair: populated pre-admission members, retained Projects/unreleased
drafts, and changed-evidence rejection. The populated case retains native events,
exact Flow references, title/feedback/caller, repeated SQL-member import and an
existing Started value of 17. An injected schema discrepancy after pending SQL
is applied rolls back schema, canonical/development ledgers and history. Inspected
bytes match `canonical-sql-upgrade-final-source.json`. This is disposable,
materialized SQL evidence; unmapped SQL inputs, filesystem origins, public
recovery, terminal/SQL disagreement and the remaining rows above stay owed.

Prove released populated schema plus four filesystem origins, headless/provider
history and command journal through final canonical schema and public CLI. Include
controller-only progress, nested/flat captures, runtime loop children, repeated
attempts, stale completion and exact once-only Flow consumption. SQLite foreign
keys must hold at the end; no partial count authorizes dropping unresolved inputs.

Development Homes need forward drafts over applied checksums; never edit history
to fit the current constructor. Materialize drafts only in disposable exact-source
copies using migration test helpers, not direct draft includes. Copied-Home proof
must clear live authority while preserving historical identity. No installed Home
conversion or branch promotion is authorized. Final real-Home procedure needs backup,
rehearsal, exact old-writer quiescence, preservation report and separate authority.
