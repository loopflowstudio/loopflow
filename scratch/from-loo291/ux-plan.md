# UX plan — information architecture, visual style, snappiness

2026-09-26. Written from Jack's demo verdict
([demo-native-workspace.md](demo-native-workspace.md)), the
[prototyping retro](retro/prototyping-retro.md), the
[state of the art](retro/state-of-the-art.md) and the
[performance study](retro/frontend-performance.md). Supersedes the
"implemented locally, awaiting acceptance" framing in
[review-note-audit.md](review-note-audit.md): nothing in the native build is
accepted until Jack accepts a slice below on the installed app.
[main-view-task.md](main-view-task.md) still governs product behavior; this
plan governs presentation and its execution order.

## Decisions carried in (Jack's, dated 2026-09-26)

- Light mode only; dark is a follow-up.
- Serif for the repo name and page titles (Wave, Task). Sidebar Wave rows are
  sans; no glyph, no count, no dot on Wave rows.
- Orphan Sessions leave the Wave tree: a section at the bottom beside search,
  headed "Orphan sessions" (or similar), whose header opens a control-room
  multiplexer of all orphans.
- Converge from both sides: `lf` launches in a Task worktree bind the Session
  to that Task; where a Session can be launched from in the UI, and what each
  launch point infers, is design work in this plan. Waveless first run is
  deferred.
- Session chrome: one tool set, progressively disclosed; the right pane must
  join the left visually and must not get loud.

## Diagnosis in one screen

