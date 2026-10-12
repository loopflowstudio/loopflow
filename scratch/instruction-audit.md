# Instruction allocation audit

2026-10-11. Allocation evidence for [the design](levels-of-lf.md).
All sections of the previous LOOPFLOW.md were allocated below. Built-in skill
inventory and Work-related passages were surveyed across every bundled skill;
portable paths were corrected while explicitly selected Work procedures retain
their authority. This is an instruction allocation, not a general correctness
audit of all Work operations.

Destinations: **Base** applies to all launches; **Work** applies through work
APIs; **Skill** applies only when that procedure is selected; **Repo** belongs
to repository policy. Desktop and custom clients use the same destinations.

## Shared LOOPFLOW.md: every current behavior

| Current instruction | Proposed destination / change |
| --- | --- |
| lf owns git, worktrees, delegation, release | Base follows user/repo tool choices; Work owns managed mutations |
| Wave/Task/Session/Flow/Process definitions | Base: Session and taskless Flow; Work: Wave, Project, Task, Workflow; Process only where needed |
| Execute here; don't relaunch merely to do assigned work | Base |
| Always manage worktrees with lf | Base preserves assigned checkout; Work uses lf for managed mutations |
| Orchestrate only when requested/selected; don't guess Wave or repair PM | Base boundary, expressed briefly |
| Commit/sync/publish/submit/arm/land/reconcile command table | Relevant delivery skills and discoverable command help; don't pay for full table every launch |
| Publication vs merge, scratch cleanup, delivery recovery | Delivery skills; preserve taskless delivery |
| One Task/PR, Task completion and follow-ups | Work semantics; completion procedure in selected skill |
| Flow completion does not complete Task | Work; Base says a Flow ends at its authored boundary |
| Preserve dirty contributions, checkpoint coherent edits | Base or Repo for general rule; selected tool details in delivery skill |
| No permission for reversible edits/tests | General agent behavior; avoid repeating provider policy without a Loopflow-specific need |
| Implement/compress/gate/CI verification cadence | Respective skills and Repo, not universal launcher policy |
| Headless checks, one-line scratch evidence | Relevant engineering skills; present/headless interaction belongs to Surface |
| Delegate independent subsets | Selected procedure or user/repo preference; no universal orchestration directive |
| Follow supplied Flow | Base execution contract, independent of Tasks |
| Session naming and lf rename | Base, concise; bound Task title behavior in Work |
| Names and attribution in persisted artifacts | Shared writing policy only if deliberately retained; keep source identity protection |
| Answer in current conversation; never open another to reach user | Surface, all tiers |
| Headless missing input stops with ordinary output | Surface, all tiers |
| Wave operator reads failure logs and resolves it | Operator skill |
| Inspect execution/effects before more work; old reviews grant no launch authority | Work operations; Flow retry semantics where exercised |
| wave list/status/roadmap are shared state owners | Work |
| Repository/Wave/Task chat is automatically its operator | Remove universal rule; selected operator/session skills own this behavior |
| Flows/schedules continue; conversations aren't reinvoked | Work/operator skills; lower-level Session liveness explained where needed |
| No GUI browser capture; secret policy | Repo or relevant capture skill, not Loopflow's universal operating contract |
| Local notes vs Linear comments, progress markers, steers | Work; generic local note practice in engineering skills |
| lf context budgets and curation | Context-authoring skills; lower-level capability, not Task prerequisite |
| Read repo guide and existing design | Relevant engineering skills/provider-native repo guidance |
| Scratch context visibility and cross-checkout limits | Base context mechanics, concise |
| Design paths, question paths, durable conclusions before cleanup | Design/delivery skills |
| Skills vs repo guide vs config ownership | Prompt-authoring/init skills |
| Wave GOAL/MEMORY editing, children, realign | Read-only Work context; writes belong to selected Flow procedures |
| No miscellaneous handoffs or maintainer rules in customer skills | Repo contributor policy/prompt-authoring skill |

## Sources that can undo the split

| Source | Evidence and disposition |
| --- | --- |
| `ops/skill/init.md` | Says launcher use is secondary; initial inspection includes Wave/PM. Make onboarding follow the chosen capability before inspecting its dependencies. |
| `ops/skill/default.md` | Recommends repo operation when work is substantial. Complexity alone must not enroll someone in Work. |
| `task/skill/advance.md` | Approved unbound design leads to Wave lookup, Task creation and Flow launch. Keep ordinary continuation possible without this promotion. |
| `task/skill/design.md` | Mandatory Placement phase despite optional Wave orientation. Make Work placement conditional; standalone design needs no unresolved-Wave boilerplate. |
| `task/skill/kickoff.md` | Task framing in generally useful planning. Separate required problem/design inputs from optional Work context. |
| `task/skill/implement.md` | General implementation plus Task/Wave sections and metric proposals. Keep ordinary implementation standalone; condition Work behavior on selected scope. |
| `task/skill/gate.md` | Verification is portable; Wave instrument proposals are not universal. |
| `task/skill/realign.md` | Already accepts unbound work; preserve that while reviewing memory curation and context-budget obligations. |
| `task/skill/ci-fix.md`, `ops/skill/pr-{land,submit,publish,message}.md` | Delivery is lower-level. Separate bound Task consequences and follow-up filing from PR mechanics. Submit and land currently describe different completion obligations; reconcile with actual owner rather than copying both. |
| `task/skill/ship-decomposed.md` | Requires a Task per PR and Task checkout stacking. Decide how lower-level multi-PR delivery uses the chosen APIs without covert enrollment. |
| `task/skill/refine.md` | Parent reviewer path assumes a Task review protocol. Keep text refinement available without Work. |
| `builtins/surfaces/{human-present,headless}.md` | Both prescribe `--task` for another perspective. Surface should describe conversation behavior; Work delegation belongs to selected work procedure. |
| `builtins.rs` | Describes one universal operating document; update embedding and attribution. |
| `prompt/process.rs`, `prompt/mod.rs` | Boolean guidance selection, document deduplication, system assembly; replace monolithic assumption. |
| `lf/commands/run.rs` | Explicit Task seed, ambient Wave resolution, continuation and source attribution need consistent selection. |
| `harness/context.rs`, `context_block.rs`, context tests | Preserve delivery across providers and turns; inspect paths for stale full guidance on resume. |

