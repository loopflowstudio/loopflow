# Prototyping retro — LOO-291, 2026-09-23 → 09-26

Jack's verdict on the native build, 09-26: "It just looks a lot worse than the
mocks. Plus I bet we can even improve on the mocks." This document traces how
the prototypes moved the product, what Jack liked in them, where the native
translation lost it, and where the mocks themselves can be beaten.

Quotes are Jack's verbatim unless marked *(interpretation)*. Paths are relative
to `scratch/`; `archive/` means
`/Users/jack/Library/Application Support/Loopflow/scratch-archives/LOO-291-20260925-180126/scratch/`.

## Summary

- The process worked as a **subtractive** one. Every accepted step removed
  something: Project tier, ELSEWHERE badges, nested Session rows, the
  Session-with-context depth, the You badge, Pause, the duplicate title, the
  "Linear snapshot" label, the raw warning wall. Jack's clearest wins were
  reductions he asked for by name.
- Jack's taste, stated repeatedly: one connected frame, header and nav as one
  UI, data at the top and search at the bottom, hairlines over pills, literal
  lowercase mono skill names, serif for Wave-level titles, sans for Task titles,
  blue loops / yellow pending human / green done / red blocked, no invented
  status. "A calmer desktop workspace is a good vibe for this project."
- The native build is **structurally** faithful to direction D (sidebar width,
  row heights, panel radii, Flow chip geometry, tone hexes all match) and
  **tonally** unfaithful: neutral black/grey ink instead of warm ink, a wide grey
  drop shadow instead of a hairline, system-styled toolbar buttons, a July-era
  glass "lens" on Wave rows, a rose tab strip and blue-grey pane header on the
  Session surface, a capsule membership chip, and a green Complete pill. Those
  come from five different design passes and none is in the accepted mock.
- Two decisions drifted between mock and native: Task title serif (Jack
  disliked serif there on 09-25; D reintroduced it via "vibes: C"), and Wave-row
  Task counts (in the original A, dropped by the polish study in favor of
  Session bubbles, which Jack now rejects).
- The sidebar is the clearest case of eras stacking: repo header (09-25 A),
  Wave lens (07-15 #963), Task dot (09-25 D), unbound Session rows (09-24
  outline), bubble counts (09-25 task-entry experiment), bottom search
  (09-25 repo-frame). See "Sidebar" below.

## 1. Decision timeline

