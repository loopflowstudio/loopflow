# Explore: the Task conversation that shows several visions before building

Status: **draft** (2026-10-24). Nothing here is accepted except the decisions
attributed to Jack below.

## What to build

A builtin `explore` skill. It is the person's node between kickoff and pursue
in the interactive `feature` Workflow. It shows several distinct visions of the
outcome, in a product-shaped Task as a self-contained HTML prototype, so the
person can say "this one, build it."

## Why

Jack (2026-10-24), verbatim anchors:

> the main goal of this step is to encourage ideation: exploring, brainstorming
> multiple directions, presenting multiple visions of a direction to the human,
> and allowing the human to say, "Okay here's what I like. Go and build this thing."

> I wanted to encourage the agent to build an HTML prototype that offers three
> visions of the world. That could be a version of radical success for the product

> we should encourage the agent to take it as inspiration or as poetry, more
> than as regulation

> I do kind of want this to be the default for a task that is interactive

When a Task is filed it gets designed a little. Kickoff then turns that into a
plan, which "may not be very good." Today the person's node after kickoff is
`review-design`. That skill edits one plan in place, so the person only ever
reacts to the single direction kickoff already picked. Explore widens that
choice before anything gets built.

The standalone `design` skill (no Task) stays as it is (Jack, 2026-10-24).

## The demo

```bash
lf task run LOO-123            # feature workflow: kickoff, then the explore node
```

The Task conversation opens at `explore`. The agent writes
`scratch/explore.html` and opens it locally. The page holds three visions side
by side, each a clickable mock of the experience. Jack says "keep B's layout,
give me three takes on how history appears." The page regenerates with three
new variants. A couple of rounds later he says "B with the second history,
build it." Along the way the design records each thing he settled, with his
name and the date.

```bash
lf task run LOO-123 pursue     # build the chosen vision
```

## Shape of the skill (`task/skill/explore.md`)

Proposed frontmatter:

```yaml
requires: a Task brief or kickoff plan
produces: a chosen direction recorded in the design, with the visions it was chosen from
default_agent: claude
action_style: exploratory
```

Content of the body, in order:

1. **Read what exists.** The Task, the kickoff plan, the risks it found. Kickoff's
   findings are constraints on the visions; its chosen approach is only one
   candidate.
2. **Dream three visions.** Each is a coherent world, not a variant of
   presentation. They should differ by mechanism or by the experience they
   promise. At least one is radical success: what this would be if it worked far
   beyond the brief. Name each with a short title and one line on what's true
   in that world.
3. **Make them tangible.** For work with a human-facing experience, build one
   self-contained HTML file (`scratch/explore.html`): inline styles and script,
   no network, one panel per vision, real-looking content, and the key
   interaction working. Open it locally. For other work, pick the form that
   makes the difference visible: CLI transcripts, data-model sketches, a
   day-in-the-life of the system, the failure each one avoids. The three-vision
   HTML is inspiration, not a rule; a Task that really has one obvious answer
   says so and spends the time on the open question instead.
4. **Follow their lead.** The first page is an opening move, not a ballot.
   The person may pick one outright, ask for A + B, or say "three more, keep X
   fixed and vary Y." That can repeat for many rounds. Each round answers the
   request as given and regenerates the page. Don't steer toward closure,
   summarize a choice they haven't made, or turn a remark into a decision.
   Keep each vision's cost and risk visible next to its appeal.
5. **Record what they've settled, as they settle it.** Write into the design
   only what the person has actually decided, attributed and dated, and keep
   open threads open. Once they say "build this," the design reflects that
   direction well enough for pursue, with kickoff-level detail (slices, Done
   when) updated where the direction changed it. Keep the latest page as
   evidence, linked from the design.

Headless: produce the visions and the page, recommend one, and leave the choice
open in `scratch/questions.md`. Never record acceptance.

## Workflow change

`task/workflow/feature.yaml`:

```yaml
nodes:
  explore: { skill: explore, description: you choose what to build }
  demo: { skill: demo, description: you try the change }
edges:
  - { from: start,   to: explore, graph: task-design }
  - { from: explore, to: explore, graph: task-design }
  - { from: explore, to: demo,    graph: pursue }
  - { from: demo,    to: demo,    graph: pursue }
  - { from: demo,    to: end,     graph: ship }
```

## Secondary goal: a page is what you review

Jack (2026-10-24):

> most or all of our interactive steps in built-in workflows should involve the
> agent creating an HTML thing. That's what you look at rather than this task
> session

> it's also just sort of the contract of interactions for interactive-level
> steps: you get presented with something for you to review and then you talk
> about what you see

