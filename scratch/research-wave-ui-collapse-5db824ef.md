# Research: Wave-only Desktop and CLI planning surfaces

## System understanding

The persistent public scope can be Wave, with Tasks directly beneath it and Chapters as its history. Projects currently occupy a real navigation and command layer, not just labels. Removing that layer requires changing the shared projections and command arguments, while preserving internal provider identity and historical attribution.

This is source research, not a rendered UI test or a live planning audit. No production files or live state changed.

### Architecture and data flow

Desktop reads typed `lf --json` responses through `RegistryQuery`; it does not own another planning database. `lf status` provides focused Wave detail, `lf roadmap` provides machine/repository-wide Work, and `lf pm show --no-sync` supplies the cached plan used when live detail fails. Rust joins PM facts with durable Work, Task delivery, and process evidence. Swift mirrors those DTOs, then projects them into navigation and views.

There are multiple reachable Desktop surfaces. The primary Podium uses `PodiumConsole` and `WorkSurfaceView`. The earlier Wave workspace remains in repository/Portfolio windows, with `WaveDetailPane` and `RoadmapView`. Updating only the older Wave pane would miss the primary navigation; updating only Podium would leave Projects visible elsewhere.

### Evidence and counterexamples

| Surface | Observed behavior | Source |
| --- | --- | --- |
| Primary navigation | Separate Wave, Project, and Task drawer columns and path segments; Project selection is committed state | `swift/LoopflowMac/Views/PodiumConsole.swift:247`, `:621` |
| Selected Work | Wave detail shows mandate, metrics, and Now Tasks; Project detail separately exposes current definition and full Task list | `swift/LoopflowMac/Views/WorkSurfaceView.swift:137`, `:209` |
| Earlier Wave workspace | Projects heading/count, Project cards, KRs, inspector, and Project-targeted chat prefill | `swift/LoopflowMac/Views/WaveDetailPane.swift:115`, `:222`, `:809` |
| Roadmap | Wave → Project → Task cards, breadcrumbs, and selectable Project focus | `swift/LoopflowMac/Views/RoadmapView.swift:220`, `:824` |
| Now | Already flat by Task condition; Project survives as a contextual label, not a necessary grouping | `swift/Loopflow/Models/NowProjection.swift:33`; `RoadmapView.swift:1115` |
| Sessions | Groups by Wave/Project; derives labels from current roadmap membership | `swift/LoopflowMac/Views/SessionsView.swift:526`, `:625` |
| Activity selection | Even Task selection supplies a Project filter alongside Wave and Task | `swift/LoopflowMac/PodiumModel.swift:377`; regression expectations in `PodiumModelTests.swift:285` |
| Metrics | Groups and owner labels keyed by Project IDs | `swift/LoopflowMac/Views/WaveDetailPane.swift:516` |
| Chat references | Project links route to a Project inspector | `swift/LoopflowMac/Views/WaveChatView.swift:48`; `ReferenceTextView.swift:206` |
| Cached plan | Separate `WavePlan.projects` projection from `lf pm show`, in addition to live status/roadmap | `swift/Loopflow/Services/RegistryQuery.swift:224`; `Models/WavePlan.swift` |

Counterexamples prevent overclaiming:

- The Project console fader is display-only (`PodiumConsole.swift:419`): `action: nil`, no independent live process. Removing it removes redundant navigation/status, not an actual worker control.
- The Swift Task-start helper requires a Project (`MacLocalWaveAgentLauncher.swift:47`, `:101`), but source search found no production caller of `startTask`. This is an API migration requirement, not evidence of a currently visible Project picker.
- The primary Wave detail already flattens Now Tasks, but that filtered list is not a full plan. Simply deleting Project detail would lose access to backlog/definition; copying the old screen is insufficient.
- Low-level Run and Work records retain Project identity as provenance. Removing it from everyday navigation does not justify rewriting historical receipts or pretending those Runs were Wave-bound.

### CLI dependencies

`lf/mod.rs:336` defines `lf wave <name>` as the foreground listener command, not a subcommand suite. `lf wave new-chapter` cannot simply be added without resolving that grammar. `bin/lf.rs:1469` dispatches that positional form. Project promotion also constructs it (`ops/project.rs:656`). Listener callers and tests must migrate together if `lf wave` becomes a suite.

The user-facing Project surface is broad:

- `lf project prepare/start/run/status/abandon/promote` (`lf/mod.rs:875`).
- `lf task start <project> <title>` (`:961`) and mandatory `lf pm task create --project` (`:1500`). PM writes explicitly error without a Project (`ops/pm.rs:1371`).
- `--project`, `--as project:...`, Work selection, activity, and usage filters. `ops/run.rs:121` resolves those bindings and assembles agent context.
- `lf ls` prints a Project count. `lf status` prints the Project hierarchy, iteration, and failures. `lf roadmap` may include Project rows (`lf/commands/waves.rs:1830`, `:1891`, `:2408`).
- `next_move_for_task` reports Project as the responsible actor for routine pending work and some PR settlement (`waves.rs:1707`). Swift renders that shared value directly. Renaming labels only in Swift would leave CLI and Desktop disagreeing.

