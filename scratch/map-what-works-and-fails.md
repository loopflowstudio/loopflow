# Map the Current `lf` Agent Experience

## Reviewed intent

LOO-234 exists because three narrow repairs shipped on 2026-08-19—terminal
launch options survive bare `lf`, shared reads default to the current
repository, and the Mac app uses its bundled `lf`—while the overall agent
experience still felt untrustworthy.

The User asked for four things: map the seven named surfaces, preserve their
explicit **Red** judgment of the Product Discord agent, choose the gap that
matters most, and explore an internal evidence-board shape. This Task ends with
that map and priority decision. It does not select or implement a new command,
DTO, dashboard, or repair.

## Evidence contract

The reviewed batch ends at `2026-08-21T03:35:49Z` (2026-08-20 local time). The
installed CLI and signed app are v0.12.10 at source revision
`011122c2d68d995cfe65dd6fd907d88f0ce28cef`.

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

## Current evidence map

| Surface | Finding | Evidence | Boundary / near-miss |
|---|---|---|---|
| Terminal-control launch options | **Works for option preservation and provider selection.** Installed `lf -m claude` ran against an isolated fake provider, assembled `lf loopflow -m claude`, selected the Claude managed account, and exited 0. | CLI hash `f708ec2e…`, revision `011122c2d…`, shipped commit `cdb1f4a3d` (#1200). | This did not exercise a real interactive vendor handoff. The temporary probe has no shared Exec receipt, so drill-down is incomplete even though the behavior reproduced. |
| Repository-scoped reads | **Works.** The linked Task worktree and canonical main returned the same five Wave ids; `--all` returned 24 machine-wide Waves. | Fresh `lf ls --json` and `lf ls --all --json` at `2026-08-21T03:35:49Z`; shipped commit `3feade2ef` (#1201). | A single-repository fixture would not prove filtering. The live multi-repository ledger did. |
| Ask handoff | **Unknown end to end; a prior attempt failed.** Earlier on 2026-08-20, two claimed User Asks remained after their answer Runs ended with unknown handback. At review time `lf ask list --user --json` returned `[]`. Neither state proves answer → requester resume. | Earlier leads `ask_d6cbb120…` and `ask_06461c65…`; current empty unresolved queue at `2026-08-21T03:35:49Z`; shipped Ask commit `156623e47` (#1175). | The earlier ids were recorded only as prefixes and `lf ask list` exposes unresolved attention, not history. The historical failure lacks an inspectable exact reference in this Task and is itself an Auditability failure. |
| Status and roadmap truth | **Fails; roadmap contains the damage.** `lf status product --json` still exits 1 and emits no snapshot because LOO-225 violates the reviewer-copy invariant. `lf activity --task LOO-234 --json` fails on the same record. `lf roadmap --json` preserves Product but marks `projects.state: unavailable` with the decoder reason. | Exact error: `invalid Task: merge request requires reviewer-facing PR copy`; Product roadmap observation at review time. LOO-240 now owns the repair. | Roadmap truthfully preserves the Wave and reason, but it still cannot answer “what is Product doing?” Status and scoped activity lose the whole requested response. |
| Run execution and child control | **Fails at two distinct boundaries.** LOO-237 currently fails before harness launch because an active Turn has no Run execution context. Its directive explicitly separates that async/thread-local propagation bug from Project-to-child durable Basis. LOO-240 fails on the same missing Run context. | LOO-237 Task `task_21b193b0ee3b4c70afd1dcb025dc6589`, event `64804`; LOO-240 Task `task_44a14d08c96e400bb4f5106967bd641e`, event `64809`; both receipts say `Loopflow active Turn authority has no Run execution context`. | A direct Task with established context, a live containment, or allocated worktree/PR identity does not prove either Project-to-child Basis or async RunContext propagation. Do not merge the two failures into one repair. |
| Installed-build provenance | **Works in human-readable diagnostics; fails as a structured receipt.** Both installed binaries identify release revision `011122c2d…`. The signed v0.12.10 app contains executable `lf` and `lfd`. `lf doctor --json` now exits 1 without a JSON snapshot because ledger audit failures abort the command. | CLI hash `f708ec2e…`; app `lf` hash `8858e48c…`; Developer ID signature; shipped commit `afc4010e8` (#1205). | Human-readable `doctor` prints provenance before failing. Ordinary status, roadmap, Ask, and chat receipts still omit producer provenance, and the JSON diagnostic path cannot currently supply it. |
| Product Wave agent in Discord `#product` | **Fails — explicit User Red judgment, freshly corroborated.** The User asked, “What tasks have you actually shipped today? It seems like you might be stuck in a loop.” The first response correctly said zero and admitted the loop. The agent then published 35 more messages through `2026-08-21T03:28:54Z`; 31 of the 36 total responses opened with process narration and they averaged 1,010 characters. | Authored Red source: Task Steer `epoch_4603a3408cc24ed89827509f21233c08:1`. Fresh User message: Discord `1540168258194243645`. Bounded interval ends at Discord `1540201000919629834`. | Transport and factual accuracy can work while the conversation is Red. The failure is not inability to answer once; it is failure to preserve the answer as the user-visible boundary and yield. |

## Priority decision

The most important **experience gap** is the Product Wave publication boundary.
The User asked for shipped outcomes, received the correct answer—zero—and then
received 35 more scheduler-shaped messages. Raw Runs should retain every phase;
Discord should publish only a new user-relevant boundary: the requested answer,
a decision, completed work, a changed blocker, or an Ask. No-op
clarify/pursue/mutate passes should be silent.

The immediate **execution blocker** is narrower: LOO-237 must carry one exact
Run execution context across User, Wave, Project, and Task runner thread changes.
It is a dependency for LOO-240, which now owns the invalid-Task containment
repair. LOO-233 is complete and explicitly absorbed into LOO-240; do not retry
or rebuild it. Project-to-child durable Basis remains a separate control problem
and must not be claimed fixed by LOO-237.

This ordering keeps the product decision honest:

1. Repair LOO-237 so repair Tasks can execute.
2. Complete LOO-240 so one invalid Task no longer destroys truth surfaces.
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

## Decisions and assumptions

Evidence-backed decisions:

- This Task ends at the reviewed map and priority order.
- Authored judgment and measured observations remain separate.
- Unknown and stale remain visible; neither becomes healthy zero.
- Every non-unknown claim needs an exact, resolvable reference.
- No aggregate score and no second Discord transcript.
- RunContext propagation, Project-to-child Basis, reader containment, and
  publication policy are distinct failures with distinct owners.

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
- Out of scope: implementing the candidate; repairing LOO-225, LOO-237, or
  LOO-240; changing Linear KRs; designing KR metric identity; adding a second
  transcript; or treating transport health as conversation quality.

## Done when

This design-only Task is done when each finding either resolves to the named
command, timestamped record, exact Discord boundary, Task event, source revision,
or commit—or is explicitly marked unknown because that proof is missing. The
map must preserve the mixed result and make one priority decision without
turning the dashboard exploration into implementation scope.

If the evidence-board candidate is selected later, its provisional proof is:

1. One shared JSON fixture carries producer state, authored assessment,
   timestamped measurements, exact references, and per-row failures.
2. One invalid Task leaves unrelated rows readable and marks only affected rows
   unavailable with a reason.
3. Missing, stale, or insufficient windows render `unknown` or `collecting`.
4. CLI and Podium present the same verdict, reason, freshness, and drill target.
5. Every non-unknown row resolves at least one exact evidence reference.

## Measures

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

The reviewed baseline is mixed: repository scoping works; status and scoped
activity abort; Ask completion is unknown; structured provenance aborts; both
RunContext-dependent repair Tasks fail before harness launch; and the Product
Wave sent 36 responses after the latest User question, 31 process-narrated.

## Reproduction commands

These commands are read-only. They reproduce current state; historical claims
use the bounded ids and timestamps above rather than assuming a mutable queue is
unchanged.

```bash
lf doctor                           # prints revision, then fails ledger audit
lf doctor --json                    # exits 1 without a structured snapshot
lf ls --json                        # 5 repository Waves
lf ls --all --json                  # 24 machine-wide Waves
lf status product --json            # exits 1 on invalid LOO-225
lf roadmap --json                   # Product survives; Projects unavailable
lf activity --task LOO-234 --json   # fails while reading LOO-225 PRs
lf ask list --user --json           # [] at the reviewed boundary
lf task status LOO-237 --json       # missing Run execution context
lf task status LOO-240 --json       # same blocker; owns reader containment
lf pm show --no-sync --json         # LOO-233 is absorbed into LOO-240
lf chat --history -w product --limit 200 --json
lf runs --task LOO-234 --json
```
