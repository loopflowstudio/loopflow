# Session launch surfaces and the orphan control room (S6)

2026-09-26. Design note for slice S6 of [ux-plan.md](ux-plan.md). Design only:
no source, commit or publication. Product behavior still answers to
[main-view-task.md](main-view-task.md); this note decides where a Session can
start, what each start point may infer, and how an orphan is resolved.

Every choice is tagged **[Jack]** (recorded in
[demo-native-workspace.md](demo-native-workspace.md)), **[consensus]** (the
six-product baseline in [state-of-the-art.md](retro/state-of-the-art.md) §6–7)
or **[proposal]** (mine, pending). Line numbers are from this checkout at the
time of writing.

## Problem

Four Sessions on the live Home have no Work (`lf session list --json` today:
`demo` and `review-slice` in `loopflow.main-view-task`, `loopflow` in
`loopflow.build-size`, `luna-aria` in `loopflow.release-run`; all
`work: null`). The sidebar appends them after the Wave tree as two-line rows
subtitled "Repository or unavailable ancestry"
(`swift/LoopflowMac/WorkspaceProjection.swift:75`, `:197`, `:247`). Jack:
"the sessions at the bottom are kind of a left over we haven't solved at a
data model level."

The data-model gap: a Session's Work comes only from the subjects declared on
its Run manifest at launch (`rust/loopflow/src/run_record.rs:2304-2347`,
`:2349-2367`), and nothing can change it afterwards. There is no manifest
rewrite (grep for `write_manifest|update_manifest|rewrite_manifest` finds
none) and no `lf session` verb touches Work
(`rust/loopflow/src/lf/mod.rs:724-779`). So any `lf` typed into a terminal
without `--task`/`--wave`/`--as` is an orphan forever, even when it sits in
LOO-291's worktree.

Who benefits: Jack, scanning the sidebar for work that needs him, without a
tail of unclassified terminals; and the convergence goal itself, since every
orphan resolved by hand is one the app can later resolve automatically.

## The demo

On the installed app against the live Home: the sidebar ends with
**Orphan sessions · 4** above search. Clicking it opens a control room: four
terminals tiled 2×2, each strip naming the Session and its worktree. On
`review-slice`, **Bind to task…** opens a picker with LOO-291 already
highlighted ("matches this checkout"). Confirm. The pane leaves the room, the
sidebar shows the Session under LOO-291, and clicking it there shows the same
terminal with its scrollback and unfinished draft. In a shell:

```sh
lf session list --json | jq '.[] | select(.title=="review-slice") | .work_path'
# "product / LOO-291"
```

## Two axes, one rule

The design's Session section separates two facts and this note keeps them
separate:

| Axis | Values | Who sets it | May it be inferred? |
|---|---|---|---|
| **Work** (ancestry) | Task / Wave / repository (none) | Declared at launch from CLI flags; after S5, also from the checkout branch at `lf` launch **[Jack, 2026-09-26]**; after S6, by an explicit human bind | Yes at launch by `lf` (branch → registered Task, `rust/loopflow/src/ops/task.rs:1064-1076`); never by Swift from path text, directory names or provider |
| **Flow membership** | `step` / `independent` / `unknown` | Rust, from the Flow position or the Run's recorded capture (`rust/loopflow/src/ops/human_session.rs:190-193`, `:255-266`) | Never. Not from Task, cwd, provider or skill; historical membership is never relabeled Independent |

Binding changes Work only. A bound Session keeps its name, its id (interactive
Session id is its Run id, `human_session.rs:1283`), its provider and its
membership label.

## Launch surfaces

Path through the code today: a UI launch builds
`[lf, --interactive, <binding>, :, <prompt>]`
(`swift/LoopflowMac/ConversationLaunch.swift:32-38`) and runs it as a new
shell pane in the chosen worktree (`swift/LoopflowMac/Views/SessionsView.swift:496-501`,
`swift/Loopflow/Models/MultiplexerStore.swift:159-173`). The shell exports
`LF_TERMINAL_ID`/`LF_TERMINAL_TTY`
(`swift/LoopflowMac/Services/Ghostty/GhosttyTerminalView.swift:1201-1206`) so
`lf` records which pane it runs in (`run_record.rs:935-943`) and the app can
focus that shell for the Session (`SessionsView.swift:205-208`). The app scrubs
`LF_RUN_ID` and friends from every shell
(`swift/LoopflowMac/Services/ProcessEnvironment.swift:33-45`), so a UI-launched
Session is never a child of another Run.