| # | Decision | Who | Problem observed | What changed | Superseded | Evidence |
|---|---|---|---|---|---|---|
| 1 | Eight navigation mockups A–H (09-23) | Jack asked: "make lots of differetn mock ups and ask me what i like" | Work and Sessions were two root surfaces | "I like A and D the best," then "it should probably be possibel to get both views" → round two A/D/I | Round-two's full-width default; later replaced by frame A | `archive/main-view-task-history-archive.txt` §Visual exploration; `archive/main-view-round-two.png` |
| 2 | One outline repo → Wave → Project → Task → Session (09-23) | Jack | Confusing app; "strip the interface back, then design elements together one by one" | "Expandable Wave → Project → Task tree" but "the wave/project layers should be pretty minimal/collapsible so that its relaly just one annotated well organized list" | Project tier made internal on 09-24 ("one Wave, one current chapter Project") | history §Latest human direction |
| 3 | One current conversation per repo/Wave/Project/Task (09-23) | Jack proposed | Sessions before Tasks, worktree sessions, top-level agent sessions | "open a session with either the repo, wave, or proejct ... you only have the 'one' session per level"; "start them over mostly" | Integration amendment 09-23: New conversation at visible scope, no cardinality enforced. Repo-scope Sessions never got a presentation home (see Sidebar §) | history §Optional conversations; §Integration amendment |
| 4 | Native outline compact/full/Sessions (09-24) | Agent, from canvas direction | Competing Work/Sessions controls | `WorkspaceProjection.outline`, Session leaves under Tasks, "Unknown ancestry remains explicit", ancestry text on unmatched rows | Frame A sidebar (09-25) kept the leaves for unmatched Sessions only | `archive/outline-implementation.md` |
| 5 | Restored demo rejected (09-25) | Jack: "it still looks kinda horrible tbh" | Dense tree + raw directive wall; burgundy/deep-wine rail restored but composition wrong | Three website compositions A/B/C (Opus 5.5), Notion/Figma/Claude research | The whole earlier native composition | `archive/visual-study/brief.md`, `demo-restore-evidence/configured-ready.png` |
| 6 | Frame A accepted (09-25) | Jack: "A seems like the right default view, and you can zoom > Session with Context > Session"; "A calmer desktop workspace is a good vibe for this project" | — | A became default; three zoom depths built | Zoom depth 1 later removed (row 12) | `visual-study/a-wide.png`, `archive/visual-study/zoom-brief.md` |
| 7 | Repo frame refinement (09-25) | Jack | Two branded boxes; ELSEWHERE tag; global tree | "I dont know about the ELSEWHERE tag ... simplify for now"; "something inbetween what we had before -- where the single repo case was optimized for and teh repo was put in the header"; "still making it seem like the header and the nav are part of one UI"; "Search maybe goes at the bottom and teh data goes at the top, to connect the nav and the repo header" | Original A's separate `loopflow` header row and top search | `archive/visual-study/repo-frame-brief.md`; `visual-study/refinement-evidence/overview-wide.png` |
| 8 | Task → Session entry (09-25) | Jack | Nested Session rows under Tasks | "I think maybe we have an in-line indicator or count for open sessions, and wehn you click on a task with one open session you get the session" | Nested Session rows; also the Wave-row Task count from original A quietly dropped *(agent choice)* | `archive/visual-study/task-entry-brief.md`; `visual-study/task-entry-evidence/started-tasks.png` |
| 9 | Task header A (09-25) | Jack selected A, refined | Duplicate title, Open label, "Linear snapshot · read only" | Wave + issue ID in breadcrumb, title once, **sans** title ("the human disliked serif in this Task-page context"), label "Description", "The objective is great as it is", sidebar = started Tasks only | Serif Task title re-entered via D (row 16) — **drift** | history §Task entry experiment; `visual-study/header-evidence/selected-a.png` |
| 10 | Flow C (09-25) | Jack prefers C | A pills / B outline / C map | Connected node map with nested group; searchable Flow name; Comments below Description | A/B | `visual-study/flow-references/study-c.png`, `flow-research.md` |
| 11 | Feature as one path with loop regions | Jack, iteratively | "two-section Feature view still read as multiple Flows"; then three return edges | One continuous path → rounded loop regions → Jack: exactly one loop (Implement…Loop decide) → mono lowercase names, You badge | Jack later reversed to two loops (row 14) | `flow-research.md` §Accepted C |
| 12 | Task 2/3 studies: execution states, named Sessions, breadcrumb drill-down | Jack | Above-node marker, You badge, view-mode switch | "reserve the above-node marker for execution status" *(paraphrased in task2-design)*; humans pale yellow, done green, running/loop blue, blocked red, stopped neutral; "Loop · Iteration 3"; Stop & restart on hover; "Selecting a session should be like drilling down even further from wave > task > session"; New session beside title; Description stays on Task | Session-with-context depth, Overview/Session mode switch, You badge, ochre human style | `task2-design.md`, `task3-design.md`, `flow-references/task2-running.png`, `task3-multiple.png` |
| 13 | Accepted "good enough" → native cycles 1–6 | Jack | — | Naming, named Sessions, frame, Flow, Comments, Recent runs | — | `main-view-task.md`, `implementation-cycle/` |
| 14 | Two loops, tuple iteration, no Pause (09-25) | Jack | Prototype single loop vs real `pursue.yaml` | "The 2nd loop is correct."; "we'll have to revisit the UI."; "keep building and once you're done we can do the UI discussion for 2x loops"; "Feature has TWO loops", "the iteration counter should just be a tuple"; Pause not yet required | Single-loop prototype, scalar `Loop · Iteration 3`, Pause control | `implementation-cycle/jack-iteration-tuple.md`; `archive/implementation-cycle/continuation-integration.md` |
| 15 | Polish A/B/C on the native build (09-25) | Jack: "Prototype 3 different visual design polishes on the work in this branch" | Raw warning, heavy selected pill, inconsistent headings, Flow tail clipped, colliding arrowheads, green reads grey under loop tint | A quiet editorial / B crisp tool / C warm spacious | — | `visual-study/polish/README.md` |
| 16 | Direction D (09-25) | Jack: "left pane: A, center pane: B, vibes: C" | — | D = A sidebar + B structure + C mood. **Agent interpreted "vibes: C" as Cormorant page titles including the Task title**, contradicting row 9 | Sans Task title | `polish/README.md` §Jack's pick |
| 17 | Native pass 1 / pass 2 (09-25) | Jack: "go ahead and get these visual changes directly implemented in the actual swift code"; "go for it" | — | `WorkspaceStyle.swift`, restyled Navigator/WorkSurface/TaskFlow/Breadcrumb/Comments/Monitor; dark contrast | — | `polish/native/README.md`, `polish/native/pass2/` |
| 18 | Verdict (09-26) | Jack | Native "looks a lot worse than the mocks"; sidebar "very incoherent ... different era of design sensibilities ... the bubbles aren't quite working" | This retro | — | `demo-native-workspace.md` |

