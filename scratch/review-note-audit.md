# Human review → implementation audit

2026-09-25. Jack asked to review all scratch documents so his review notes reach
implementation. This is the current handoff; the accepted product design remains
[main-view-task.md](main-view-task.md). Dated receipts retain their original
source identities and limitations. A prototype interaction is not native proof.

## Result

The main visual direction reached native implementation. Several details were
lost between slice handoffs, and stale notes were still advertising resolved
questions. The concrete remaining items below must survive every subsequent
implement/compress/review pass. They are not optional polish inferred from taste.

| Human decision / source | Current implementation and evidence | Disposition |
| --- | --- | --- |
| A, calmer cream/burgundy workspace; connected repo header and sidebar; bottom Search | WorkspaceNavigator and cycle-03 native captures | Implemented locally; final visual acceptance open |
| Started Tasks only in sidebar; Wave contains full plan; 20 started across Waves and 50 per Wave | WorkspaceProjection shared `runtime.started`; `startedWorkingSet`, `denseWorkingSet`; real CLI preparation-versus-execution proof | Implemented; do not restore process-dependent membership or All tasks sidebar |
| Wave objective and Current KRs, no public Project | WorkSurfaceView Wave detail and shared chapter projection; cycle-03 | Implemented; configured chapter/Home readiness still required |
| Task title once; issue ID in breadcrumb links to Linear; Description | WorkSurfaceView, WorkspaceBreadcrumbBar, MarkdownBlocks; cycle-03 and latest direction D | Implemented; D now uses serif page titles, superseding the earlier sans choice |
| Remove ELSEWHERE, Local work evidence, snapshot/internal preview copy | Reachable native view search and cycle-03 | Implemented; keep truthful read failures separate from ordinary content |
| Better Description authoring; dated logs belong in comments | Builtin design/prompt/launch-plan/task-pursue/chapter skills retain the changes from task-description-prompts.md | Implemented in prompts; existing Linear prose deliberately unchanged |
| Collapsed Comments with actual count below Description | Real paginated shared read, TaskCommentsView; cycle-05 implement/compress and active review | Implemented with local native/HTTP proof; configured Linear/app read remains |
| C connected Flow; literal mono skills; blue regions/arrows/running, yellow humans, green completed humans, red blockers, no You header | TaskFlowView and cycle-04 native fixture captures | Implemented locally; inspect actual expanded Feature, not only synthetic fixture |
| Searchable Flow name; Start; status between Flow and Description; hover/focus Stop & restart with confirmation | TaskFlowView, shared task_flow controls/catalogue/restart validation; cycle-04 | Implemented locally; real successful configured controls remain unproven |
| Two loops, demo between deciders, final Advance → queue → land | Actual engine topology and native graph projection; cycle-04 | Retained; final two-loop composition discussion remains after build |
| Iteration is a tuple, each loop counts its own returns | parent-iteration-tuple.md; shared projection, capture, DTOs and native header | Implemented; seven Rust, one CLI and five Swift checks pass; independent review remains |
| Pause not required; absent worker shows stopped at saved step with Resume | cycle-04-compress removes unsupported Pause from contract and view | Implemented locally; older Pause prototypes are superseded |
| Recent Runs expand on demand (Task 2) | TaskFlowView shows current state; Task detail has no historical Run disclosure | **Missing: carry into implementation after Comments** |
| New session beside title, independent Task conversation even before Flow | WorkSurfaceView → existing Task preparation/launch; cycle-03 | Implemented; delayed app preparation needs behavioral proof; Task-keyed error/busy feedback now has [focused proof](implementation-cycle/parent-session-launch-feedback.md); actual subprocess preparation remains unproven |
| One Session opens directly; multiple Sessions named in Task; drill down Wave → Task → Session, no Description there | SessionsView, breadcrumb, native named-session/ancestor proofs | Implemented locally |
| Multiple Session rows include provider and useful summary | Rows currently render title, membership and state only | **Missing: project/render available real data; never invent a transcript excerpt** |
| Skill seed else historical generator, editable name; human rename wins | Run-owned naming, shared rename/suggest, operating guidance; cycles 1–2 CLI/race/native proofs | Implemented; historical generator was magical-musical, with no animal list |
| Distinguish exact Flow membership from independent/unknown/historical | Shared membership/occurrence plus parent structural node-key correction and nested CLI/DTO proof | Implemented locally; tuple update must preserve exact boundary identity |
| Explicit conversation-pane focus updates breadcrumb and Rename; companion preserves context | SessionsView.focusPane; parent-session-focus.md; final cycle-04 and compress native proofs | Implemented; stale concept-review “unimplemented” statement no longer governs |
| Membership chip highlights exact graph node (accepted concept usage) | Shared structural `node` exists; breadcrumb membership remains plain Text | **Data foundation only; click/reveal/highlight interaction missing** |
| Retain drafts, viewport, focus, splits, companions; no implicit transfer | Existing retained workspace/surface owners, native PTY regressions and stream receipts | Local proof; current configured vendor/demo acceptance remains open |
| Exact current Linear data for Loopflow, Etude, Kata; align dev lf/Home before demo | Prior snapshot/demo-restore receipts; fresh installed roadmap still reports missing chapters | **Recovery candidate now passes retained-Home preflight on a copy; independent review and promotion/readback remain** |