### Shared abstractions

`WaveDetailSnapshot.projects`, `WaveRoadmap.projects`, `WaveWorkMap.projects`, `RoadmapProject`, and `ProjectPlanningSnapshot` encode the intermediate navigation layer (`lf/commands/waves.rs`; `Services/RegistryQuery.swift:490`; `Models/WaveWorkMap.swift`). `WorkNextMoveOwner.project` encodes it as an actor too.

Evidence is explicit: unavailable, empty, loading, and stale-last-good differ. `unavailable_projects` preserves stranded Tasks under historical or missing planning parents. A flatter contract must still expose affected Tasks and recovery evidence; dropping that field without replacing its behavior hides real work.

## Tensions

1. **Permanent mandate versus current ambition.** Both must appear on the Wave, with current objectives/KRs prominent and the durable mandate available. A new visible “Chapter” parent row would recreate the intermediate navigation level.
2. **Current context versus history.** Moving a Task changes its current parent, not its identity or earlier Run attribution. Activity must filter by stable Task identity across chapters; historical snapshots supply dated membership. Session labels cannot depend solely on a current-Project join that disappears during rotation.
3. **UI simplification versus data loss.** Tasks, KRs, error evidence, PR actions, and historical reports must survive flattening. Counts and labels alone are not the contract.
4. **Public language versus provider internals.** Linear still has a Project ID and may call its linked page a Project. Ordinary Loopflow commands and navigation should not require that ID or vocabulary. Diagnostics may disclose the actual external identity.

## Observations

### Complexity

Project selection propagates through drawers, breadcrumbs, inspectors, Activity, Sessions, metrics, references, and the cached-plan fallback. Shared projection changes offer more leverage than independently hiding rows. Binding/context assembly is also an agent UI: leaving Project selectors in skills would keep the coordination cost after Desktop changes.

### Quality and proof

DTO fixtures already join Rust and Swift. `PodiumStateTests` explicitly exercise Wave → Project → Task clicks; they must become Wave → Task behavioral tests. `PodiumModelTests` verify source-scoped Activity filters; add a Task-moves-chapter case that retains earlier history. Existing `WaveDetailReadingTests` and mock states cover fresh/cached/unavailable views. Some vocabulary tests inspect source text, which is insufficient evidence of navigability. `scripts/prove_wave_surface_states.sh` produces native app captures at two widths but only proves images differ, not that objectives, Tasks, and controls remain reachable.

### Potential

The flat Now projection and shared Task action model already supply the core Wave → Task experience. Keep Task workspace/PR controls intact. Use the same Rust current-chapter projection in status, roadmap, and cached plan; do not create a second Swift authority. Promote Project definition/KRs into that Wave view, then remove Project-only selection and management.

## Recommendations

### Make Wave the ordinary scope everywhere

**Observation:** a one-current-Project invariant removes the need for selection, but current arguments and DTOs still demand it.
**Cost:** coordinated CLI grammar, binding, DTO, Swift, skill, and fixture changes.
**Benefit:** one navigation and steering model, including agent prompts.
**Verdict:** necessary for the user's requested simplification; include in the same implementation design.

Use `lf wave new-chapter --wave <name> --chapter <id>`; make foreground serving explicit as `lf wave serve <name>` and migrate its callers. Preserve existing convenient `lf start/status/roadmap/chat` commands rather than reorganizing the entire CLI. Task creation takes Wave context; updates resolve the existing Task. Ordinary Project management and selectors disappear. Low-level provider IDs remain diagnostic detail, not another supported planning workflow.

Desktop selection becomes Wave or Task. Wave detail shows current chapter definition, KRs, full Tasks, metrics, and conversation. History is a view/filter by chapter, not a selectable Project parent. Sessions group by Wave; Task links remain stable. Chapter transition receipts and history use chapter language.

### Keep current reads flat and historical reads explicit

**Observation:** all main clients use Rust projections, and current arrays encode the obsolete choice.
**Cost:** coordinated DTO migration and handling unavailable Task evidence.
**Benefit:** both UIs use the same semantics; Project cannot reappear via cached plan or next-owner copy.
**Verdict:** replace current Project arrays with one chapter summary and direct Tasks. Preserve exact external Project IDs only in source/diagnostic details and historical storage. Current Task actor ownership must name Wave/Task/CI/User as appropriate, not mechanically relabel every Project event.

## Open questions and limits

- Desktop was inspected in source, not launched or visually verified. Native screenshots and interaction tests remain implementation proof.
- No live Linear portfolio was read. Migration preview remains necessary before changing actual Wave/Project state.
- Existing `WorkRef::Project` and historical Run subjects may remain internal. Removing all internal Project types would be a separate claim not needed to deliver Wave-only UI.
- Chapter rotation from a standalone CLI call and repository chapter publication have distinct completion boundaries. UI must label pending publication without making Git merge a prerequisite for preserving active Task execution.
