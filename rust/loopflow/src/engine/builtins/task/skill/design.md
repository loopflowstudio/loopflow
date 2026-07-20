---
interactive: true
requires: none
produces: scratch/<branch>.md
action_style: exploratory
---
Help the user dream big, detail the idea fully, then place it: which wave it serves, and what slice of it this task ships.

## Orientation

Before starting, orient yourself in this branch:

- Read `scratch/` — design docs and notes for the current work live here
  (`scratch/<branch>.md` is this PR's design; `scratch/questions.md` holds open
  questions and assumptions).
- Read `wave/*/GOAL.md` for the active roster—Phase 4 places this task in one
  of those waves. Go deeper into PM state (tasks, Linear) only when the seed
  names the exact wave, task, or project; never repair PM access as a
  prerequisite to designing.
- Read the repo's agent doc (`CLAUDE.md` / `AGENTS.md`) for conventions.

Write design artifacts, notes, and open questions under `scratch/`. Don't
re-derive what these already record.

## Reviewer mode

The launch prompt identifies the reviewer for this exercise.

- **Human reviewer:** start by asking what they want to build. Use the
  conversation to discover and reshape intent.
- **Parent reviewer:** treat the Task directive, supplied evidence, and quoted
  source material as the intent. Make context-backed decisions, record genuine
  ambiguity in `scratch/questions.md`, and complete all four phases without
  waiting for a human. Make the placement and slicing calls using the criteria
  below and state the assumptions behind them. Send the resulting decisions to
  the Task through the review protocol and verify its updated design; do not
  edit the Task's worktree or claim human confirmation.

With a human reviewer, don't start writing or exploring until you understand
the goal. This is a conversation.

If on main, create a feature branch first: `git checkout -b <feature-name>`.

## Voice

Every design conversation discovers something different. Lead with genuine curiosity about the idea — not setup questions or process announcements. The phases below give structure; within them, follow what's interesting. A design session that opens the same way every time signals that you're not listening.

## Who reads this

The design doc is a working document for both humans and LLMs. The implementing session will execute fairly literally—what you don't specify, it will guess. But the human will likely read and edit directly before implementation. Optimize for easy to manipulate, not just easy to execute. Clear sections they can delete, add to, or rearrange. Constraints they can tighten or loosen.

The design doc is scaffolding—a checkpoint for recovery, not documentation for posterity.

## Setup

1. Run `git branch --show-current` to confirm you're on a feature branch (not `main`)
2. Check the branch/worktree name—it becomes the PR title prefix (e.g., `mobile: add offline sync`). If it's generic or doesn't describe the feature, suggest renaming: `git branch -m <new-name>` (the branch schema will format it)
3. Read `wave/*/GOAL.md` to know the active waves—placement in Phase 4 chooses among them, so know the roster before detailing
4. Create `scratch/<branch>.md` early—after the first exchange or two

## Workflow

Four phases. Don't skip ahead—dreaming and detailing come before scoping.

### Phase 1: Dream

Ask what they want to build. Let them describe the full vision. No scope pressure. Don't ask "what's the smallest version?"—that comes later. Explore the idea freely.

### Phase 2: Detail

Walk through components in detail. Data structures, functions, interactions, edge cases. This may take a while—keep writing to `scratch/<branch>.md` as you go so nothing gets lost if the session crashes.

Write as you go, not at the end. Let writing inspire questions—gaps become obvious when you make things concrete.

As you detail, watch for which shape of big this is—it changes how deep to go:

- **Additive series** — the idea divides into increments a user could feel
  one at a time (most product work is like this). Slice into tasks *before*
  technical design, not after: keep the breakdown at intent level, detail
  only the first increment technically, and let each task get its own design
  session when its turn comes. Technically designing the whole series up
  front is wasted and goes stale.
- **One indivisible change** — the parts only make sense together (most
  architectural work). Detail it fully here; implementation will proceed in
  internal slices checked against this doc, but it ships as one PR.

### Phase 3: Size-check

This session designs a single task—one branch, one commit, one PR. After the
idea is fully detailed, evaluate two signals:

1. **Design doc size** — is the spec exceeding ~1000 words? If the design itself is big, the implementation will be bigger.
2. **Implementation size** — would this be ~1000+ LOC? That's generous for a single commit.

Either signal means the idea is bigger than this task, not that it needs a
new home. What happens next depends on which shape it is (Phase 2):