Structure landed; tone didn't. Every dimension in the native build matches
direction D. What reads wrong is (1) neutral ink and grey halos where the
mock had warm ink and hairlines, (2) components from four different passes
sharing one sidebar with no reconciliation, (3) chrome that was never in any
mock (system buttons, rose tab strip, blue-grey pane header, green Complete
capsule, July's `WaveLensView`), and (4) a type ramp with two screens of
hierarchy where one is enough.

## Slices

Each slice names its owner files so Runs can work concurrently without
colliding. Proof for every visual slice: a native capture of the installed
build at 1440×900 and 1100×800 on live data, placed beside the mock, plus the
focused Swift tests that already mount that surface. Test counts are not
acceptance; Jack looking at the installed app is.

### S1 — Tokens (foundation, serial, first)

Owner: `swift/Loopflow/Design/BrandColors.swift`, `DesignSystem.swift`,
`swift/LoopflowMac/Design/WorkspaceStyle.swift`, `AppBootstrap.swift` (fonts).

- Warm light ramp from the state-of-the-art baseline §8: canvas `#FAF8F5`,
  panel `#F3EEE7`, element `#ECE6DD`, surface `#FFFDF9`, hairline `#E6DFD6`,
  border `#D6CCC0`, active `#C2B5A6`, accent `#722F37` / hover `#5E2630`,
  muted `#7A716A`, text `#2A2624`, secondary `#5F5752`. `borderStrong` and
  `textTertiary` become hexes, not opacities.
- Surface ladder, not shadows: delete the `workspacePanel` blur; hairline
  only. Shadows survive only on popovers.
- Type ramp, seven steps: Cormorant 34 / 26 / 17 (display, title, lede);
  Lato 13 body, 13 bold, 11 caption, 11 caps label; JetBrains Mono 12.
  Register every Lato weight the code uses (or stop using `.semibold`);
  remove the two `.system(size:14)` fallbacks.
- State colours unchanged; rule: blue means running and loop region, nothing
  else. Motion tokens 100/160/240ms; reduce-motion zeroes them.
- Proof: contrast check ≥4.5:1 for text on each surface step (script the
  check); one capture of Wave and Task pages before/after.

### S2 — Sidebar (parallel with S3 after S1)

Owner: `WorkspaceNavigator.swift`, `WorkspaceProjection.swift` (outline
only), `WaveLensView` usage in the navigator, navigator tests.

- One row component, two levels. Wave: sans 13/600 with chevron, no glyph.
  Task: 13/400, 6pt dot only when running / blocked / needs-you, count
  accessory only when >0. Row 28. Delete `WaveLensView` from the navigator.
- Orphan Sessions: a collapsed section at the bottom, above search, headed
  with a count ("Orphan sessions · 4"), one-line rows, no ancestry subtitle.
  Sessions whose worktree branch matches a Task fold under that Task (this is
  the UI half of "both sides"; S5 is the lf half). Header click navigates to
  the control room (S6 renders it; until then, the header selects the first
  orphan).
- Search stays at the bottom as a filter.
- Proof: outline tests updated for the new grouping; capture at both widths
  with the 20-started/50-per-Wave density fixture and with live data.

### S3 — Session chrome (parallel with S2 after S1)

Owner: `SessionsView.swift` (`WorktreeNodeView`, `SessionPaneView`,
toolbar), `WorkspaceBreadcrumbBar.swift`, `TaskWorkspaceView.swift`.

- The breadcrumb bar is the only toolbar, in sidebar tone, sharing the
  sidebar's hairline as an inverted L. Worktree name is a quiet mono chip in
  it. Complete moves into it as text. Sidebar toggle and Monitor become quiet
  glyphs, not AppKit buttons.
- No pane header with one pane; a 24pt strip with two or more. Split / close
  / zoom live in keybinds (`⌘D`, `⌘⇧D`, `⌘W`, `⌘⇧↩`), the context menu, and a
  hover-only trio on the focused pane. Unfocused panes dim to 0.85; no blue
  focus border.
- Terminal edge-to-edge (no 12pt gaps or radii), background warm charcoal
  `#24211F`, foreground `#EDE7DF`, prompt/cursor rose `#D9959D`. Check ANSI
  legibility on that background before landing.
- Membership chip in the breadcrumb becomes neutral mono text, not a capsule.
- Proof: `namedSessionDrillDownRetainsTerminal` and the split-retention proof
  still pass with real PTYs; capture one-pane and two-pane Sessions.

### S4 — Wave and Task pages (after S1; can overlap S2/S3)

Owner: `WorkSurfaceView.swift`, `TaskFlowView.swift`, `WaveDetailPane.swift`,
`MarkdownBlocks.swift`, `TaskCommentsView.swift`, `TaskRunsView.swift`.

- Apply the ramp: Wave 34, Task 26 (serif, Jack's call), objective 17
  clamped to four lines with "more".
- Remove the `Started` chip from every started plan row; chips only for
  running / done / human / blocked. Move `Wave controls` out of the reading
  column (into the toolbar's overflow).
- One running status line under the Flow: `● review-slice · 12m · claude`
  with elapsed time from the Run record, replacing the sentence. The running
  node carries the only animation (2s shimmer).
- Flow: return edges land at distinct ports of `implement`; loop labels
  inside the region's top-left; 1.5pt connectors; `↳ then` reads
  `↳ delivery` and queue/land show their real state.
- Description line-height to match the mock's 1.55; Comments and Runs
  headings on the shared label step.
- Proof: TaskFlowProofTests, TaskCommentsProofTests, TaskRunsProofTests;
  capture the real 13-node Feature at both widths.

### S5 — Bind lf-launched Sessions to their Task (Rust, independent)

Owner: `rust/loopflow/src/lf/commands/run.rs`, `ops/run.rs`,
`ops/human_session.rs` and their tests; Session DTO fixtures if the wire
shape changes (it should not).

- At interactive/headless launch with no explicit `--task`/`--as`, consult the
  existing `ops::task::task_for_checkout` (branch → registered Task). When it
  resolves, the Run and its Session carry that Task as Work; otherwise Work
  stays `None`. No inference from cwd path text, no history rewrite.
- This reverses the earlier "never infer membership from the checkout" rule
  only for launches that happen through `lf` (Jack, 2026-09-26).
- Proof: real isolated CLI test — a launch in a Task branch's checkout lists
  the Session with that Task's Work; a launch on an unregistered branch lists
  `null`; an explicit `--task` still wins. fmt + clippy.

### S6 — Launch surfaces and the orphan control room (design first)

Owner: a design note `scratch/session-launch-surfaces.md`, then
`SessionsView.swift` control-room presentation and `WorkspaceProjection`.

- Design: enumerate every place a Session can start (Task title button, Wave
  page, orphan section, `lf` in a terminal, a companion pane, ⌘K) and what
  ancestry each infers. Waveless first run is out of scope.
- Control room: the existing multiplexer with one pane per orphan Session and
  a bind-to-Task action per pane. Opened from the orphan header.
- Proof: design reviewed by Jack before the implement Run starts; mounted
  proof that binding from the control room updates the sidebar without
  relaunching the client.

### S7 — Snappiness (second pass, Jack 2026-09-26)

Jack: "we can do perf as a second pass." The instrumentation the study added
(`Services/Perf.swift` signposts, `DesktopPerformanceTests`) stays in the tree;
the optimizations wait until S1–S6 are accepted. Details in
[frontend-performance.md](retro/frontend-performance.md).

Carried into the second pass from the study's hand-back:
- Two safe fixes already shipped: `PodiumReading: Equatable` with
  assign-on-change (the idle app re-rendered the whole workspace every 2 s),
  and `MarkdownBlocks` `.equatable()` on source.
- Measured idle on the installed app: 51 ms/s hitches in a 6 s probe, 5.1 ms/s
  over 45 s, eight potential hangs of 166–483 ms with no interaction, RSS
  flat. Harness cold start at density 104–112 ms.
- Harness blocker: the perf fixture answers `lf ls` with `[]`, so only the
  cold-start test runs; the ~10-line fix is to return the four Waves as
  `WaveSnapshot` rows the way `WorkspaceNavigationProofTests` does. First job
  of the second pass.
- Next three, evidence-led: stream Sessions instead of 2 s `lf session list`
  / `lf activity` spawns; cache `WorkspaceProjection`; memoize
  `FlowDiagram.layout` per (graph, width).
- Build budget: this checkout sits at 11.5/12 GiB (target 7.7 G, swift/.build
  3.7 G). Clean before the Xcode fallback compile, as in
  [final-build-plan.md](implementation-cycle/final-build-plan.md).

### S8 — ⌘K palette (after S2–S4)

Owner: new `CommandPalette.swift`, `SessionsView` key handling.
Waves / Tasks / Sessions / Flows / actions, recents first, shortcuts shown.
Bottom search stays a filter. Lowest priority of the visual slices.

### D0 — Data model: docs first, then tables (Jack, 2026-09-26)

Jack: "Make sure the first piece of work for this project is rewriting the
architecture docs and user-facing docs to fit this new mental model."

1. **Docs as spec.** Rewrite `docs/architecture.md` (Core models tree and
   table), `docs/architecture-reference.md` (planning vocabulary, core
   models, persistence map, questions and sessions), `docs/waves.md`,
   `docs/lf.md` where it names chapters/positions/sessions, and the
   CLAUDE.md "Wave planning" block, to the model in
   [data-model.md](data-model.md): repo-level Chapter as a clock; Project =
   (Wave, Chapter) holding Tasks/KRs/targets and the chapter's Flow
   (template); Task holds Flow invocations; Flow invocation as one object (unrolled graph + cursor + returns, `parent` for
   runtime nesting); `runs` with nullable `invocation ⇒ task ⇒ wave` and a
   source; `sessions` as a child of `runs`; validators on every
   denormalization; no sidecars. Reviewed by Jack before step 2.
2. **Tables and readers.** Migrations for `runs`, `sessions`,
   `flow_invocations`; one reader per object; delete the string subjects,
   the sidecars, `WorkCatalog` matching, the launch resolver on reads,
   Swift `subject(for:)` and the per-body projection rebuild.
3. Then S6 (bind + control room), S2's orphan predicate, perf #1/#2, S9.
4. **Deletion research (last, Jack 2026-09-26).** A `research` Run whose
   only question is "what code can we delete now that we've redesigned
   around this data model and API?" — every reader, resolver, sidecar
   writer, string matcher, Swift re-derivation, DTO field, test and doc
   paragraph that the new model made unreachable, with path:line and the
   proof that nothing depends on it. Followed by an implement Run that
   deletes, and a review that confirms the system got smaller.

### S9 — Flow template vs invocation views (Jack, 2026-09-26)

Owner: `TaskFlowView.swift`, `FlowGraph` projection in
`engine/flow_graph.rs`, `ops/task_flow.rs`, `lf flow list --json`.

Before a Run: the Task page shows the Flow *template* — composed sub-Flows
(`task-design`, `pursue`) folded, each disclosable to its steps. Once
running: the *invocation* view, fully unrolled, with cursor, per-loop return
counts and child invocations for entered loop bodies. Needs the engine to
keep template composition past load (it flattens at load today) and the
invocation tree from [data-model.md](data-model.md). After the data model
slice.

## Execution

Runs are `lf -b -m claude` in this checkout, one per slice, following the
pattern of earlier cycles (implement → compress → review-slice). File
ownership above is the concurrency rule; a Run does not edit outside its
owner list. Order:

1. S1 alone. Then promote to the dev Home and capture; Jack looks.
2. S2 + S3 + S5 in parallel (disjoint owners). S4 starts when S1's tokens
   are stable, in parallel with the others.
3. Review-slice over S1–S5 together; promote; Jack walks the seven-step demo
   script from [demo-native-workspace.md](demo-native-workspace.md).
4. S6 design → Jack → S6 implement. S7 as its study dictates. S8 last.

Mocks: S1 and S4 go native directly; the tokens are fully specified. S2 and
S3 changed most from D, so each Run first updates the polish mock
(`visual-study/polish/`) for its surface and captures it, then implements
natively against that capture. This is my executive decision, not Jack's;
it costs one HTML pass per slice and buys a same-day comparison target.

## Not in this pass

Dark mode. Waveless first run. Attention roll-up dot and OS notifications.
Comments posting. The measurements and external trials in
[questions.md](questions.md) stay owed to the Task, not to this pass.
