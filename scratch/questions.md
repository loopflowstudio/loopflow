# 3d4350 open questions and assumptions

## 3d4350 kickoff (2026-11-21, headless)

- Assumed: iPhone keeps its current sizes in a private `MobileType` (new faces only); mapping it onto the Mac ramp would shrink its text. Passed over: `#if os(iOS)` sizes in `Typography`. Reversible.
- Assumed: sidebar rows show no status start time ("Blocked since 12:07" in the prototype); program status reports carry none. The row accepts one when a later change records it.
- Assumed: Ghostty's `font-family` loads after the person's config (it appends, so loading it in `loopflowConfig` first would beat their family). They keep theirs first and get Spline Sans Mono as a fallback.
- Assumed: `GraphCompositionView` stays as the Graph-definition drawing; only renderers of a Task's position merge into the workflow line, and `WorkflowDiagram` is deleted.
- Open: DESIGN.md exists only on cfadc6's unpublished branch; its pointer and "Tokens today" edits wait for cfadc6 on main. Both branches edit `GhosttyManager.loopflowConfig`.
- 2026-11-21 concept review (headless), first pass over the 3d4350 plan: three changes, in the plan. Found: `position.at` is a bare step name and `concept-review` is a step of both `plan` and `build` in `code`, so the line's running step comes from a new `TaskGraph.current_step` that `read_task_graph` computes from the cursor. Chosen by the review, reversible: the sentence keeps the concrete step ("build is running concept-review."); the bar's Complete/Claim/Abandon/"Start from…" move into one "…" menu shared with the sidebar's `TaskActionItems`, and the Overview disclosure goes with `WorkflowDiagram`. Page: `scratch/concept-review.html` (LOO-515's sixth-pass page is in git at `9c25f3a21f`).
- 2026-11-21 write-docs (headless). Assumed, reversible, all in the docs: with auto on the sentence's second clause is "Auto stands in next at ship-review."; the Brief's sentence reads "Active. build is running implement."; Diff's title counts files ("4 changed files") and hidden files read "4 of 9 changed files", with the change count gone from the title; the pane header's notice line shows only when a Task's conversation starts fresh; the Files menu reads "TaskRow.swift · 1 of 3". `swift/DESIGN.md` not edited (cfadc6 deletes it).

## Slice 2 (ramp migration)

- Sized bold below 12 pt (`strong(8…11)`, 9 sites) mapped to `textStrong`
  (13 pt) rather than `meta` (11 pt regular), so bold labels keep their
  emphasis and `TaskChangesView`'s tour/non-tour group titles stay distinct.
  The table had no row for it. Reversible per site.

- 2026-11-21 slice 3 (headless): `lf task status --json` emits `TaskDetail`, not `TaskWork`; added a top-level `graph: TaskGraph | null` beside `task.graph` (`{name, step}`, the list row) so the acceptance `jq '.graph'` holds. Alternative: put `steps`/`sentence` on `TaskRowGraph`, which `lf task list` builds from summaries without the captured graph.
- 2026-11-21 slice 3: the Workflow catalog inspector shows `WorkflowSteps` instead of a definition-built `WorkflowLine`; a definition-built line needs top-level labels from Rust in the graph catalog (or a Swift copy of the `sources[1]` rule).
- 2026-11-21 slice 3: sentences chosen beyond the plan: an At at a non-interactive step (a landing wait) reads "Waiting at land."; a run with no interactive step ahead ends after "X is running." with no second clause.
- Sync kept this Task’s concept review at its existing path. LOO-515’s twelfth-pass page is in f4c8e99c48.

## Slice 4 (bar actions), choices made headless

- Auto-Ship while on reads **Stop Auto-Ship** and turns auto off; the
  prototype shows only the off state. Alternative: keep "Auto-Ship" with a
  checkmark.
- Waiting at an interactive step, the next-step button reads **Run** (was
  "Continue"), as swift/README.md says ("Run once its Graph has stopped"); the
  help text still says "continue after <step>".
- A completed Task with no Graph still shows a disabled **Start** carrying
  Rust's refusal ("Reopen the Task"), as the header did.

- Sync with LOO-515 kept the 3d4350 concept review in this checkout. Each
  Task uses the same scratch filename for its own review.

## Slice 5 (components), choices made headless

- The row's status word is the program's report label (**Busy**, **Blocked ·
  Auth**, **Available · inferred**) and falls back to the lifecycle word
  (**Active**) when nothing reports. swift/README.md said "ID and lifecycle
  word"; it now says both. Alternative: lifecycle word always, report only
  in the ask line.
- The sidebar's live-excerpt line ("Summary · …", turn output with its age)
  went with the summary line, with `SessionProgramStatus.preview(now:)` and
  its tests. The wire `SessionRecord.preview` now has no Mac reader; iPhone
  never read it. Follow-up question: drop it from Rust, or show the excerpt
  somewhere in the Task pane.
- The Task ID stays a link to the issue (plain text without a URL); the PR
  link and owner left the row.
- The ask line shows for a **blocked** report only (`msg`, else `title`);
  `ProgramStatusState` has no waiting state of its own.
- `.accent` buttons are filled with the accent (prototype `.btn.primary`);
  the old `emphasized` outline is gone. Used by the backlog's two primary
  actions and New Wave's **Create wave**.
- Sidebar Wave rows lost the selection border too (`SidebarSelectionShape`
  is shared), so every sidebar selection is fill only.
- No second title bar exists in code: the backlog's capsule is RepoView's
  same `TitleBar`; the moon button in the screenshot has no source here.
- The pane strip shows one `StatusMark` after the agent menu: the report's
  phrase, or a bare dot from the Session's availability.

- Slice 7 (implement, 2026-11-21): the Diff pill still shows the first part of
  `TaskDelivery.headline` ("Open #1949"), not the prototype's "#1949 open, 1 check
  failed". Rewording `TaskDelivery` changes the Brief's PR line and its tests too;
  left for review. Check order in the card (failed, in progress, passed) is also
  GitHub's order, unchanged.

## Slice 8: how main reaches this branch (2026-11-21)

LOO-515's branch (`31fc852dca`) does not contain #2011, so `lf sync` here is a
no-op. Waiting for LOO-515 to sync main or land keeps this PR's diff to 3d4350's
own commits; `lf sync main` now would detach and show LOO-515's work in this PR.
Took the wait. If LOO-515 stays open long, the other choice is to sync LOO-515
onto main from its own checkout.

- Sync kept this Task’s concept review and notes. Main removed the inherited LOO-515 scratch material.