Which were Jack's: rows 1–3, 5–9 (quotes), 10–12 (preference + corrections), 14, 16, 18.
Agent interpretations that stuck: zoom-depth mechanics (6), dropping the Wave Task count (8), serif Task title in D (16), the lens on Wave rows and the system chrome (never in a brief).

## 2. What Jack liked in the mocks

Concrete tokens from the accepted files. Mock A = `visual-study/mockups.html`; D = `visual-study/polish/polish.css`.

**Frame.** One grid: sidebar 264px (D `--side-w`) / 272px (A `--sw`), header row 48px, content max 1160px with `padding: 36px 44px 80px` and section gap 26px (D `.v-d`). Sidebar and header share one material (`--paper #F4F0E9` in A, `--side #F3EEE7` in D) with a 1px `--line` right rule, no second brand row. Search anchored at the bottom in a `rgba(255,255,255,.55)` field, 30px tall, radius 6–7.

**Type.** `Cormorant` 500–600 for Wave-level titles only (repo 25px/500 `--burgundy-ink #5E2630`; Wave rows 19px/500; Wave page 48px; objective 18px serif `--ink-2`). `Lato` 13–13.5px body, 11px/700/`.06em` uppercase section heads in `--ink-3`. `JetBrains Mono` for skill names (11px chips), issue IDs (12px), tuple chip (11.5px). Task title: 24px sans/600 in the accepted header A; D moved it to 32px serif/500 (see drift).

**Palette.** Cream `#FAF8F5` page, `#FFFDF9` paper panels, warm ink `#2A2624` / `#5F5752` / `#8C837C`, rules `#E6DFD6` / `#D6CCC0`. Burgundy `#722F37` as accent only: 2px selection edge, outline primary button, breadcrumb ID underline, repo name. Charcoal `#2B3036` / `#24282D` terminal surround with `#DDD7CD` ink and `#D9A0A7` prompt. Loops `#3A74C4` / `#24508F` on `rgba(58,116,196,.10)`; running `#2F6BC0` on `#E7EFFA`; done `#3E7A4E` on `#D6EBD9`; pending human `#9A6B12` on `#F8E3AE`; blocked `#B23A30` on `#FBE6E3`; stopped `#6F6862` on `#EEEAE5`.