| # | Surface | Exists? | Work inferred | Checkout | Initial name | Breadcrumb | Never inferred |
|---|---|---|---|---|---|---|---|
| 1 | **Task title → New session** | Yes: `WorkSurfaceView.swift:230-247` → `newTaskSession` `SessionsView.swift:506-521` | Task, declared (`--task ISSUE`) | The Task's worktree; prepared through `lf task prepare ISSUE --json` if absent (`swift/LoopflowMac/MacLocalWaveAgentLauncher.swift:98-105`; `ops/task.rs:343`), which registers Work and worktree without executing | Word pair (`human_session.rs:1276-1281`: skill name if any, else `naming::word_pair(run_id)`); no skill is invoked here | Wave / Task / *name* | Flow membership. The Session is Independent even while the Task's Flow runs |
| 2 | **Wave page** | Partly: only the sidebar context menu "New conversation · *Wave*" (`swift/LoopflowMac/Views/WorkspaceNavigator.swift:297-300` → `startConversation` `SessionsView.swift:487-490` → `conversationScope` `ConversationLaunch.swift:43-56`) | Wave, declared (`--wave NAME`) | Repository root (`.wave(repo: repoPath, …)`), not a worktree | Word pair | Wave / *name* | A Task. A Wave-scope conversation that produces a Task does not retroactively belong to it **[proposal]** |
| 3 | **Orphan section / control room** | No | None on purpose. The room is where orphans are *resolved*, not created **[proposal]** | n/a for the room. A deliberate repository conversation stays where it is: repo header menu "New repository conversation" (`WorkspaceNavigator.swift:306`), `--interactive` with no binding, at the repository root | Word pair | Orphan sessions / *name* | Everything. A repository conversation is an orphan by declaration, not an accident |
| 4 | **`lf` in an external terminal** (Terminal.app, Warp, ssh) | Yes: any TTY `lf` (`docs/lf.md:895-905`) | Today: only from `--task`/`--wave`/`--as` (`lf/mod.rs:107-121`, `run.rs:732-746`). After S5: the checkout's branch resolves a registered Task; otherwise none | Wherever cwd is. No preparation | Invoked skill (`lf design` → `design`), else word pair | After S5 in a Task worktree: Wave / Task / *name*. Otherwise: Orphan sessions / *name* | A Wave from the path (`loopflow.release-run` is not a Wave name); a Task from a directory name; membership. `terminal_ids` is empty, so the app shows the pane placeholder with Open / Move here (`session_actions` `human_session.rs:93-135`) |
| 5 | **Companion pane inside a Session** (New terminal in this worktree `SessionsView.swift:578-584`, pane menu `:1070`, ⌘D) | Yes | Same as 4: the pane's cwd is the worktree, so S5 resolves the Task by branch. The pane's *neighbour* Session contributes nothing (env is scrubbed) | That worktree | Skill or word pair | Wave / Task / *name* | The neighbouring Session's Flow membership. A shell opened beside a Flow review is not part of the review. Its Session row focuses the existing shell surface (README `swift/README.md:143-145`) |
| 6 | **⌘K palette** (S8) | No | Whatever the chosen row names: "New session · LOO-291" (=1), "New session · product" (=2), "New repository session" (=3), "Bind *orphan* to task…" (control-room action) | As the target surface | As the target surface | As the target surface | Nothing from the current selection silently; the row text states the target **[proposal]** |

Session sources that are not launch surfaces, listed so the enumeration is
complete: `lf ask` inherits Work from the parent Run's preferred subject and runs
in the parent's cwd (`human_session.rs:425-435`); Flow human steps run
`lf --tui --as task:<id> <skill>` (README `:123`). Both already have Work and
never appear as orphans.

### Wave-page launch **[proposal]**

Add **New session** beside the Wave title, the same outline button as the Task
page, dispatching scope 2. Wave-scope Sessions today render as depth-1 leaf
rows before the Tasks (`WorkspaceProjection.swift:225`). Under S2's rule that
Wave rows carry no glyph or count, keep those leaf rows but style them as
one-line Session rows (terminal glyph, name), and list them in a Sessions
panel on the Wave page like the Task page's. S2 owns the styling; this note
only fixes their position. Open question 4 asks Jack whether Wave-scope
Sessions are wanted at all before the waveless-beginner work.

## Orphan section **[Jack: position and header; consensus: conditional, one-line rows]**