## Implementation order and ownership

1. Tuple correction is implemented under the conditional parent takeover in
   audit-coordination.md; parent-iteration-tuple.md records focused shared,
   capture, stale-boundary, CLI and native proof. Review with the next slice.
2. Cycle-04 review is complete. Native direction D now follows Jack's recorded
   "left pane A, center B, vibes C" decision, including serif page titles;
   visual-study/polish/native/README.md supplies its scoped 49-test receipt.
   Actual 13-node Feature at 1100pt and configured composition remain unproven.
3. Comments implement/compress are complete and review is active. Preserve its
   genuine last-good/error/empty distinctions; configured readback remains.
4. Complete the Task evidence/details slice: demand-loaded recent Runs;
   provider/useful Session summary; exact membership-chip graph navigation;
   preserve the parent Task-keyed New-session feedback correction and add real delayed preparation proof.
   Reuse existing readers/owners and preserve earlier/past/unknown membership.
   If a historical graph is unavailable, disclose that rather than highlight a
   same-named node in the current invocation.
5. Verify the aligned app/CLI/Home and real chapter reads, build both native
   paths, then present the new composition. Final human discussion, configured
   input/retention, measurements and external acceptance remain outstanding.

## Contradictions resolved

- The earlier one-loop prototype, single-current-Session constraint, separate
  Session-with-context depth and mandatory Pause are superseded. Do not revive
  them from old studies or test counts.
- Naming/generator recovery and Flow membership are implemented, not open
  investigations. Direct ordinary Flow invocations can have exact Step
  membership; the older “all direct flows are independent” assumption is stale.
- Shared Session legal actions, chapter ownership and streaming discovery were
  integrated. Earlier “absent” or “manual Refresh only” reports describe their
  dated source, not current missing implementation.
- The nested-node and native-focus findings have fixes/proofs. SessionFixture
  now uses Complete; its empty planning fixture still leaves the Flow Session
  unmatched. A proposed Task-row lookup change is not justified by Task Work
  alone. The hosted fixture test still needs its own proof.
- Follow-up UX forks (LOO-297) and resident Wave interpreter removal are separate
  work. Their scratch documents do not authorize folding those projects into
  this native UI slice.
- Existing ten external-work trials, authorized Description edit/readback,
  twenty long-lived-registry trials and published performance budgets survive
  redesign. Capture/OCR/PTY measurements do not prove compositor latency/hitches.
  Optimization follow-ups remain deferred; no new PM items are needed here.

## Audit scope and limits

The [inventory](review-note-audit-evidence/inventory.json) covers every scratch
Markdown document and text archive at the final scan, with content hashes and
headings. All were scanned for human decisions, supersession, required outcomes
and remaining work; current design/study/concept/cycle documents and relevant
historical passages were read against reachable source and their receipts.
Raw logs, screenshots and generated fixtures are evidence attachments, not
additional design authorities; this audit does not claim to have rerun or
visually inspected all of them. Source paths behind the matrix are captured in
the same inventory. Writers are active, so hashes identify an observation, not
a frozen branch or new gate pass.

Changed current handoffs and implementation briefs to retain these items;
historical archives and accepted prototype sources stay intact. No product
tests were rerun for this documentation audit. No publication, PM write,
installation, live Session action or Task completion occurred.
