# Map the Current `lf` Agent Experience

## Problem

LOO-234 exists because three narrow repairs shipped on 2026-08-19—terminal
launch options survive bare `lf`, shared reads default to the current
repository, and the Mac app uses its bundled `lf`—while the overall agent
experience still felt untrustworthy.

The User asked for four things: map the seven named surfaces, preserve their
explicit **Red** judgment of the Product Discord agent, choose the gap that
matters most, and build enough internal evidence tooling to answer the question
for this observed period. This Task ends with the answered map, its receipts,
and the smallest tooling needed to reproduce the batch. It does not select a
public command, stable DTO, dashboard contract, or runtime repair.

The Product Wave objective promises that a user can understand and steer work
without caring which process owns it. Auditability sharpens that promise: every
visible claim must preserve its reason and route back to evidence. Narrow fixes
can all be green while the joined experience remains untrustworthy, so the unit
of review is the user's path across the seven surfaces, not the commit list.

## The demo

Run the task-scoped audit helper, then read one frozen evidence batch. The mixed
truth is immediate: launch options and repository scoping work; Ask has one
durable successful handoff but no reliability window; status and activity still
fail on the installed build; and Discord remains Red even though its transport
and first factual answer worked. Every conclusion names the executable, time
window, and exact receipt—or stays unknown.

## Approach

Freeze one installed-build batch, assess each named surface independently, and
keep authored judgment separate from measurements. Build only the read-only
collection and analysis helpers needed to answer this batch, following the
existing `lifecycle_scorecard.py` pattern for repository scoping and explicit
unknown evidence. Choose the highest-impact user experience gap from the map.
Do not generalize this investigation into a public evidence product.

### Evidence contract

The refreshed batch ends at `2026-08-21T17:40:26Z`. The installed CLI and signed
app are v0.12.12 at source revision
`631d623667f3dbe8e025cd0585f1ddb7d87d962f`. The build is one merged commit
behind `origin/main`; the missing commit is LOO-240's status containment repair.

- **Works:** reproduced through the installed or real configured path, with an
  observable result and executable identity.
- **Fails:** reproduced through the real path, or contradicted by an immutable
  receipt from real operation.
- **Stale:** the evidence predates the batch or no longer joins to the installed
  build.
- **Unknown:** available evidence cannot distinguish success from failure.

Mutable queue output is not a durable receipt. A measurement needs an
`observed_at`, a bounded window, and exact references to the records in that
window. An abbreviated id or an unbounded “run this now” command is a lead, not
proof.

## De-risking

| Question | Finding | Impact on design |
|---|---|---|
| Are the observations tied to the executable users are running? | Both installed `lf` binaries report v0.12.12 and revision `631d62366…`; `doctor --json` names the exact one-commit freshness gap. | Producer identity and freshness belong in every frozen batch. A merged fix does not turn the installed behavior green. |
| Does an empty Ask queue prove handoff works? | No. The durable row for `ask_30ec04624a1f4e7c9e3b0b424da576ff` does: it resolved to User answer, and its origin Turn continued until `2026-08-21T03:29:27Z`. | Assess exact Ask lifecycle receipts; use the mutable queue only for current attention. One success is not a seven-day reliability claim. |
| Can one malformed Task erase an entire view? | Yes on the installed build: focused status and activity abort, while roadmap degrades the Product Project collection to unavailable. LOO-240 has merged but is not installed. | Evidence failure must stay row-local, and delivered code must remain separate from active runtime behavior. |
| Does Discord transport health imply a good agent experience? | No. After one correct answer, the Wave published 49 more responses; 44/50 opened with process narration. | Keep authored Red judgment distinct from delivery and factual-accuracy measurements. Silence is a valid result. |
| Should the board own a new evidence store? | No existing evidence requires copied transcripts or a second planning truth. | Store stable references to existing authorities and render missing or stale sources explicitly. |

## Current evidence map