- Membership: `WorkspaceProjection.unmatchedSessions` (`:75`), which is every
  Session whose `work` is nil or not in this repository's roadmap. A Session
  with unavailable ancestry (`work_path` like `Task … (unavailable)`,
  `human_session.rs:1352-1372`) sits here too, with that text as its row
  detail. The "Repository or unavailable ancestry" sentence goes.
- Placement: last section in the sidebar, above the search field. Header:
  caps-11 label **Orphan sessions · N** (wording is open question 1).
- Header click opens the control room **[Jack]**. A chevron at the left of the
  header toggles the row list, the same split between disclosure button and
  row button the Wave rows already use (`WorkspaceNavigator.swift:174-185`).
  Default collapsed **[consensus]**.
- Rows: one line, 28pt: terminal glyph, name, provider in `textMuted`, and a
  6pt dot only for `active`. Clicking a row opens that Session alone through
  the existing `openSession` (`SessionsView.swift:438-460`). The sidebar filter
  already matches orphan rows by title and detail (`:192-199`).
- Zero orphans: the section is absent **[Jack: "if we get things smooth
  enough…"; consensus]**.
- Pre-S5 Sessions whose cwd is a Task worktree (today: `demo` and
  `review-slice`): **leave them as orphans** **[proposal, open question 3]**.
  Folding them by path is exactly the checkout inference the design forbids,
  and Jack's reversal covers launches through `lf`, not history. Instead the
  row shows a muted hint "in LOO-291's worktree" and the control-room picker
  preselects that Task, so resolution is one click. No retroactive rewrite by
  S5 either.

## The bind operation **[proposal]**

The missing `lf` verb. Rename is the precedent: it writes a `session-name.json`
sidecar in the Run's record directory under a lock and readers prefer the
sidecar over the seed (`run_record.rs:1079-1110`, `human_session.rs:1375-1384`,
`:1401-1450`).

```sh
lf session bind <session-id> --task LOO-291 [--json]
lf session bind <session-id> --wave product [--json]
lf session bind <session-id> --repository [--json]     # clear a binding
```

- `ops::human_session::bind` resolves the target through the existing
  `resolve_work_binding(store, cwd, "task:LOO-291")` (`rust/loopflow/src/ops/run.rs:101-127`),
  so only a registered Task or Wave binds, then writes `session-work.json`
  `{schema_version, selector: "task:<TaskId>", source: "human"}` beside the
  manifest, under a `.session-work.lock`, and returns the authoritative record.
  `--repository` removes the file. No store table, no migration (sibling PRs race on migration ordinals until merge), no manifest
  edit: the manifest keeps recording that the Run launched unbound.
- **One reader.** `run_record::work_selector(dir, manifest)` returns the
  sidecar selector when present, else `preferred_work_selector(manifest)`.
  `attributed_work` gains the `dir` it already has at every call site
  (`human_session.rs:1275`, `:1320`), and `RunSnapshot.subjects`
  (`run_record.rs:382-384`) folds the sidecar in at read time so
  `lf runs --task` (`rust/loopflow/src/lf/commands/work_catalog.rs:128-137`)
  and the Task page's Recent runs agree with the sidebar. Two readers of Work
  is the forbidden outcome.
- **Legality is Rust-owned.** `SessionActionKind::Bind` joins Open / MoveHere /
  Complete (`human_session.rs:79-83`, `:93-135`): available for interactive
  Sessions; for Ask and Flow Sessions it carries `unavailable_reason: "This
  Session's Work is its execution"` since their Work comes from the parent Run
  or the Flow position, not a sidecar. DTO change → `tests/fixtures/dto/session*.json`,
  `SessionActionKind.bind` in `swift/Loopflow/Models/SessionRecord.swift:16-20`,
  and both fixture tests move together, no defaults.
- Interactive Sessions are always local (`human_session.rs:1252-1266` scans
  the local observability Home), so there is no remote-Home refusal to write.
- Not guarded: cwd outside the Task's worktree, a completed Task, a Task in
  another Wave. The control room exists for human override; a wrong bind is
  reversed with `--repository` or another bind.

## Control room

**[Jack]**: clicking the orphan header opens "a special multiplexer with all the
orphan sessions in some sort of control room".

### Ownership

- The multiplexer is `SessionsWorkspace` (one `MultiplexerStore` plus the
  window's shared `GhosttySurfacePool`), keyed by checkout path in
  `SessionsWorkspaceRegistry` (`SessionsView.swift:15-50`, `:52-78`). The
  control room is one more `SessionsWorkspace`, keyed by a reserved key
  (`orphans`) in the same registry, sharing the same pool. No new store, no
  second surface owner.
- Panes are `.session(id)` (`swift/Loopflow/Models/MultiplexerLayout.swift:13-18`),
  rendered by the existing `SessionPaneView` (`SessionsView.swift:795`), so
  placeholders (Not running here / Open here / Move here) and the live surface
  behave exactly as in a worktree multiplexer.
- **One mount.** "Native surfaces belong to that window and are never mounted
  twice" (README `:148-149`). The room therefore replaces the worktree layout
  in the terminals area instead of sitting beside it: `WorkspaceNavigation.content`
  (`WorkspaceProjection.swift:307`) gains `.controlRoom`, and the ZStack in
  `SessionsView.swift:364-395` shows the room's `MultiplexerView` in place of
  `WorktreeNodeView`. While the room is on screen no worktree multiplexer is
  mounted, and vice versa, so a surface open in both stores is on screen once.
  Leaving the room restores the previous worktree layout untouched, like
  hiding a worktree (README `:139-140`).
- Layout: on entry, the room's store is reconciled to the orphan id set
  (`MultiplexerStore.reconcileSessions`, `:238-265`) and missing orphans are
  added by splitting, alternating axis, into a near-square grid. Zoom, focus
  keys and undo work unchanged (`:188-236`).

### Chrome (S3 rules)

The room is where N ≥ 2 is normal, so every pane has S3's 24pt strip: name in
mono 11, worktree basename in `textMuted`, provider, a 6pt state dot, and
**Bind to task…** as quiet text at rest. It is visible at rest because it is
the single primary action of that surface (S3's progressive-disclosure rule);
split / close / zoom stay in keybinds and the context menu. Unfocused panes at
0.85 opacity, no blue border, no per-pane close.

The breadcrumb reads **Orphan sessions · N**, no Monitor button. Selecting a
pane makes its Session current: the final crumb names it and offers rename,
siblings are the other orphans (`WorkspaceProjection.swift:91-93` returns
`siblings: [session]` today; it becomes all orphans).

### Bind interaction

1. **Bind to task…** opens a typeahead popover anchored to the strip, reusing
   the Flow picker pattern (search, choose, confirm; cancel changes nothing).
   Rows are this repository's Tasks grouped by Wave: `identifier · title`. The
   Task whose worktree path equals the Session's cwd is preselected and tagged
   "matches this checkout"; that comparison is presentation of two wire fields
   already present (`SessionRecord.cwd`, `task.reference.workspace.worktree`),
   never a write.
2. Confirm calls `RegistryQuery.bindSession(id:task:cwd:)` → exactly
   `lf session bind <id> --task <issue> --json` in the repository; the popover
   shows busy; failure appears inline in the strip with Retry, the pane stays.
3. Success triggers the ordinary Sessions refresh. The projection recomputes:
   the record now has `work`, so it leaves `unmatchedSessions` and joins its
   Task's `sessions` (`WorkspaceProjection.swift:45-75`). Consequences, in
   order:
   - Sidebar: the orphan row disappears; the Task's inline Session count
     rises (`WorkspaceNavigator.swift:242-243`); if it was the Task's only
     Session, clicking the Task now drills straight into it
     (`SessionsView.swift:462-472`).
   - Control room: `reconcileSessions` removes the pane. It does **not**
     release the surface: release happens only when a Session leaves the
     list entirely (`SessionsView.swift:412-414`, `:205-208`). The terminal,
     its provider client and scrollback are retained.
   - Task page and breadcrumb: Wave / Task / *name*; `lf runs --task` lists
     its Run; membership label unchanged.
   - Selection: if the bound pane was focused, `selectedSessionId` stays and
     `subject(for:)` now resolves to the Task (`:107-116`). While other
     orphans remain, stay in the room. When the room empties, navigate to the
     just-bound Session under its Task **[proposal, open question 6]**.
4. `--wave` binding is offered through the same picker's Wave headers
   (choose the Wave row itself) **[proposal]**. `--repository` is the
   picker's "Keep as orphan" only for a Session bound by mistake; the room
   does not expose it otherwise.

### Density **[proposal, open question 5]**

At 1440×900 a 3×2 grid gives roughly 470×400pt per pane: about 60×24 cells at
13px mono, readable but tight. Rule: the room mounts at most six panes, most
recently active first; beyond six, a one-line list at the room's left edge
(same rows as the sidebar section) names the rest and swaps one in on click.
Live count today is four, so the cap is a safety rail, not the demo path.

### Zero orphans

The section is hidden and the room is unreachable from the sidebar. If the
room is open when the last orphan binds or completes, it navigates as in
step 3. If the last orphan disappears for another reason (completed elsewhere),
the room shows one line, "No orphan sessions", and the breadcrumb's Wave
ancestors are the way out.

## De-risking

| Question | Finding | Impact |
|---|---|---|
| Can Work change after launch without rewriting history? | No manifest rewrite exists; `session-name.json` is the established post-launch sidecar (`run_record.rs:1079-1110`) | Bind is a sidecar, manifest bytes unchanged |
| Does binding change Session identity? | Interactive Session id is the Run id (`human_session.rs:1283`); name lives in its own sidecar | Selection, rename drafts and pane ids survive a bind |
| Does removing a pane kill the terminal? | `reconcileSessions` drops panes only (`MultiplexerStore.swift:238-265`); surfaces are released only when the Session leaves the list (`SessionsView.swift:412-414`) | Terminal retained across "row moves under its Task" |
| Can one surface show in the room and a worktree at once? | Pool is per window and views must not mount twice (README `:148-149`); today the ZStack shows one of `WorktreeNodeView`/`WorkSurfaceView` (`SessionsView.swift:364-395`) | Room is a third content mode, never rendered beside worktrees |
| Will `runs --task` disagree with the sidebar after a bind? | `matches_run` reads `run.subject("task")` from the snapshot (`work_catalog.rs:128-137`), a second reader of manifest subjects | Fold the sidecar into `RunSnapshot.subjects` at read time; one selector function |
| Do companion-pane launches know their pane? | `LF_TERMINAL_ID` receipt (`run_record.rs:935-943`) → `terminal_ids` → `localTerminal(for:)` | Surface 5 needs no new plumbing |
| Could a pane launch inherit its neighbour's Run? | `ProcessEnvironment.enriched` strips `LF_RUN_ID`, `LF_RUN_DIR`, `LF_PARENT_RUN_ID` (`:33-45`) | Membership cannot leak from a neighbouring review |
| How many orphans exist on the live Home? | Four (`lf session list --json`, 2026-09-26), two in LOO-291's worktree | Demo is real data; six-pane cap is not exercised |
| Is bind legality shareable with the CLI? | `actions` already carries Rust-owned labels and unavailable reasons (`human_session.rs:93-145`) | Bind is an action, not a Swift `kind` check |

## Alternatives considered

| Approach | Tradeoff | Why not |
|---|---|---|
| Fold orphans under Tasks by cwd match in Swift | Zero orphans on day one for the two LOO-291 Sessions | Swift inferring Work from a path is the forbidden inference; two builds would disagree with `lf session list` |
| Rewrite manifest `subjects` on bind | One file, no new reader | Manifests are launch evidence with path validation (`run_record.rs:542-545`); rename set the sidecar precedent |
| A `session_bindings` table in the store | Queryable | Migration ordinal races between sibling PRs; Session name already lives beside the Run, splitting one Session across two stores |
| Bind from every Session row (Task page, breadcrumb) | Rebinding anywhere | Scope; the room is the resolution surface, rebinding is a later pass |
| Room as an extra slot in the worktree layout | Reuses `WorktreeLayoutStore` splits | A Session pane in both the room slot and its worktree slot would mount one surface twice |
| Orphan rows stay in the tree at depth 0 (today) | No new section | Jack rejected it; it is the pre-drill-down leftover |

## Key decisions

- Work and Flow membership are separate axes; bind touches only Work.
  **[design, restated]**
- Orphans are resolved, not created, in the room; deliberate repository
  conversations keep their existing menu entry. **[proposal]**
- Pre-S5 Sessions stay orphans with a one-click preselected bind. **[proposal]**
- Bind is `lf session bind` with a Run-dir sidecar and a single selector reader
  shared by Sessions and `runs --task`. **[proposal]**
- The room is a content mode that replaces the worktree layout, not a slot
  beside it. **[proposal, forced by one-mount]**
- Room chrome follows S3: strip only, bind visible at rest as the single
  primary action. **[consensus + S3]**

## Scope

- In: orphan section, control room, `lf session bind` (+ `--wave`,
  `--repository`), Bind action in the Session DTO, Wave-page New session
  button, ⌘K rows named here (S8 implements the palette).
- Out: waveless first run **[Jack]**; dark mode **[Jack]**; auto-binding of
  historical Sessions; rebinding from Task pages; OS notifications; posting to
  Linear; any change to Flow membership or the runtime cursor.

## Done when

- `lf session list --json` shows `work_path` for a Session bound through the
  room, `runs --task` lists its Run, and its `flow_membership` and `title` are
  byte-identical before and after.
- On the installed app with live data: the section shows the real count, the
  room tiles the real orphans, binding `review-slice` to LOO-291 moves it
  under the Task, and the same terminal reopens there with its draft.
- Zero orphans hides the section; the room never mounts a surface that a
  worktree multiplexer is also showing.

## Forbidden outcomes

- A second reader of Run Work (Swift path matching, a Swift-only binding
  store, or `runs --task` ignoring the sidecar).
- Bind altering `flow_membership`, the manifest, or the Session name.
- A Session surface reachable from two mounted panes.
- The section or room deciding a Session's Task from cwd text.
- A guard that refuses binding by cwd, Task state or Wave.

## Internal slices

1. **lf bind** (Rust): `session bind`, sidecar, one selector reader,
   `Bind` action, DTO fixtures. Proof: real isolated CLI test.
2. **Room and section** (Swift): orphan section in the navigator (coordinate
   with S2's owner list; land after S2 or as its final commit), `.controlRoom`
   content mode, room workspace, strip with bind picker,
   `RegistryQuery.bindSession`. Proof: mounted native test below.
3. **Launch surfaces**: Wave-page New session; breadcrumb ancestor "Orphan
   sessions"; ⌘K row names handed to S8.

## This slice

Slices 1 and 2 together are the smallest cut that shows the demo. One
behavioral proof, `OrphanControlRoomProofTests/orphanBindsFromControlRoom`,
mounting the production `SessionsView` with fixture transport and two owned
`/bin/cat` PTYs, in the pattern of `TaskRunsProofTests`:

1. Two orphan Sessions listed → sidebar shows **Orphan sessions · 2**, no
   depth-0 Session rows, no "Repository or unavailable ancestry" text.
2. Header click → content is the room, two `.session` panes, worktree
   multiplexers unmounted.
3. Type a draft into pane A. Bind A → exactly
   `session bind <A> --task W2-131 --json` is issued; fixture returns the
   record with `work`.
4. After refresh: room has one pane (B); sidebar shows W2-131 with count 1;
   selecting W2-131 drills into A; A's surface is the same identity and the
   draft echoes back; B's `cat` still replies.
5. Bind B with a fixture failure → error in B's strip, pane stays, B still
   an orphan.

Rust proof for slice 1, `session_bind_moves_work_without_touching_history`:
launch an unbound interactive stand-in in an isolated Home; `session list`
shows `work: null` and an available `bind` action; `session bind --task
INF-123 --json` returns `work_path`, `runs --task INF-123` lists the Run,
manifest bytes and `session-name.json` unchanged, `flow_membership` unchanged;
bind to an unregistered Task refuses and writes no sidecar; `--repository`
restores `null`; an Ask Session lists `bind` with an unavailable reason.

## Slice ledger

- 2026-09-26: design written; no source changed.

## Open questions for Jack

1. Header wording: **Orphan sessions** (your phrase) or **Other sessions**
   (state-of-the-art recommendation)? Proposal: Orphan sessions.
2. Header click opens the room and a chevron toggles the list. Or should the
   header only disclose, with a separate "Open control room" affordance?
3. Pre-S5 Sessions in a Task worktree (`demo`, `review-slice` today): leave
   as orphans with a preselected one-click bind (proposal), or fold by path?
4. Wave-page **New session**: wanted now, or defer with the waveless
   beginner? If wanted, do Wave-scope Sessions stay as depth-1 sidebar rows
   under S2's no-glyph Wave rule?
5. Room cap of six panes with an overflow list: acceptable, or tile
   everything?
6. When the room empties after a bind, jump to the bound Session under its
   Task (proposal), or stay on an empty room?
7. Should bind exist only in the room and ⌘K for this pass, or also on Task
   page Session rows for rebinding?
8. For S5's resolver (relayed, not mine): should a branch that maps to a
   completed Task still bind, and should the S5 subject be recorded with a new
   `AttributionSource` (only `Declared` and `Inherited` exist,
   `run_record.rs:180-185`) so a bind sidecar and a checkout-inferred subject
   are distinguishable in evidence?