The contract: at every person node, the agent makes a page. You look at the
page, and the conversation is about what you see there. The terminal or Task
session is where you talk, not where you read. `pr-review` is already shaped
this way and is the stylistic reference.

### Where each interactive skill stands today, and what its page would be

| Skill (node) | Today | The page |
|---|---|---|
| `explore` (feature) | new | Three visions side by side, each a working mock or the fitting non-UI form. Ends with "which, and what to steal from the others." |
| `demo` (feature, code) | Walks you through in chat; writes a Markdown note | A runbook for what changed: each action to take, with its expected result beside it and a slot for what you saw. For UI, before/after screenshots. Failures stay on the page. The Markdown note becomes the page. |
| `research` (findings) | "Answer here" in chat, or a scratch note | The answer first, then the system map as a diagram, sources linked to the claims they support, counterexamples, open questions. Its "don't force a report shape" stays. |
| `concept-review` | "Show the user's actions, the few concepts… and the APIs… in one screen" | That screen, written literally: actions → concepts → APIs, before and after, with the one consequential mismatch called out. The skill already describes a page without naming one. |
| `design` (no Task) | "show the exact final artifact"; Markdown only | Dream phase can use the explore page; the final artifact stays Markdown (the plan is for agents). |
| `pr-review` | Already HTML | The reference style. Unchanged. |
| `refine`, `capture-tasks`, `prompt` | Text-native | No page; the text is the artifact. Listed so the omission is deliberate. |
| `task-session` | Status in chat | Open: should a Task have one living page (position, latest page per node, what's next)? Not in this change. |

### Shared page guidance (draft)

Distilled from pr-review and extended:

- One self-contained `.html` under `scratch/`: inline CSS/JS, no network, no build
  step. Escape code. Readable in light and dark, and at narrow widths.
- Lead with the decision you need from the person. End with the questions,
  numbered, so "2b" is a complete answer.
- Show the thing, not a description of it: real-looking content in mocks,
  real snippets, real command output. Mark proposed vs. existing behavior visibly.
- Editorial restraint: one reading column, generous whitespace, no dashboard
  chrome or badges. Vary the structure to the content; no fixed template.
- Header: Task, node, date, and the files it's built from.
- Write it and say its path. Desktop shows it in Files; in a bare terminal the
  agent opens it in the browser. Regenerate the same path when the conversation
  changes it, so the view shows the new state.
- The decision itself is recorded in the Markdown design/note, never only in
  the page. The page is a view; `lf scratch clear` removes it on land.

### Where you look: Files, reading-first, following scratch

Decided by Jack, 2026-10-24: "lets do follow scratch. md and html only."
Earlier anchors: "this could just be viewing a scratch/ html file"; "having our
file browser be smart and read-ux-biased with displaying html, markdown etc."
Jack confirmed there's no embedded web browser, only the Files spot.

Observed: every Task workspace already opens Files beside the Session
(61.8/38.2), and Files lists `scratch/`. Files shows every document as editable
text (File / Diff modes). Nothing in `swift/` imports WebKit.

Desktop change:

- **Reading-first.** A document opens rendered: `.html` as a live page, `.md`
  through the existing `MarkdownBlocks`. Edit (today's File mode) and Diff are
  modes you switch into.
- **Follows scratch.** When a `scratch/**/*.md` or `scratch/**/*.html` file is
  created or rewritten while the Task workspace is open, Files selects it and
  shows it rendered. Other extensions and non-scratch files never auto-select.
- **Takes focus only from stale attention.** Jack (2026-10-24): a write "can
  grab focus when we think your file focus is stale/unimportant"; otherwise it
  "opens tab without selecting it." A write selects the file when your current
  Files view is unimportant. Otherwise it opens the file's tab in the background
  (or leaves its existing tab where it is). Accepted by Jack 2026-10-24 (2-minute interval is a starting value to tune); all local view
  state:
  - **Never take:** Edit or Diff mode, an unsaved draft, or Files holding
    keyboard focus.
  - **Take:** nothing selected yet; the current document was itself
    auto-selected; or your last interaction with Files (select, scroll,
    click, find) is older than a quiet interval (proposal: 2 minutes) while you've
    been in the conversation.
  - Otherwise the tab opens unselected.
  A write to the file already showing just reloads in place and keeps the
  scroll position.
- **Live.** Rendered views reload on disk change, so regenerating the page each
  round shows up beside the conversation.
- **HTML sandbox.** JS allowed (mocks need interaction); network loads blocked.
- **WebKit.** The deployment target is still macOS 15 (CI tests on
  `macos-15`), so pages render through `WKWebView` in an `NSViewRepresentable`
  with a content rule list blocking network schemes. Moving to SwiftUI's
  `WebView` waits for the macOS 26 target change.

No skill field, launch record, DTO field or path convention: agents just write
files. Rejected (2026-10-24): a `presents:` frontmatter field that names the
contract file per skill. It was precise but added a field, a launch record and a
wire DTO field.

Accepted cost: mid-explore, an edit to the Markdown design takes the view from
the HTML page, unless you've picked a document since. Skills that present a page
should write the page last in a round, after updating the design, so the view
ends on the page.

### Where the shared page guidance lives (open)

With no declaration to hang it on, two placements remain:

- **A. Inline in each presenting skill** (explore, demo, research,
  concept-review; pr-review already has its own). Self-contained, matching the
  builtin rule, but ~10 lines in four places can drift.
- **B. At the Workflow node.** When `lf task run` opens a Session at a node, the
  launch adds the page guidance. That matches the "interactive-level step"
  boundary and covers custom Workflows for free, but ad hoc `lf skill demo`
  misses it. It needs a launch hook at the node; check `workflow.rs` and Task
  launch before choosing it.

`surfaces/human-present.md` stays out: it would reach `lf debug`.

## Open tensions (left for Jack)

1. **Before or after kickoff?** After kickoff (as proposed): the visions are
   grounded in risks kickoff already researched, but kickoff has already
   "committed to a direction" and Explore reopens it, and a different choice can
   leave kickoff's slices stale. Before kickoff: ideation comes first and kickoff
   plans only the chosen vision, but the visions aren't checked for
   feasibility. One possible reconciliation is to make kickoff stop at
   de-risking plus "candidate approaches" and let Explore commit. That reshapes
   kickoff too.
2. **Does `review-design` survive?** Explore takes its only Workflow slot.
   Choosing a direction and then refining it in the same conversation covers
   review-design's job. Under the delete-what-it-replaces rule, it goes (see
   below). The alternative is to keep it for Graphs that want a narrower "check
   this plan" step.
3. **Who re-plans after a choice?** Explore rewrites the design itself (as
   proposed), or the `explore → explore` edge reruns kickoff on the chosen
   vision. A self-loop that reruns kickoff would pick up the new choice with no
   new code.
4. **"Default for interactive Tasks."** `feature` is the only builtin with a
   person's design node, and no repository default is set here. Should
   `feature` become the implicit default when neither the Wave nor the repo
   names a workflow, or does Explore simply ride along wherever `feature` is
   chosen?
5. ~~Viewing the prototype~~: decided 2026-10-24 (Files, following scratch).
6. **Page guidance placement**: inline per skill or at the node (see above).

## Deletions (done under default 2 in `scratch/questions.md`)

`review-design.md`, its builtin listing (now asserted absent), and the
`docs/authoring.md`, `docs/config.md` and `PROMPTS.md` examples are replaced by
`explore`. Feature-Workflow fixtures in Rust now name `explore`. Pure string
fixtures (`DTOFixtureTests`, `SessionRenameTests`, the `session_membership_graphs.json` DTO fixture and its Rust test) and shipped records
(`store/migrations.rs:7052`, release notes) keep `review-design`.

## Forbidden outcomes

- Both `review-design` and `explore` competing for the same node.
- Explore turning into a template: always three, always HTML, even for a
  migration Task.
- A recorded "accepted" choice when no person chose it.
- Treating the first set of visions as a vote: pushing to pick, or ending the
  step after one round.

## Done when

- `cargo test -p loopflow builtins` passes with `explore` resolving as a builtin
  skill, and `feature` Workflow loading with an `explore` node.
- `lf task workflow show` on a fresh feature Task lists `explore` between
  `task-design` and `pursue`.
- Demo/review: Jack runs a real product Task through `explore` and picks a
  vision from the HTML page.

## Internal slices

Status (2026-10-24): Keystone and Desktop are implemented on this branch with
the defaults in `scratch/questions.md`: page guidance inline (option A) in
`explore`, `demo`, `research` and `concept-review`; Files opens in Read mode
(Read/Edit/Diff), renders `.md` through `MarkdownBlocks` and `.html` in an
offline `WKWebView`, and follows `scratch/` page writes via
`TaskFilesStore.follow`.

Remaining:

- Checks (2026-10-24): `scripts/test_desktop.sh --filter 'TaskFilesTests|ToolTabTests'`
  30 passed; `cargo test -p loopflow --lib -- builtins workflow graph::tests human_session`
  passed after keeping `review-design` in the shared DTO membership fixture test;
  integration `cmux_opens_the_task_conversation` passed. Full matrix → gate/CI.
- Demo: Jack runs a real product Task through `explore` and picks a vision
  from the page in Files.
- Jack's decisions on the open tensions; each default is revisitable in demo.
- Follow-up (open): a living Task page for `task-session`.