**Density.** Sidebar rows 30px, Wave rows +10px top margin, Task rows indented 26px with a 7px state dot, titles truncated to one line. Plan rows 32px in a bordered panel with columns `20px 14px 1fr 84px 64px` (rank, dot, title, chip, mono ID). KR rows 13px/3px padding with `01`/`02` burgundy tabular counters (A).

**Panels vs hairlines.** D: only Flow, Sessions, plan and metric table are framed (`border-radius:12px; border-color: rgba(230,223,214,.7); box-shadow: 0 1px 0 var(--rule), 0 8px 22px -18px rgba(42,38,36,.35)` — a hairline plus a shadow that is almost entirely clipped by its negative spread). Everything else is text on cream with 11px caps heads. A used hairline section heads only.

**Sidebar selection.** `rgba(255,255,255,.7)` tint, 2px burgundy bar at `left:-10px; top:7px; bottom:7px`, Wave name turns `--burgundy-ink`. No pill, no fill.

**Flow.** Chips 11px mono, `padding:4px 8px 4px 6px`, radius 5, gap 11, 9.5px sans number at left; 1px connectors with 3.5px arrowheads; loop region rounded, `--loop-fill-1`, 1px stroke at .35 alpha, second loop dashed; label "Loop 1 · 2 returns" top-left / "Loop 2 · 1 return" top-right; tail row indented with `↳ then`. Header: mono Flow name + chevron, mono `Iteration (2, 1)` chip on loop fill.

**Breadcrumb.** 13px `--ink-3`, `/` in `--rule-strong`, ancestors as quiet links, issue ID mono 12px burgundy with a 35%-alpha underline, Session name bold 14 with pencil rename on hover.

**Session rows.** Bordered list, 7px/8px padding, bold 13px name, provider 12px muted, mono `Feature · implement` membership in loop blue, right-aligned pill state (`Working` blue / `Your turn` yellow), one-line excerpt.

## 3. Mock → native loss diagnosis

Sources: `visual-study/polish/native/pass2/*.png` (fixture data, 1500pt @2x) and `demo-ready-evidence/configured-product-after.png` (installed app, live data). Swift values from `swift/LoopflowMac/Design/WorkspaceStyle.swift`, `swift/Loopflow/Design/BrandColors.swift`, `swift/Loopflow/Design/DesignSystem.swift`, and the views named.

What matches (so the fix is not "start over"): sidebar 264pt, rows 30pt, Wave 19pt serif / Task 13.5 sans, selection tint + 2pt edge, page 1160/36/30/72, section gap 26, Flow chip height 24 / gap 9 / mono 11 / number 9.5, loop hexes, all six tone fills, 12pt panel radius, 11pt tracked caps heads, outline burgundy button, breadcrumb mono ID with 35% underline. Structure landed; tone did not.

