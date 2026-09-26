# Demo — configured native workspace, 2026-09-26

Reviewed: the installed signed app (`/Applications/Loopflow.app`, lf
`0.12.21+ad82f11c9.dirty`, built 2026-09-25 22:01) on the retained
chapter-bearing Home. It read live Product/Infrastructure planning, Comments
and four unbound Sessions (`demo`, `loopflow`, `luna-aria`, `review-slice`).
Governing design: [main-view-task.md](main-view-task.md). Build readiness:
[demo-ready-evidence](demo-ready-evidence/README.md).

## Jack's verdict

Jack, verbatim: "It just looks a lot worse than the mocks. Plus I bet we can
even improve on the mocks."

He did not walk the seven-step script (Wave → unstarted Task → Flow width →
LOO-291 → New session → Session drill-down → repo return). The first
impression ended the walkthrough. No step was accepted; none was individually
rejected either. Treat every "implemented locally" row in
[review-note-audit.md](review-note-audit.md) as still awaiting human
acceptance.

My interpretation (not Jack's words): the native build ported the structure of
polish direction D faithfully and lost the mood — type, spacing, contrast and
surface treatment read as a generic SwiftUI app rather than the mock. The
two-loop composition discussion never happened because the surface it would
happen on wasn't accepted.

## Jack on the sidebar (screenshot, 11:30)

Jack, verbatim: "the left pane reads to me as very incoherent. the sessions at
the bottom are kind of a left over we haven't solved at a data model level.
the style on each component represents a slightly different era of design
sensibilities. the bubbles aren't quite working yet. maybe they would be
better as like simple counts of open tasks or something?"

What the screenshot showed (my reading): repo header `loopflow` in serif with
a chevron and filter glyph; Wave rows in serif with large filled state circles
(infrastructure a burgundy ring, intelligence and product solid black); Task
rows in sans with truncated titles and small red/grey dots; then four unbound
Sessions (`demo`, `loopflow`, `luna-aria`, `review-slice`) as two-line rows
with a terminal glyph and the subtitle "Repository or unavailable ancestry",
no section heading, no separation from the Tasks above; selected row with a
light tint and burgundy edge; rounded search field at the bottom. Center:
a mono "Flow membership unknown" beside the Session name, a pink tab strip
`loopflow.release-run`, a blue-grey `codex` pane header, and a large bold
"Not running here / Open here" empty state — four surface styles in one view.

Three distinct problems in Jack's note:
1. **Data model:** Sessions with no Task ancestry have nowhere to live. The
   sidebar's Session list is a leftover of the pre-drill-down design.
2. **Coherence:** each component (repo header, Wave row, Task row, Session
   row, search, empty state, pane chrome) was styled in a different pass.
3. **State glyphs:** the Wave/Task circles ("bubbles") don't communicate.
   Jack's suggestion: replace with counts of open Tasks or similar.

## Jack on the right pane (same screenshot)

Jack, verbatim: "The right pane seems like a more faithful representation of
the style we were building towards in the prototypes, but is somehow visually
disconnected from the left and does seem like it could potentially get loud
fast. weird to have similar tool sets on both the run and the session — that
might need to get collapsed or progressively disclosed somehow."

What he is pointing at (my reading): the Session breadcrumb row, then a
worktree strip (`loopflow.release-run`) with terminal/split/stack/close
icons, then a pane header (`codex`) with split/stack/close/expand icons —
two toolbars with near-identical controls stacked one above the other, each
on its own tinted band, above the dark terminal. The left sidebar is cream
and quiet; the right stacks three chrome bands in three tints before content.

Problems named:
4. **Disconnection:** left and right share no rhythm, tint or type — they
   were styled separately (sidebar in polish D, pane chrome earlier).
5. **Loudness:** three stacked chrome bands; more panes means more bands.
6. **Duplicated tool sets:** the Run/worktree strip and the pane header both
   offer split/close/expand. Jack: collapse them or progressively disclose.

## Jack's decision on orphan Sessions

Offered two options: every Session belongs to a Task or the repository, with
repo-level Sessions in one quiet "Conversations" group; or Sessions never
appear as sidebar rows and orphans surface only through search.

Jack, verbatim: "Right, I think we need to keep working towards this from
both sides. If we get things smooth enough, we can start to auto assign
sessions into tasks/waves. If the session launches happen from within lf. I
think maybe move the sessions to the bottom near the search bar, and give it
a header like Orphan Sessions or something, and maybe clicking into that
header gets you a special multiplexer with all the orphan sessions in some
sort of control room?"

Agreed (Jack's direction, not a proposal):
- Long term, both sides converge: Sessions launched through `lf` get bound
  to a Task/Wave automatically once binding is reliable; the sidebar stops
  needing an orphan list.
- Now: orphan Sessions move to the bottom of the sidebar beside search, under
  a header such as "Orphan Sessions", out of the Wave tree.
- Clicking that header opens a control-room view: one multiplexer showing
  all orphan Sessions together.

Proposal (mine, pending): the header shows a count; the control room is the
existing multiplexer with one pane per orphan Session and a bind-to-Task
action per pane, so it is also where auto-assignment failures get resolved
by hand.

## Jack on where Sessions launch from

Jack, verbatim: "There's some work to be done in just thinking about where in
the UI you can launch a new session, and what we might infer. For now we can
assume some waves exist and worry about the waveless beginner later."

Scope decision (Jack's): the launch-surface question — every place a Session
can start from (Task title button, Wave page, orphan control room, `lf` in a
terminal, a companion pane) and what ancestry each launch point can infer —
is real design work that the plan must include. The waveless first-run case is
explicitly deferred; assume Waves exist.

Evidence on inference (mine): `ops::task::task_for_checkout` already maps a
checkout's branch to its registered Task and is what every PR entry point
uses. Interactive `lf` launches never consult it for the Session's Work, which
is why `demo` and `review-slice` sit in the LOO-291 worktree with `work: null`.
Inference from a UI launch point is stronger still: the Task page knows its
Task before any checkout exists.

## Jack on dark mode

Jack, verbatim: "I also think we should stick to light mode for now, and
then do dark mode when we're at a happy place as a followup."

Scope decision (Jack's): light mode only for this pass. Dark mode is a
follow-up once light is accepted. The polish pass-2 dark-contrast work
(`Color.adaptive`, `accentInk`) stays in place but is not extended, captured
or reviewed in this pass.

## Jack on the serif Task title

The retro flagged a drift: Jack disliked a serif Task title in the 09-25
header study; direction D reintroduced it via the agent's reading of
"vibes: C". Asked to decide once.

Jack, verbatim: "Im kinda liking the serif task title now."

Decision (Jack's): the Task page title stays serif (Cormorant), as D renders
it. The 09-25 header-study preference is superseded.

## Jack on sidebar Wave rows

The state-of-the-art study left two tensions: serif vs sans Wave names in
the sidebar, and whether Wave rows carry a count, a needs-you dot, or nothing.

Jack, verbatim: "1. sans-serif wins. 2. sure, nothing at all is even simpler.
maybe we add the dot back later when we've got a better handle on the basics."

Decisions (Jack's):
- Sidebar Wave rows are sans section heads. Serif is reserved for the repo
  name and page titles. This supersedes D's serif Wave names and the earlier
  "counts of open Tasks" suggestion.
- Wave rows carry no glyph: no circle, no count, no dot. `WaveLensView`
  leaves the sidebar. A needs-you roll-up dot is a possible later addition,
  not part of this pass.

## Jack's answers to the S6 design questions

From [session-launch-surfaces.md](session-launch-surfaces.md), 2026-09-26.

| # | Question | Answer | Whose |
|---|---|---|---|
| 1 | Header wording | "Orphan sessions" | Jack: "mine" |
| 2 | Header click | Opens the control room; chevron toggles the list | Jack: "header click opens" |
| 3 | Pre-S5 orphans in a Task worktree | Leave as orphans with a preselected one-click bind | Jack: "no idea" — default taken; no Swift-side path inference |
| 4 | Wave-page New session | Defer — and revisit once the basics are liked | Jack: "defer ok, but note it as a thing to discuss once we like the basics" |
| 5 | Room pane cap | Tile everything, no cap | Jack: "acceptable" |
| 6 | Room empties after bind | Jump to the bound Session under its Task | Jack: "jump" |
| 7 | Where bind lives | One universal bind — same operation, picker and legality from the room, ⌘K and Task-page Session rows, even at extra cost | Jack: "worth making the design and architecture simple and universal if it takes a little extra" |
| 8 | Completed-Task branch binds; new `AttributionSource::Inferred` | Explained "bind" to Jack (assign a running Session's ancestry after launch via a sidecar; name/id/panes/Flow membership untouched; reversible). Defaults yes and yes stand pending his read | Jack: "i think i need to understand what 'bind' means here" |

Open for later discussion (Jack): Wave-page launch (Q4).

**Bind mechanism revised.** Jack, verbatim: "Hmm ok i dont love the sidecar
record. can we just have a task field on Session that can start null?"
Decision (Jack's): no `session-work.json` sidecar; the Task is a nullable
field on the Session. Where that field is stored was my proposal (the Run
manifest's `subjects`, updated in place), not Jack's words; the
[infra review](data-model-review.md) instead recommends a `sessions` store
row with nullable `task_id`/`wave_id`, `work_source`, and name + provenance
as columns, with rename and bind as the same `UPDATE`. Either way each
binding carries a source (`declared` / `inferred` / `bound`), which answers
Q8 without a second record. Row vs manifest is open for Jack.

**Data model review requested.** Jack, verbatim: "The fact that you didnt
design in this way to start makes me think we need some sort of data model
review." A read-only review Run is writing
[data-model-review.md](data-model-review.md) ahead of the S6 implement.

**Run owns the field.** Jack, verbatim: "Run.task = Session.task I guess.
Runs should have tasks too." Decision (Jack's): the nullable Task lives on
Run; a Session projects its Run's field; bind sets `Run.task`. Headless Runs
carry the same field, so `runs --task` and the Session list read one value.
The review's proposal must be checked against this when it lands.

**Main objects are main tables; Session is a child of Run.** Jack, verbatim,
on the finding that Session is four projections over four stores: "This
seems wrong. Sessions should be a child of Run. The main user objects should
line up with the main tables in the DB and when we see stuff like this where
a main record is actually a union over 4 things, we should be suspicious."
Decision (Jack's): `runs` is written as the Run record (typed nullable
`task_id`/`wave_id`, source); `sessions` is its child table (`run_id` FK,
kind, title + provenance, state); Flow review and Ask Sessions become rows
keyed by their existing `session_run_id`; the manifest stays launch
evidence; rename and bind are `UPDATE`s; every sidecar goes. This closes the
row-vs-manifest, one-id-scheme and `runs`-table questions in
[data-model.md](data-model.md). Presentation:
https://claude.ai/artifact/5QiVvD9jBQH7uVrZw2fE8b

**No occurrence object; Flow invocations own Runs; Run has a nullable
invocation.** Jack, verbatim: "Not sure what a step occurrence is, can we do
without that as an explicit object? Also shouldn't flow invocations own some
of the runs?" then "so a Run should also have a nullable flow (flow
invocation)." Decisions (Jack's): an occurrence is three values on the Run
(invocation, node, iteration tuple), not an object; `Run.invocation` is a
nullable FK set by the Flow driver at launch, null for independent Runs; a
Session's Flow membership is its Run's invocation + node, structural rather
than inferred. Validators: `invocation ⇒ task ⇒ wave`, constructor fills
upward, mismatch refused.

Jack, verbatim: "And I think maybe flow invocation has a nullable flow
invocation? we need something for nested loops right." Decision (Jack's):
`invocation.parent` is nullable; each nesting level is its own invocation
with its own cursor and return counts; the node path string and the
list-of-lists iteration tuple are replaced by a walk to the root. Jack, verbatim: "there's also the potential parentage you could have with
flows within flows (in their templates). However flow invocations should
always be fully unrolled from that perspective." Decision (Jack's): template
composition is definition-time parentage only; an invocation is the fully
unrolled path, and `invocation.parent` models runtime nesting (a loop body
entered on a pass), never the template tree. My earlier note that
composition might move onto the invocation tree is withdrawn.

**Two Flow views: template before, invocation after.** Jack, verbatim: "At
the UI level I think we may need to have separate UIs for flow invocations
and flows. so tasks mostly show flows (i.e. minimally unrolled by default,
but progressively disclosable), but once its running it goes into the flow
invocation mode which is fully unrolled." Decision (Jack's): the Task page
shows the Flow template until a Run exists — composed sub-Flows folded,
disclosable to the full step list — and switches to the fully unrolled
invocation view once running. Today both states draw the same flattened
13-node graph; this is a new UI slice (S9 in ux-plan.md) that depends on the
engine keeping template structure past load.

**Chapter is a first-class object.** On the page's line "chapter is internal,
never navigated", Jack, verbatim: "This is just because we're not that far
along. at a data model, clearly chapter history needs to be investigable,
and you need ways to create synchronized new chapters for all your waves or
some subgroup of waves." Decision (Jack's): Chapter (today's internal
Project) is a model object: a Wave has 0..n chapters, one current; a chapter
owns Tasks, KRs and metric targets; history is navigable (Wave → chapter →
plan, read-only when past); `new_chapter` rotates a set of Waves together.
The "never navigated" rule is a current-UI scope limit, not a model rule.
Denormalization to guard: `Task.wave` is derivable via `Task.chapter`.

Then, Jack, verbatim: "Maybe Chapter is at the repo level and Project is at
the wave level, and we just always do fully synchronous chapter
incrementing." Proposal (Jack's, marked maybe; applied on the page as the
working model): Chapter is a repository-level clock, one current for the
whole repo, incremented for all Waves at once; Project is the cell at
(Wave, Chapter) owning that Wave's Tasks, KRs and targets for that chapter;
a Wave with nothing to do has an empty Project. Validators:
`Project(wave, chapter)` unique; `Task.wave` derivable via `Task.project`.

Jack, verbatim: "I think it is odd for example to have flows on Projects."
Decision (Jack's): the recommended Flow leaves the chapter plan and becomes
a Wave default that a Task may override; Project holds only what changes per
chapter — Tasks, KRs, metric targets. Today the chapter recommends the Task
Flow (`lf task run --flow` doc), so this is a small migration.

Jack, verbatim: "then i think we can say 'recommended flow' is just a
project's flow, 'Flow position' can be replaced with 'Flow Invocation' (or
perhaps another name, but combine both the serialized, unrolled flow along
with its cursor)." Decision (Jack's): Flow invocation is one object holding
the serialized unrolled graph plus its cursor and return counts; today's
`task_flow_positions` row and `flows/<uuid>/position.json` fold into it;
`PinnedTaskFlow`/`FlowPosition` go. On "recommended": read (mine, pending
Jack) as dropping the word — a Task has a Flow, inherited from the Wave
default until overridden; the only distinction is Flow (template) vs Flow
invocation (running). Jack then, verbatim: "Now I'm saying PROJECTS have a flow and tasks have a
flow invocation. not sure on verbiage, also open to flow invocation is
called flow and then maybe flow is template?" Decision (Jack's, supersedes
the Wave-default reading and the earlier "odd to have flows on Projects"):
Project owns the Flow (template) for its chapter; Task owns Flow
invocations (0..n, one current); "recommended" disappears. Open: whether a
Task may invoke a different Flow than its Project's (`lf task run --flow`
does today). Naming: default keeps "Flow" for the template and "Flow
invocation" for the run (no CLI rename); the alternative "Flow template /
Flow" is open.

Jack, verbatim, on whether a Task may invoke a Flow other than its
Project's: "Yes, this kind of thing should definitely work. Tasks should be
smart so by default they grab from project, but trust our users — if they
want something different, give it to them." Decision (Jack's): the
Project's Flow is the default; `lf task run --flow` and Stop & restart may
invoke any Flow; the invocation records which Flow it unrolled. No guard.

**Docs first.** Jack, verbatim: "Make sure the first piece of work for this
project btw is rewriting the architecture docs and user-facing docs to fit
this new mental model." Recorded as D0 step 1 in [ux-plan.md](ux-plan.md).

**Old Runs: one-time rewrite, clean code.** On whether pre-migration Runs
keep their Task through a rewrite from `subjects`, Jack, verbatim: "Yeah
hack my computer if need be, keep the codebase clean." Decision (Jack's):
a one-time migration populates `runs.task_id`/`wave_id` from the old
manifest subjects and drops the sidecars on the Homes on his machine; no
code path reads `subjects` or sidecars afterwards. Local data may be
hand-repaired if the migration cannot; the codebase carries no shim.

**Green light.** Jack, verbatim: "ok i give you the green light to pursue
this design." The model on the presentation page (v13) and in
[data-model.md](data-model.md) is approved for implementation in the D0
order: docs → tables and readers → S6, S2 predicate, perf #1/#2, S9. The
remaining defaults stand as decisions unless Jack redirects: `started` is
derived from Runs; bind to a done or landed Task is allowed; usage
attribution follows the field; headless Runs may carry a name; "Flow" is
the template and "Flow invocation" the run.

**Deletion research at the end.** Jack, verbatim: "make sure there's a
research step at the end which is 'what code can we delete now that we've
redesigned around this new data model and API'." Recorded as D0 step 4 in
[ux-plan.md](ux-plan.md).

**Invariants on denormalized fields.** Jack, verbatim: "We need something
like the verifications you get on Pydantic to enforce constraints. In this
case for example Run.wave should always == Run.task.wave. Maybe other things
like that wherever there are denormalizations." Requirement (Jack's): every
denormalized pair in the model is either removed or guarded by a validator at
construction/write time so an inconsistent record cannot exist. Both data
model reviews must enumerate the pairs; the S6 implement must ship the
validators with the field.

**Task implies Wave; nothing else is required.** Jack, verbatim: "I think
maybe ok to have Run.task = null but Run.wave != null" then, correcting my
reading, "Wave is not required, but Wave doesn't imply task, just task
implies wave." Decision (Jack's): both `Run.wave` and `Run.task` are
nullable. The valid states are (none, none), (wave, none) and (wave, task);
(none, task) is invalid, and when both are set `Run.task.wave == Run.wave`
is enforced. Jack, verbatim: "I think ok if constructor infers wave" — so
given a Task and no Wave the constructor fills `wave` from the Task rather
than refusing; given both, a mismatch is refused. Consequence (mine): a Wave-only Run is a Wave conversation; a
(none, none) Run is a repository conversation; both are orphans in the
sidebar sense, and the orphan section can group by Wave where one exists.

## Agreed direction

Jack redirected the Task from "accept the demo" to a fresh UX pass with three
studies first, then a plan, then delegated execution:

1. Prototyping retrospective — what improved across the Task, what Jack liked
   in the mocks, where mock → native lost it. → `scratch/retro/prototyping-retro.md`
2. State of the art — Notion, Linear, Figma, Claude, OpenCode, Codex: what
   they agree on, disagree on, and what Loopflow uniquely needs.
   → `scratch/retro/state-of-the-art.md`
3. Front-end performance — measurement of cold start, Wave/Task/Session
   clicks, typing latency, memory; infrastructure for tests and for real use
   on Jack's machine, no telemetry. → `scratch/retro/frontend-performance.md`

Then: a UX plan emphasising information architecture, visual style and
snappiness, executed by `lf` Runs managed from this Session. Jack asked for
this explicitly; it is agreed, not proposed.

## Unresolved

- Whether the accepted D combination is still the target or the mocks
  themselves get revised ("we can even improve on the mocks").
- The two-loop Flow composition at 1100pt.
- Every configured-input/retention proof in [questions.md](questions.md).

## Next action

Write `scratch/ux-plan.md` from the three studies, then run design → implement
→ compress → review-slice Runs per slice. Proof for each slice: a native
capture compared against the mock (or the revised mock), not a test count.
