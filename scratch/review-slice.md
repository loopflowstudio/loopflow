# Slice review — Wave chapters

2026-09-24. Disposition: advances the full design, but does not yet satisfy the
clarified ownership contract. Do not approve or publish as complete. Keep the
remaining work in this PR, as specified by `projects.md`.

## Findings

1. **P1 — metric targets still belong to the Wave.**
   `controller/wave/metrics.rs` stores `target` on `MetricContractDefinition`
   and `MetricContract`, includes it in the revision hash, and evaluates from
   that contract. `ops/metrics.rs::wave_metric_portfolio` reads Wave contracts
   without resolving chapter targets. `pm/mod.rs::ProjectContent` has only
   definition, flows, and KRs. Rotation therefore cannot author a new target,
   unset an omitted target, or preserve chapter-specific target/verdict history.
   Existing metric tests prove the superseded ownership, not the new contract.

2. **P1 — chapter authoring still introduces an independent objective.**
   `engine/builtins/wave/skill/wave_start-chapter.md` asks for fresh objectives
   and emits a Project `definition`; `WaveDetailPane.swift::WaveChapterView`
   displays it as chapter objectives. Native fixture captures visibly show
   both Wave goal and chapter definition. The human assigned the objective to
   the Wave. The authoring instructions, content model, and both UI projections
   must agree on that single authority.

3. **Fixed — redundant public Project creation writer.**
   Removed unused `ops/pm.rs::pm_create_project`, its async implementation and
   `LocalProject`, plus unused `pm_resolve_project`/`PmResolvedProject`. The
   creation path bypassed the durable chapter identity and current binding.
   Searches found no callers. Provider/local creation primitives remain for
   the chapter writer; historical storage remains for provenance.

4. **Fixed — historical Project inspection was rejected by CLI grammar.**
   Restored `lf work status project <id>` while leaving Project enable,
   disable, interrupt, and abandon unavailable. The existing read path already
   supports Project provenance. A focused parser test proves this distinction.

## Evidence matrix

| Claim | Planned behavior | Implemented behavior | Proof | Result |
|---|---|---|---|---|
| Ownership | Wave objective/instruments; chapter Tasks/KRs/targets | Tasks/KRs correct; targets and objective authoring remain wrong | Findings 1–2; inspected native captures | Gap |
| Target reset/history | Change target without changing instrument; omission unsets; old dated verdict survives | No chapter target representation or evaluation snapshot | `ProjectContent`, metric composition, chapter receipt source | Gap |
| Provisioning and one current chapter | One binding; fixed successor identity; no duplicate on retry | Durable binding and pending receipt; provider UUID reconciled | Fresh chapter suite, including SQLite cutover and local HTTP lifecycle | Pass locally |
| Deterministic preview/apply | Read-only preview, refresh evidence on apply, resumable lost responses | Shared classifier and transition; retries reuse identity | HTTP simulation covers lost create/move/cancel replies and repeated application | Pass locally; real CLI rotations not demonstrated |
| Task dispositions | Started unfinished carry; untouched cancel; terminal remain historical; ambiguity never auto-closes | Structured state and durable execution evidence drive classification | Classifier and HTTP simulation, including a start after preview | Pass locally |
| Task continuity | Preserve claim, Flow, worktree, PR and identity; stale saves cannot undo transfer | Dedicated parent transfer; ordinary updates omit parent | Fresh store transfer test | Pass locally |
| First-start race | Retirement cannot discard a Task that already began | Durable first-start evidence and claim boundary | Fresh retirement/first-claim test; source inspection | Pass locally |
| Historical task evidence | A moved Task remains in old chapter without later completion credit | Boundary receipt freezes content/membership/evidence | HTTP lifecycle completes moved Task later and rereads history | Pass locally |
| Chapter skills | One Wave proposal/review; deterministic API owns dispositions | Project delegation removed; preview/apply/history commands taught | Builtin registration check and source inspection | Structural pass; ownership text wrong; authored skill runs not demonstrated |
| Wave-only ordinary UI | Both workspaces show Wave → Task and full backlog | Flat task navigation, chapter KRs, history and metrics on Wave | Fresh native fixture captures in `evidence/review-slice/` | Rendering pass; live interactions not demonstrated |
| Secondary UI continuity | Selection, Sessions, Activity, references and cache survive chapter change | Shared projections and source-reference routing | Prior recorded Swift checks; reviewed source | Prior evidence only; no new live rotation proof |
| Historical diagnostics | Read old Project Work without reopening planning | Status grammar restored; mutation selectors remain Wave/Task | Fresh `historical_projects_are_inspectable_but_not_controllable` test | Pass |
| Removed planning authority | No independent Project operator, launch tier or ordinary creator | Deleted skills/commands/writers; chapter creation remains | Negative source searches; bounded deletion above | Pass for inspected paths |
| Future Wave hierarchy | Preserve ancestry; no hierarchy expansion now | Existing `parent_wave_id`, ancestry resolution and history retained | Wave model and PM ancestry source; prior historical checks | Preserved; future behavior not claimed |

## Proof and limits

- `cargo test -p loopflow --lib chapter`: 9 passed. Exercises production
  operations against a local HTTP provider and SQLite. It is not a live Linear
  or real CLI end-to-end demonstration.
- `cargo test -p loopflow --lib historical_projects_are_inspectable`: 1 passed.
- `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings`: passed.
  The first lint pass caught imports left behind by deleting the unused writer;
  moved the test-only imports into the test module and reran successfully.
  Fresh test/lint logs are saved alongside the native captures.
- Inspected fresh native captures `wave-900.png` and `portfolio-900.png` in
  `scratch/evidence/review-slice/`. Both expose Tasks directly under Waves.
  The unbundled executable reports its missing installed `lf` helper; captures
  prove fixture rendering, not live controls, click-through or rotation.
- Earlier Rust/Swift/DTO evidence is preserved in `review-wave-chapters.md`.
  Those checks were not rerun merely because a review phase began.
- Exact Git patch coverage remains unproven: this generic worktree has no
  Task binding (`lf task diff projects --json` reports no Task). Reviewed a
  filesystem comparison with `/Users/jack/src/loopflow` plus focused source
  reads. That comparison can include sibling drift and is not an authoritative
  branch diff. No raw Git was used to bypass Loopflow.
- No live PM changes, push, or PR publication occurred.

## Next coherent slice

Move target authorship into Project content and carry it through plan parsing,
preview/apply/update, history, Rust/Swift wire fixtures, and both Wave views.
Evaluate the persistent Wave instrument against the selected chapter target;
freeze dated historical targets/verdicts and remove the Wave-target fallback.
Keep observation identity/history independent of target changes. Make chapter
skills author KRs and targets against the one Wave objective; remove the second
objective from the model and UI rather than merely relabeling it.

Focused proof: rotate twice with different targets for the same instrument,
then omit the target. Confirm unchanged Wave objective/instrument, the new
target or explicit absence, preserved old target/verdict, and continuous Task
identity. Follow with installed CLI/native interaction evidence for the
remaining demo claims. Wave hierarchy remains a later design, not another
implementation requirement for this slice.