## Other instruction owners

Directory names (`task`, `project`, `ops`) do not select a tier. The Work-related
clauses in these groups were reviewed against their entry conditions.

**Portable procedures:** commit-message, loopflow-validate, pr-review,
release-notes, release-run, sync-conflicts, token-compress, xor-route, polish,
reduce, testing-audit, 5whys, compress, concept-review, debug, demo, loop-or-next,
prompt, qa, research, review-design, triage, unbreak. Their Work-aware clauses
are conditional on supplied scope or discuss the artifact being authored.

**Explicit Work procedures:** repo-operate, repo-session, review-chapter,
start-chapter, capture-tasks, task-operate, task-session, s1–s5,
split-wave, wave-report, wave-operate, wave-review-chapter, wave-session,
wave-start-chapter. These procedures remain selected, not implied by Work attribution. Their
operating methods remain in the skills rather than moving into tier two.

**Flows and Workflows:** inspected bundled definitions and generated checklists.
`pursue` and `ship` are callable Flows; their storage under `task/` must not
imply Task enrollment. A selected Task Workflow legitimately supplies its
edges/review points. Preserve that explicit selection across either UI. The
`incident` Flow reaches `launch-plan`; that skill now keeps unbound work local
unless managed work was explicitly requested.

**Other instruction producers:** reviewed work-context rendering, shared launch
and resume assembly, skill composition/export and CLI help. Exported Flow
checklists now explicitly allow taskless use. The external `skills/loopflow`
contract now points to `lf help skill work` instead of mirroring all Work doctrine.
No API response envelope or Desktop-specific instruction selector was added.
Repository AGENTS.md is contributor policy, not a document to inject into
other repositories. User-authored instructions remain user-owned.

## Review findings

Two documents alone cannot satisfy the intended experience: onboarding and
portable skills would still promote launcher users. Conversely, putting every
operator procedure into the Work document would take control from custom
clients. The split must distinguish API semantics from selected procedures.

The CLI smoke review found `lf help work` resolves to `lf monitor work`.
All guidance uses `lf help skill work`; the regression test compares that
help body with the preloaded contract.

## Repository ownership refinement

Jack's 2026-10-11 refinement keeps `wave/` maintenance inside selected Loopflow
Flows. Work association alone cannot enable it. Work, implement, realign,
Wave operation, S1–S5 and split-wave guidance now name that boundary; standalone
procedures retain proposals in scratch. This is agent instruction scope, not a
filesystem sandbox or a ban on explicitly requested Wave CRUD APIs.

2026-10-12 — Jack rejected the repository-wide scratch opt-in. Replacement:
cleanup selects added scratch paths from the Git diff; pre-existing paths and
edits survive. Delete the flag, global directory removal/untracking and blanket
scratch merge restoration. Skills retain their usual writing paths. No manifest
or reserved directory. `lf pr submit` and `lf pr land` invoke the same cleanup operation exposed as
`lf scratch clear`; default Flows need no extra step. Ordinary commits and PR
open/publish keep scratch in every checkout; automatic Markdown context inclusion stays unchanged.

Broader audit found onboarding unnecessarily asked users to classify themselves
as launcher/Desktop/custom-client users. `init` now infers the needed capability
and defaults to getting a plain launch working. Existing Work continues without
another opt-in. Automatic WorkContext selection, explicit-skill deduplication,
read-only provenance and optional evaluation tooling add no setup requirements
and remain. The accepted Flow-only Wave-maintenance boundary remains; no new
filesystem access controls are added. Existing scratch may be edited as needed;
cleanup authority is narrower than ordinary editing authority.

Instruction provenance now uses the resolved Task/Wave association directly.
Opt-out takes precedence over deduplication; selecting a skill still records its
actual inclusion independently. `--instructions` reads capture decisions rather
than reinterpreting current checkout state. Historical gaps remain gaps. Review
also corrected a steer test's assumption that every decision without an asset
was a steer. Decision fixtures reuse production formatting and keep expected
actions out of exported model inputs; scoring tests are not model evaluations.