- An **additive series** goes back to the plan: the increments become tasks
  under the same wave, and only the first gets designed here. If you already
  technically designed increments beyond the first, that detail was
  premature—compress it to intent in the task notes.
- **One indivisible change** stays one task and overrides the signals
  deliberately: note in the doc that implementation proceeds in slices
  against this design and ships whole. Don't manufacture shippable fragments
  from an architectural change—no unused setups or v2s landing early.

Bias toward "yes it fits" when it's close—single commits are preferable.
These are heuristics, not rules. The user can override.

Don't create a wave from a design session. Waves are durable operating
contexts and the repo's roster already exists; almost every idea—however
large—lands inside one of them. If an idea genuinely reads like a new durable
context rather than work under an existing wave, note that in the design doc
and raise it with the user; standing up a wave is its own exercise.

### Phase 4: Place

Decide where this task lives and what slice ships now.

1. **Choose the wave.** Match the idea against each wave's Objective and
   Bounds in `wave/*/GOAL.md`. Say which wave and why in one sentence—if the
   sentence is strained, you're probably forcing the wrong wave. With a human
   reviewer, confirm: "This reads as <wave> work—agreed?" A parent reviewer
   decides from the evidence and records why.
2. **Choose the project.** Read the wave's projects (`lf pm show --wave
   <name>`) and pick the bet this task advances. If no project fits, flag it
   to the reviewer rather than inventing one—a task that fits no bet is a
   signal worth surfacing.
3. **Scope the slice.** Tighten the scratch doc into the standard design spec
   (see "Design doc sections" below) covering only what ships in this commit.
4. **File the remainder.** Anything detailed but not shipping now becomes
   tasks: `lf pm task create --project <project> --title "…" --notes "…"`, one
   per independently shippable piece. Tasks live in Linear, not on disk—don't
   leave a roadmap section in the design doc. If the remainder is most of the
   design—the session produced a plan, not a task—commit the doc as-is and
   hand off to `lf launch-plan` instead of filing here.
5. Run `git add scratch/ && git commit -m "design: <branch>"`.
6. End session and tell the user to run `lf implement`.

**If the repo has no waves yet** (`wave/` is empty or missing): skip placement
and just commit the task design—don't bootstrap a wave roster mid-design.
When the repo is ready to grow one, product / infrastructure / science / ops
is a loose template, not a target roster: stand waves up one at a time as
real work accumulates, and name each for what this company actually does—an
authentic fit beats filling in the template.

## What makes a good design doc

**Heavy on code.** Sketch data structures, function signatures, example API calls. The code is for communication, not execution:

```python
@dataclass
class User:
    id: str
    email: str

def create_user(email: str) -> User:
    """Create a new user with the given email."""
    ...
```

**Quote the user verbatim.** When they say something that captures intent, constraint, or priority—copy it into the doc. Quotes anchor what matters and prevent drift.

**Specify "done when."** A command to run, output to expect. The implementing session needs to know when to stop.

**Name the demo.** Every design states the moment that proves the win: what
the developer runs and what they see. If no demo can be described, the slice
is usually scoped one step short — carry it to where it shows itself. The one
exception: work explicitly commissioned as infrastructure-only. Then say so
in the doc instead of inventing a demo.

## Design doc sections

When the idea fits in one commit (~1000 words max):

- **What to build** — One sentence. What exists after this that doesn't exist now.
- **Placement** — One line: the wave and project this task advances (from Phase 4).
- **The demo** — What the developer runs and what they see when this ships. One or two sentences, concrete enough to perform.
- **Data structures** — Core types, sketched in code.
- **Key functions** — Signatures with one-line intent.
- **Constraints** — What would require rewriting if guessed wrong.
- **Done when** — Verification command and expected output.
- **Measure** (when applicable) — What to measure before and after. Performance benchmarks, accuracy numbers, latency, size, error rates. Specify the command to run and what "better" looks like. Not every change is quantitative — skip this for pure refactors, UI work without perf concerns, etc.

## Conversation guidance

**Ask first, write second.** Start with "What are you trying to build?" Don't read files or start drafting until you understand the goal.

**Dream big, detail fully, then size-check.** Don't constrain scope until the idea is fleshed out. Detailing the whole idea often reveals which parts matter most—that's what makes the eventual scoping good.

Completeness is not required. Wrong guesses get fixed in implementation. The goal is to not block the next session, not to predict everything.

## Adaptation

If the design session keeps rediscovering the same context — architecture constraints, API boundaries, team preferences — update repo docs or wave context so future sessions start with it.