| Surface | Finding | Evidence | Boundary / near-miss |
|---|---|---|---|
| Terminal-control launch options | **Works for option preservation and provider selection.** Installed `lf -m claude` ran against an isolated fake provider, assembled `lf loopflow -m claude`, selected the Claude managed account, and exited 0. | CLI SHA-256 `543d8ec3ad5233079c81631695dc4d652be4568918dd80b491b36981edb0bf75`, source revision above, probe at the reviewed boundary; shipped PR #1200. | This did not exercise a real interactive vendor handoff. The temporary probe has no shared Exec receipt, so drill-down is incomplete even though the behavior reproduced. |
| Repository-scoped reads | **Works.** The linked Task worktree returns five Wave ids; `--all` returns 24 machine-wide Waves. | `lf ls --json` and `lf ls --all --json` at `2026-08-21T17:37:14Z`; shipped commit `3feade2ef` (#1201). | A single-repository fixture would not prove filtering. The live multi-repository ledger did. |
| Ask handoff | **Works once through requester continuation; current-build reliability is unknown.** User Ask `ask_30ec04624a1f4e7c9e3b0b424da576ff` resolved, its Ask Invocation exited successfully, and its origin Turn continued for more than six hours after settlement before ending partial. | Origin Run `run_1b8d52377e64488f8bd069f01d608b77`; origin Turn `turn_98d059b754144ba885c7cfeedc42e17f`; Ask Invocation `invocation_2896ba2ede8845a3a490e346ced26408`; current unresolved queue `[]`. | The exact proof required a raw `ask_exchanges` read because `lf ask list` exposes attention, not history. One success does not establish the seven-day N/N reliability target, and the partial origin Turn plus earlier abbreviated failure leads remain evidence debt. |
| Status and roadmap truth | **Fails on the installed build; roadmap contains the damage.** `lf status product --json` and scoped activity abort on LOO-225. Roadmap preserves Product but marks its Project collection unavailable. | Exact error: `invalid Task: merge request requires reviewer-facing PR copy`. `lf doctor --json` says revision `631d62366…` is one commit behind merged repair `6de5fd727` (#1219 / LOO-240). | Delivery and runtime are separate: LOO-240 is complete, but the installed v0.12.12 binaries do not contain it. Roadmap retains the reason but still cannot answer “what is Product doing?” |
| Run execution and child control | **Fails at two distinct boundaries.** LOO-237 still reports that an active Turn has no Run execution context. Separate Project Runs have also lacked durable child-control Basis; LOO-237 does not own that second repair. | LOO-237 Task `task_21b193b0ee3b4c70afd1dcb025dc6589`, Run `run_a185cb1e4fe2462d8d3a80fa6a38a5d7`, event `64821`; published PR #1214. Product Discord receipt `1540224185048104970` preserves the separate Basis counterexample. | A live containment, allocated worktree/PR, or repaired RunContext does not prove Project-to-child Basis. The latest recorded PR #1214 CI observation fails only `scratch-clear`, consistent with settlement not running. |
| Installed-build provenance | **Works as a structured diagnostic.** Both installed binaries are v0.12.12 at the release revision above; `doctor --json` emits a parseable provenance and freshness snapshot even though the overall audit exits 1. | CLI SHA-256 `543d8ec3ad5233079c81631695dc4d652be4568918dd80b491b36981edb0bf75`; app `lf` SHA-256 `84632c70aa7fa7ad00cf43b197413e421e869d2532a93107148fb46cc843af3c`; Developer ID signature; `doctor --json` from both binaries. | Ordinary status, roadmap, Ask, and chat receipts still omit producer provenance. A successful provenance probe does not make the separate ledger audit healthy. |
| Product Wave agent in Discord `#product` | **Fails — explicit User Red judgment, freshly corroborated.** The User asked, “What tasks have you actually shipped today? It seems like you might be stuck in a loop.” The first response correctly said zero and admitted the loop. The Wave then published 49 more responses; 44 of the 50 opened with process narration and they averaged 998 characters. | Authored Red source: Task Steer `epoch_4603a3408cc24ed89827509f21233c08:1`. User message `1540168258194243645`; bounded interval ends at `1540224884183924848` (`2026-08-21T05:03:48Z`). | Transport and factual accuracy can work while the conversation is Red. The failure is not inability to answer once; it is failure to preserve the answer as the user-visible boundary and yield. |

## Priority decision

The most important **experience gap** is operational closure: the system must
reliably carry authority through Wave → Project → Task control and advance real
work to delivery. Reporting surfaces depend on that path; a clearer UI cannot
make a stalled or unauthorized control loop work.

The current map exposes two independent proof boundaries inside that priority.
LOO-237 must carry one exact Run execution context across User, Wave, Project,
and Task runner thread changes. Project-to-child durable Basis is a separate
control contract, owned by LOO-227, and must not be claimed fixed by LOO-237.
The product outcome joins them—a Task advances and settles—but each boundary
needs its own receipt so one repair cannot hide the other failure.

Discord is a symptom and a useful observation lens, not the priority. The User
asked for shipped outcomes, received the correct answer—zero—and then received
49 more scheduler-shaped messages. That publication pattern makes
non-convergence visible: the system narrated internal motion because it was not
producing a new outcome. A future publication contract may make the surface
quieter, but suppressing narration would not repair execution.

This ordering keeps the product decision honest:

1. Settle and verify LOO-237's Run execution-context propagation without
   treating another parent Run as recovery.
2. Separately prove Project-to-child Basis at the LOO-227 boundary, then prove
   one real Wave → Project → Task path advances and settles with exact receipts.
3. Install a release containing #1219 and rerun status and activity so the
   reporting surfaces can observe operational truth without one malformed Task
   erasing the view.
4. Use Discord's narration, repetition, and outcome counts as evidence of
   convergence quality; design publication suppression only after the working
   execution boundary is established.

## Discord as symptom and lens

The Red verdict is authored judgment; the counts explain it without replacing
it.

- **Outcome buried by process:** the first answer contained the useful result,
  but it arrived behind skill narration and did not end the interaction.
- **Self-observed non-convergence:** the agent said “this was a loop, not
  delivery” and “the Wave should yield,” then continued publishing no-op phases.
- **Reactive focus:** later messages followed whichever Task or scheduler
  receipt changed, not the stable User question “what shipped?”
- **Cliche voice:** repeated openings such as “I’m using …” make internal
  orchestration the conversational subject.

Silence may be the right publication result when no user-relevant boundary
changed, but that is a downstream presentation policy. The first proof is that
the system can produce or truthfully fail to produce the boundary at all.

## Current-period audit tooling

The tool serves this investigation rather than defining a product surface. Add
one read-only helper under `scripts/` that captures the seven probes for an
explicit cutoff, records the exact executable and observation time for each,
and emits a raw machine-readable bundle. It may use the specific Tasks, Ask,
Discord interval, and installed binary paths named by this batch. Generalizing
those inputs is not a success criterion.

The helper must preserve:

- command, executable identity, observation time, exit status, and unmodified
  output for each live probe;
- bounded references for historical Ask, Run, Turn, Task-event, and Discord
  evidence;
- an independent failure result per probe so one malformed Task does not erase
  successful evidence from other surfaces;
- `unknown` when the available records do not prove the claim.

The reviewed Markdown map remains the interpretation layer. The tool collects
and checks evidence; it does not automatically convert measurements into a
single health judgment, copy transcripts into another authority, or establish
a stable wire format. Reuse the repository-scoped, read-only SQLite approach in
`scripts/lifecycle_scorecard.py` and the real-path proof style in
`scripts/prove_product_wave_surface.sh` where useful.

The earlier `ExperienceEvidenceSnapshot`, new `lf evidence` command, and Podium
rendering proposal are explicitly deferred. Project-owned KR metric identity
and fingerprinting remain LOO-235's scope.

## Success and failure test

Wild success for this Task is an honest answer, not a dashboard launch. The
current-period tool reproduces the mixed evidence without losing failures, the
map identifies operational closure as the priority, and each conclusion opens
onto the exact record that supports it. Discord helps expose non-convergence
without becoming the root-cause claim.

Wild failure is premature infrastructure or a polished second truth: a public
DTO or dashboard is designed before this period is understood, a score averages
Red conversation with green transport, stale samples look current, one invalid
Task blanks the batch, or quieter Discord output hides the same non-converging
control loop.

## Key decisions

Evidence-backed decisions:

- This Task ends at the answered current-period map, its receipts, and the
  minimal tooling needed to reproduce it.
- Reliable execution and control closure precede reporting UI work.
- Discord is a symptom and diagnostic lens on convergence, not the primary gap.
- Authored judgment and measured observations remain separate.
- Unknown and stale remain visible; neither becomes healthy zero.
- Every non-unknown claim needs an exact, resolvable reference.
- No aggregate score and no second Discord transcript.
- RunContext propagation, Project-to-child Basis, reader containment, and
  publication policy are distinct failures with distinct owners.
- Merged, installed, and observed behavior remain separate states.

Deferred questions, not implementation scope:

- Whether a future shared projection extends `status`, `roadmap`, or `activity`,
  or becomes another command.
- Whether Podium should render a durable evidence board.
- The exact future Discord publication policy once execution can reliably
  produce or fail a user-relevant boundary.

## Alternatives considered

| Approach | Tradeoff | Why not |
|---|---|---|
| Write the current-period Markdown map without a collection helper | Fast, readable, easy to annotate. | The conclusions cannot be reproduced and will drift away from their exact executable, window, and receipts. |
| Add a Mac-only dashboard fed by several subprocess calls | Strong visual scan. | It creates a Swift-owned join and leaves terminal agents unable to inspect the same truth. |
| Derive one automatic health score | Compact and sortable. | It would average hard failures, narrow successes, missing evidence, and an authored Red verdict into false precision. |
| Put assessments in Linear Project prose | Keeps prose near KRs. | Linear owns the plan, not local Run freshness or Discord/command receipts. |

## Scope

- In scope: seven agent surfaces, installed provenance, evidence freshness, the
  authored Discord Red verdict, one priority decision, and task-scoped tools
  that collect and verify this period's evidence.
- Out of scope: a public evidence command or stable DTO; Podium implementation;
  repairing LOO-225, LOO-227, or LOO-237; rebuilding the installed release to
  activate LOO-240; changing Linear KRs; designing KR metric identity; adding a
  second transcript; or treating transport health as conversation quality.

## Done when

This Task is done when the read-only helper reproduces the current-period batch
and each finding resolves to the named command, timestamped record, exact
Discord boundary, Task event, source revision, or commit—or is explicitly marked
unknown because that proof is missing. The map must preserve the mixed result
and make one priority decision without turning the investigation into product
infrastructure.

This advances Auditability's “what is this wave doing?” and “curation always
points back” KRs by naming every drop to raw evidence and binding every reviewed
claim to a receipt. It does not claim the one-week or one-month reliability
windows complete.

The tool's focused proof is:

1. It records producer identity, timestamps, exit status, raw evidence, and
   exact references for the reviewed probes.
2. One failing probe leaves unrelated evidence readable and names its reason.
3. Missing, stale, or insufficient evidence remains `unknown`.
4. Re-running against the frozen cutoff reproduces the measurements used by
   the map without contacting or mutating an external authority.

## Measure

Capture before and after against one source revision and explicit time window:

- usable `status`/`activity` subjects divided by requested subjects;
- Ask handoffs completing create → settle → requester resume, N/N over seven
  days;
- claims with resolvable references divided by non-unknown claims;
- structured provenance probes returning a receipt, N/N;
- Discord responses after the last User message, process-narrated responses,
  outcome-boundary responses, and messages after an explicit yield decision;
- Task launches whose harness starts with trace capture bound to the supervising
  Run, N/N;
- Project child-control attempts with durable Basis, N/N, measured separately.

The refreshed baseline is mixed: terminal option preservation and repository
scoping work; usable focused status/activity is 0/2 on the installed build; one
historical Ask handoff is proven but the seven-day denominator is absent;
structured provenance probes return receipts 2/2 even though the wider ledger
audit fails; LOO-237 still reports missing Run context; and the Product Wave
sent 50 responses after the latest User question, 44 process-narrated.

## Reproduction commands

These commands are read-only. They reproduce current state; historical claims
use the bounded ids and timestamps above rather than assuming a mutable queue is
unchanged.

```bash
lf doctor                           # prints revision, then fails ledger audit
lf doctor --json                    # emits a JSON snapshot, then exits 1
lf ls --json                        # 5 repository Waves
lf ls --all --json                  # 24 machine-wide Waves
lf status product --json            # exits 1 on invalid LOO-225
lf roadmap --json                   # Product survives; Projects unavailable
lf activity --task LOO-234 --json   # fails while reading LOO-225 PRs
lf ask list --user --json           # [] at the reviewed boundary
lf task status LOO-237 --json       # missing Run execution context
lf task status LOO-240 --json       # done; #1219 merged but not installed
lf pm show --no-sync --json         # LOO-233 is absorbed into LOO-240
lf chat --history -w product --limit 200 --json
lf runs --task LOO-234 --json
sqlite3 -readonly ~/.lf/loopflow.db \
  "SELECT id, origin_run_id, origin_turn_id, state, terminal_at FROM ask_exchanges WHERE id = 'ask_30ec04624a1f4e7c9e3b0b424da576ff';"
```
