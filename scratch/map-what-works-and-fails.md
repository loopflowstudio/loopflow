# Map the Current `lf` Agent Experience

## Problem

LOO-234 exists because three narrow repairs shipped on 2026-08-19—terminal
launch options survive bare `lf`, shared reads default to the current
repository, and the Mac app uses its bundled `lf`—while the overall agent
experience still felt untrustworthy.

The User asked for four things: map the seven named surfaces, preserve their
explicit **Red** judgment of the Product Discord agent, choose the gap that
matters most, and explore an internal evidence-board shape. This Task ends with
that map and priority decision. It does not select or implement a new command,
DTO, dashboard, or repair.

The Product Wave objective promises that a user can understand and steer work
without caring which process owns it. Auditability sharpens that promise: every
visible claim must preserve its reason and route back to evidence. Narrow fixes
can all be green while the joined experience remains untrustworthy, so the unit
of review is the user's path across the seven surfaces, not the commit list.

## The demo

Run the reproduction block, then read one frozen evidence batch. The mixed truth
is immediate: launch options and repository scoping work; Ask has one durable
successful handoff but no reliability window; status and activity still fail on
the installed build; and Discord remains Red even though its transport and
first factual answer worked. Every conclusion names the executable, time
window, and exact receipt—or stays unknown.

## Approach

Freeze one installed-build batch, assess each named surface independently, and
keep authored judgment separate from measurements. Choose the highest-impact
user experience gap from that map. Explore a shared evidence projection only
far enough to test whether the same map could remain live; do not turn the
exploration into implementation scope.

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

The most important **experience gap** is the Product Wave publication boundary.
The User asked for shipped outcomes, received the correct answer—zero—and then
received 49 more scheduler-shaped messages. Raw Runs should retain every phase;
Discord should publish only a new user-relevant boundary: the requested answer,
a decision, completed work, a changed blocker, or an Ask. No-op
clarify/pursue/mutate passes should be silent.

The immediate **execution blocker** is narrower: LOO-237 must carry one exact
Run execution context across User, Wave, Project, and Task runner thread changes.
PR #1214 contains that repair but remains unsettled. LOO-240 is no longer blocked:
its containment repair merged as #1219, while the installed release remains one
commit behind it. LOO-233 is complete and explicitly absorbed into LOO-240; do
not retry or rebuild it. Project-to-child durable Basis remains a separate
control problem and must not be claimed fixed by LOO-237.

This ordering keeps the product decision honest:

1. Settle LOO-237 through an authorized release path; do not treat another
   parent Run as recovery or fold child-control Basis into it.
2. Install a release containing #1219, then rerun status and activity before
   calling the containment repair active.
3. Design and validate the Discord publication/yield contract against a fresh
   User conversation.

## Discord diagnosis

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

Silence is a valid successful publication result when no user-relevant boundary
changed.

## Evidence-board candidate

The useful follow-up is a frozen evidence batch, not a synthetic Wave-health
score. `lf` should own one repository-scoped projection; the Podium may render
the same fixture using its existing `PodiumReading` states
(`loading`, `available`, `unavailable(lastGood, reason)`). The command boundary
is deliberately unsettled: extending `status`, `roadmap`, or `activity` may be
truer than adding `lf evidence`.

The smallest candidate shape follows the seven surfaces rather than inventing
KR evidence architecture:

```text
ExperienceEvidenceSnapshot
  observed_at, repo, wave
  producer { version, source_revision, binary_identity, home_id, state, reason? }
  surfaces[] {
    id, finding, state, reason
    assessment? { verdict, author, authored_at, source_ref }
    measurements[] { value, unit, window, observed_at, stale_after, verdict }
    evidence_refs[]
  }
  failures[] { subject, reason, evidence_refs[] }
```

An `EvidenceRef` points to an existing authority—Run, Invocation, Turn, Ask,
Task event, Steer, Discord message, Linear item, command receipt, or commit. It
does not copy a transcript into another store. One invalid record damages one
surface row; it does not abort the snapshot. Missing or expired samples render
`unknown`, never pass.

Project-owned KR metrics, KR identity, and fingerprinting belong to LOO-235.
They are not part of this Task's agent-experience map.

## Success and failure test

Wild success is quieter than a dashboard launch. The same frozen batch renders
in terminal and Podium; a user sees that Discord is Red while transport is up,
opens the exact authored judgment and message window, and sees no update at all
when the next scheduler pass changes nothing user-relevant.

Wild failure is a polished second truth: a score averages Red conversation with
green transport, stale samples look current, one invalid Task blanks the board,
and copied Discord prose can no longer reach its source. Suppression can also
fail by hiding a genuinely changed blocker. The publication rule therefore
filters on changed user-relevant boundaries, not on message type or lifecycle
phase alone.

## Key decisions

Evidence-backed decisions:

- This Task ends at the reviewed map and priority order.
- Authored judgment and measured observations remain separate.
- Unknown and stale remain visible; neither becomes healthy zero.
- Every non-unknown claim needs an exact, resolvable reference.
- No aggregate score and no second Discord transcript.
- RunContext propagation, Project-to-child Basis, reader containment, and
  publication policy are distinct failures with distinct owners.
- Merged, installed, and observed behavior remain separate states.

Candidate assumptions, not User-confirmed implementation scope:

- A shared `lf` projection should precede Podium rendering.
- The existing Podium unavailable/last-good pattern fits an evidence board.
- Discord should suppress no-op phase publications after the requested boundary
  has been answered.

## Alternatives considered

| Approach | Tradeoff | Why not |
|---|---|---|
| Keep a manually updated Markdown scorecard | Fast, readable, easy to annotate. | It stales immediately and cannot enforce freshness or exact references. Keep this document as the seed, not the live surface. |
| Add a Mac-only dashboard fed by several subprocess calls | Strong visual scan. | It creates a Swift-owned join and leaves terminal agents unable to inspect the same truth. |
| Derive one automatic health score | Compact and sortable. | It would average hard failures, narrow successes, missing evidence, and an authored Red verdict into false precision. |
| Put assessments in Linear Project prose | Keeps prose near KRs. | Linear owns the plan, not local Run freshness or Discord/command receipts. |

## Scope

- In scope: seven agent surfaces, installed provenance, evidence freshness, the
  authored Discord Red verdict, one priority decision, and a bounded board
  candidate.
- Out of scope: implementing the candidate; repairing LOO-225 or LOO-237;
  rebuilding the installed release to activate LOO-240; changing Linear KRs;
  designing KR metric identity; adding a second transcript; or treating
  transport health as conversation quality.

## Done when

This design-only Task is done when each finding either resolves to the named
command, timestamped record, exact Discord boundary, Task event, source revision,
or commit—or is explicitly marked unknown because that proof is missing. The
map must preserve the mixed result and make one priority decision without
turning the dashboard exploration into implementation scope.

This advances Auditability's “what is this wave doing?” and “curation always
points back” KRs by naming every drop to raw evidence and binding every reviewed
claim to a receipt. It does not claim the one-week or one-month reliability
windows complete.

If the evidence-board candidate is selected later, its provisional proof is:

1. One shared JSON fixture carries producer state, authored assessment,
   timestamped measurements, exact references, and per-row failures.
2. One invalid Task leaves unrelated rows readable and marks only affected rows
   unavailable with a reason.
3. Missing, stale, or insufficient windows render `unknown` or `collecting`.
4. CLI and Podium present the same verdict, reason, freshness, and drill target.
5. Every non-unknown row resolves at least one exact evidence reference.

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