| Discrepancy | Mock value | Native value | Where | Effect |
|---|---|---|---|---|
| Body ink is cold | `--ink #2A2624`, `--ink-2 #5F5752`, `--ink-3 #8C837C` (warm) | `text 0x1A1A1A`, `textSecondary 0x6B6B6B`, `textTertiary = textSecondary.opacity(0.78)` (neutral) | `BrandColors.swift` `LoopflowPalette.light`; `WorkspaceStyle.swift` | Every label reads harder and greyer than the mock; the cream stops feeling warm. Single biggest lever. |
| Hairlines are cold too | `--rule #E6DFD6`, `--rule-strong #D6CCC0` | `border 0xE3DDD5`, `borderStrong = text.opacity(0.16)` (grey over cream) | same | Grey rules on cream instead of sand rules |
| Panel shadow | `0 1px 0 var(--rule), 0 8px 22px -18px rgba(42,38,36,.35)` (crisp 1px rule + nearly-clipped warm blur) | `.shadow(color: .black.opacity(0.05), radius: 9, y: 5)`, border `palette.border.opacity(0.7)` | `WorkspacePanel` | Wide muddy grey halo under every panel; visible in `wave-light-1500.png` and `task-feature-light-1500.png` |
| Selection tint | `rgba(255,255,255,.7)` on `#F3EEE7` | `surface.opacity(0.72)` = `#FFFDFB` at .72 — equivalent, fine | `WorkspaceNavigator` L240 | ok |
| Font smoothing | `-webkit-font-smoothing: antialiased` (thin) | AppKit default | n/a | *(interpretation)* Lato at 13–13.5 renders heavier natively; combined with `#1A1A1A` the sidebar looks bolder than the mock |
| Lato weights | Regular 400, Bold 700 only | `.weight(.semibold)` used in Flow detail, KR rows, `WorkSurfaceView` L483/560 (`.system(size:14, weight:.semibold)`) | `AppBootstrap.swift` registers only Regular/Bold | Semibold snaps to Bold or synthesizes; also two places fall back to the system font |
| Task title | Header A: 24px Lato 600 (Jack: sans, 09-25). D: 32px Cormorant 500 | `Typography.sectionTitle(32)` = Cormorant medium | `WorkSurfaceView` L220 | Drift: implements D, contradicts the earlier explicit preference. Needs Jack's call. |
| Wave row glyph | A: 7px rounded square in Wave accent. D: none | `WaveLensView` 11pt radial-gradient "HAL lens" with specular highlight, black rim, glow shadow; black when "Off · no active work" | `WorkspaceNavigator` L206; `WaveLensView.swift` (from #963, 07-15) | Skeuomorphic object in a flat sidebar; the "different era" Jack sees. Never in any brief. |
| Task row dot | 7px circle: `--running`/`--blocked`/`--stopped`, else `--rule-strong` | 7pt circle: tone ink, `neutral → borderStrong` (`text.opacity(.16)`) | `WorkspaceNavigator` L187, `taskTone` L321 | Neutral dots vanish; running/blocked read as small red/grey specks |
| Session count | D: 13px bubble svg + 11.5px count, 30px empty slot | `bubble.left` 11pt + 11.5 count in fixed 40pt column | L268 | Faithful to D — and Jack rejects it. Original A had a Task count on the Wave row (`.wave-row .count` 11px `--faint`), which is what he now asks for. |
| Unbound Session rows | A: one row "Untitled conversation · NO TASK". D: none | Two-line rows, `terminal` glyph, subtitle "Repository or unavailable ancestry", depth 0 after all Waves, no heading | `WorkspaceProjection.swift` L197, L247 | Four to ten of these on live data; they carry the 09-24 outline's ancestry text into the 09-25 frame |
| Toolbar chrome | A/D: breadcrumb only, one bordered sidebar-toggle icon | Breadcrumb + system-bordered sidebar toggle + system `Monitor` button, both grey system style | `SessionsView` toolbar | Two AppKit buttons in a page that otherwise has none |
| Session surface chrome | A: charcoal `.work` panel, 38px `.wbar` `#1C1F23` rule, dim `#8F8B84` labels, pane border `#3D434A`, focused `#8E5A60` | Rose tab strip (`loopflow` folder tab), blue-grey pane header with `Shell` + four system icons, then terminal; large green `Complete` capsule bottom-right | `SessionsView`, `TaskWorkspaceView` | `session-light-1500.png`: four surface styles stacked. None in the accepted Session mock |
| Membership chip in breadcrumb | D `.iter`: mono 11.5, `--loop-fill-1`, radius 4 | `Typography.code(11.5)`, padding 9/3, `Capsule()` fill | `WorkspaceBreadcrumbBar` L164–168 | A long pale-blue pill dominates the crumb; the mock's is a small square-cornered tag |
| Plan chip "Started" | D: chips only for running/done/human/blocked/upcoming | neutral outline chip `Started` on every started Task | `WorkSurfaceView` plan row | Adds a bordered chip to every row in the live capture |
| Wave page extras | D: objective, notice, KRs, Tasks, Metrics (`Edit description` is in D too) | + `Wave controls` disclosure below Metrics | `WorkSurfaceView` L192 | A control row in the reading column; native pass 1's own choice |
| Description line-height | A `.brief` 1.55; D `.desc` 13.5px | `MarkdownBlocks` default; only the objective gets `lineSpacing(3)` | `WorkSurfaceView` L126 | Denser paragraphs than the mock |
| Repo name color | D `--burgundy-ink #5E2630` | `accentInk` = `#722F37` | `WorkspaceNavigator` L105 | Slightly brighter; minor |
| Dark mode | never designed | tokens derived by opacity; contrast patched in pass 2 | `WorkspaceTone`, `accentInk` | Works, but is a derivation, not a design |

Pixel reading of `task-feature-light-1500.png` vs `/tmp/polish-d/task-1440x900.png` *(interpretation)*: the native page is about 8% wider in type (46/32/19 pt at 2x vs 48/32/19 px at 1x with browser antialiasing), the panel shadow is visible as a grey band under the Flow card, the sidebar row text is darker than the Wave name it sits under, and the Flow itself is the one region that reads exactly like the mock.

## 4. Sidebar: where each component came from

Jack, 09-26: "the left pane reads to me as very incoherent. the sessions at the bottom are kind of a left over we haven't solved at a data model level. the style on each component represents a slightly different era of design sensibilities. the bubbles aren't quite working yet. maybe they would be better as like simple counts of open tasks or something?"

### 4.1 Component origins

| Component | Introduced | Styling source | Swift |
|---|---|---|---|
| Repo header: serif name + chevron switcher + presentation glyph | 09-25 repo-frame refinement (Jack: repo "put in the header", "header and the nav are part of one UI") | A `.rsw .nm` 22px Cormorant 600 → D `.repo` 25px/500 `--burgundy-ink`; D `.icon-btn` 28px hover target | `WorkspaceNavigator` L90–149: `sectionTitle(25)`, `accentInk`, 28pt glyph, `surfaceMuted` |
| Wave row: chevron + glass lens + serif name | Row shape: 09-24 native outline (`outline-implementation.md`). Lens: **07-15 #963** "shared wave/project/task lens", wired into this navigator at `b493bb882` (09-25). Serif 19: D | Lens has no mock; A used a 7px accent square, D nothing | L171–178 chevron; L206 `WaveLensView`; L212 `sectionTitle(19)` |
| Task row: 7pt tone dot + 13.5 sans, one line | Dot: A `.task-row .st` (review wine / progress moss / upcoming hollow) → D `.dot-running/blocked/stopped` | D | L187 `Circle().fill(tone.ink)`; L212 `body(13.5)`; `taskTone` L321 maps pinned Flow execution |
| Session count bubble | 09-25 task-entry experiment (Jack: "in-line indicator or count for open sessions") → cycle 3 "Task Session leaves replaced by an inline count (bubble + n)" → D "fixed count column with bubble icon" | D `.nav-count` | L268 `bubble.left` + count, 40pt column |
| Unbound Session rows: terminal glyph, title, "Repository or unavailable ancestry" | 09-24 native outline: "Unknown ancestry remains explicit. Equal Session labels use distinguishing ancestry" | None of A/D styled it; the two-line row is the outline's own | `WorkspaceProjection` L197 (detail text), L247 (appended at depth 0 after Waves); Navigator L184 `terminal` glyph, L217 caption(11) detail |
| Selection: tint + 2pt burgundy edge | D from A ("light tint plus a 2px burgundy edge mark") | D | L240–246 |
| Bottom search, rounded | 09-25 repo-frame (Jack: "Search maybe goes at the bottom") | A `.search` 30px radius 6 → D `.side-search` radius 7 white .55 | L64–76 radius 7, `surface.opacity(0.55)` |
| Deep-wine sidebar (rejected) | pre-09-25; `configured-ready.png` | — | gone; `surfaceMuted` now |

So the pane is: A's frame, D's rows and selection, the 09-24 outline's Session leaves, and a July lens. Four passes, no reconciliation pass.

### 4.2 Unbound Sessions

1. **09-23.** Design put Session children under every Task; Jack pushed back on Task-only population: "amybe we do need unfiled tasks / taskless sessions", and proposed "open a session with either the repo, wave, or proejct". The design answered with one-current-Session-per-subject and a repo-header Start conversation — a home for repo-scope conversations.
2. **09-23 integration amendment.** That cardinality was dropped: "New conversation starts a fresh conversation at the visible repo/Wave/Project/Task scope ... Neither creates a Task, enforces cardinality". The repo-header home went with it; repo/Wave conversations became context-menu actions with no row.
3. **09-24 native outline.** Sessions became leaves under Tasks, Waves, and — for anything unmatched — the repository root, with distinguishing ancestry text. This is where "Repository or unavailable ancestry" was born, as honesty about a join the app cannot make (shell-launched clients register a cwd, not Work).
4. **09-25 task-entry experiment.** Jack removed nested Session rows under Tasks (count instead). The brief said "Remove nested Session rows only for A ... Unbound Sessions remain reachable." Cycle 3 kept "Wave-level and unmatched Sessions remain leaves." The accepted design still says "Keep historical, unmatched and repository/Wave Sessions reachable through the same inventory."
5. **Result.** Task Sessions got a new model (count → drill-down → named breadcrumb). Repo/Wave Sessions kept the 09-24 leaf presentation because no later decision touched them. Jack is right that this is a data-model gap: the app has Sessions whose subject is a checkout, and the only subject kinds the sidebar knows are Wave and Task. `demo-native-workspace.md` records the same reading.

### 4.3 State circles and counts

- **Wave circle = `WaveLens`** (green/red/blue/black/unknown-amber), folded from Task conditions: blocked > waiting > unknown > clear; black means "Off · no active work". It was designed in July for the Wave roster (HAL allusion, glass rendering) and predates every mock in this Task. In the live capture it reads as a burgundy ring on `infrastructure` and solid black on the other two — the black is the *normal* state, which is why it communicates nothing.
- **Task dot = `WorkspaceTone`** from the pinned Flow's execution (running/human/blocked/stopped), else neutral. Same 7px vocabulary as the mock; the neutral color is too faint (`text.opacity(.16)`).
- **Bubble = open-Session count.** Introduced by Jack's 09-25 suggestion for Task→Session entry, styled by D. It counts conversations, which is orthogonal to the "what needs me" question the sidebar is scanned for.
- **Counts were there first.** Original A (`a-wide.png`, `mockups.html` `.wave-row .count`) showed a quiet Task count on each Wave row (`Product 2`, `Infrastructure 1`). The polish study's B kept a mono count pill, A/D moved the count column to Session bubbles, and the Wave count disappeared without a decision. Jack's "simple counts of open tasks" is the original A sidebar.

## 5. Beyond the mocks

Grounded in what Jack kept pushing for, what the HTML could not do, and what native can.

1. **One glyph vocabulary, one count.** Wave row: serif name + open-Task count (A's `.count`), no lens. Task row: 7pt tone dot only when there is a tone (running/human/blocked); nothing when idle. Delete `WaveLensView` from the navigator; it belongs to the roster era.
2. **Give unbound Sessions a subject.** Either a `Terminals` tray at the bottom of the sidebar above search (glyph + name, one line, no ancestry sentence), or move them out of navigation into the workspace's retained-terminals menu that already exists (`Show retained terminals`). The sidebar then lists only Work. This is the data-model fix Jack named: a Session's subject is repo | Wave | Task, and repo-subject Sessions get the repo header as their row, which Jack proposed on 09-23 and the design dropped.
3. **Warm the ink.** Replace `LoopflowPalette.light.text/textSecondary` with `#2A2624 / #5F5752` and add a real `textTertiary #8C837C`; make `borderStrong` a sand hex, not `text.opacity`. This alone closes most of the "looks worse" gap because every string inherits it.
4. **Hairlines, not halos.** `WorkspacePanel`: keep the 12pt radius and `.7` border, drop the 9pt blur; if depth is wanted, a 1pt bottom rule in `--rule`. Jack chose A's "hairline rules" for the sidebar and B's bordered panels for the center; neither had a soft shadow. C's shadow was the only part of "vibes: C" the mocks let through and it is the part that reads muddy natively.
5. **One chrome, no system controls.** Breadcrumb bar is the only toolbar; sidebar toggle and Monitor become quiet glyphs styled like `.icon-btn`; the Session surface uses A's charcoal `wbar` (38pt, `#24282D`, `#8F8B84` labels, focused pane edge `#8E5A60`) for tab strip and pane header alike; Complete moves into that bar as text. Native can draw the terminal chrome with real vibrancy and hairline separators the HTML faked.
6. **Task title: decide once.** Jack said sans on 09-25 and "Cormorant for page titles" is the agent's reading of "vibes: C". Options: sans 24/600 (header A), or serif 32/500 (D). Show both natively side by side; do not carry the ambiguity into another cycle.
7. **Flow: use native measurement.** The diagram already matches the mock; go further: keep one row at any width by letting the loop lanes wrap under the chips instead of wrapping the tail; hover a chip to show that occurrence's latest Run summary (the HTML fixture could only open a static detail); pulse the running chip's stroke at 1.5s under reduced-motion rules. Put the tuple only in the Flow header; in the breadcrumb, membership becomes quiet mono text, not a capsule.
8. **Session name provenance is visible.** Generated names (`luna-aria`, `wren-canon`) render in `textTertiary` italics until renamed; human names in `text`. The website fixture had no provenance; the native `SessionRecord` does.
9. **Density fixtures as sticky sections.** `mockups.html?population=large` (20 started / 50 per Wave) proved the list holds; native `LazyVStack` with pinned Wave headings keeps the Wave visible while scrolling 50 rows — something the HTML study did not attempt.
10. **Dark as a designed theme.** The mocks' charcoal `#2B3036` work panel is already the dark palette's background. Design dark from that surface (ink `#DDD7CD`, rules `#3D434A`, prompt rose `#D9A0A7`) instead of deriving tokens by opacity. Pass 2 patched contrast; nobody has seen dark as a composition.

## Files

- Accepted design: `../main-view-task.md`; audit: `../review-note-audit.md`
- Mocks: `../visual-study/mockups.html`, `../visual-study/polish/polish.css`, `../visual-study/polish/README.md`
- Mock captures: `../visual-study/a-wide.png`, `../visual-study/refinement-evidence/overview-wide.png`, `../visual-study/flow-references/{study-c,task2-running,task3-multiple}.png`, `/tmp/polish-d/{task,wave,session}-1440x900.png`, `/tmp/loo291-accepted-{frame,task}-reference.png`
- Native captures: `../visual-study/polish/native/pass2/*.png`, `../demo-ready-evidence/configured-product-after.png`, `../demo-restore-evidence/configured-ready.png` (rejected 09-25)
- Native source: `../../swift/LoopflowMac/Design/WorkspaceStyle.swift`, `../../swift/Loopflow/Design/{BrandColors,DesignSystem}.swift`, `../../swift/Loopflow/AppBootstrap.swift` (fonts), `../../swift/LoopflowMac/Views/{WorkspaceNavigator,WorkSurfaceView,WorkspaceBreadcrumbBar,TaskFlowView,SessionsView,WaveLensView}.swift`, `../../swift/LoopflowMac/WorkspaceProjection.swift`, `../../swift/Loopflow/Models/WaveLens.swift`
- History: `archive/main-view-task-history-archive.txt`, `archive/visual-study/*-brief.md`, `archive/visual-study/polish/brief*.md`, `archive/outline-implementation.md`, `archive/implementation-cycle/cycle-03-implement.md`
