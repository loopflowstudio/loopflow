---
interactive: true
requires: none
produces: scratch/<branch>.md | wave/<name>/
action_style: exploratory
---
Help the user dream big, detail the idea fully, then decide whether to implement or plan as a wave.

## Orientation

Before starting, orient yourself in this branch:

- Read `scratch/` — design docs and notes for the current work live here
  (`scratch/<branch>.md` is this PR's design; `scratch/questions.md` holds open
  questions and assumptions).
- Read wave/PM context only when the seed names the exact wave, task, project,
  or a concrete coordination question; never infer it or repair access as a
  prerequisite.
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
  waiting for a human. Choose implement or wave using the size criteria below
  and state the assumptions behind the choice. Send the resulting decisions to
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
3. Check `wave/` for existing waves, architecture notes, or context that informs this design
4. Create `scratch/<branch>.md` early—after the first exchange or two

## Workflow

Four phases. Don't skip ahead—dreaming and detailing come before scoping.

### Phase 1: Dream

Ask what they want to build. Let them describe the full vision. No scope pressure. Don't ask "what's the smallest version?"—that comes later. Explore the idea freely.

### Phase 2: Detail

Walk through components in detail. Data structures, functions, interactions, edge cases. This may take a while—keep writing to `scratch/<branch>.md` as you go so nothing gets lost if the session crashes.

Write as you go, not at the end. Let writing inspire questions—gaps become obvious when you make things concrete.

### Phase 3: Size-check

After the idea is fully detailed, evaluate two signals:

1. **Design doc size** — is the spec exceeding ~1000 words? If the design itself is big, the implementation will be bigger.
2. **Implementation size** — would this be ~1000+ LOC? That's generous for a single commit.

Either signal suggests breaking into a wave. Bias toward "yes it fits" when it's close—single commits are preferable. But these are heuristics, not rules. The user can override.

### Phase 4: Fork

Present the size assessment to the human reviewer and ask explicitly:
**implement or wave?** When a parent reviewer is assigned, make that decision
from the evidence and record why.

- "This looks like it fits in one commit—ready to implement?" or
- "This is bigger than one commit—want me to break it into a wave?"

This is the natural session exit point. The user's answer determines what to run next.

**If implement:**

1. Tighten the scratch doc into the standard design spec (see "Design doc sections" below)
2. Run `git add scratch/ && git commit -m "design: <branch>"`
3. End session and tell the user to run `lf implement`

**If wave:**

1. Choose a wave name and create `wave/<name>/`.
2. Write `wave/<name>/GOAL.md` — the wave's identity and anchor:
   - frontmatter: machine config only (`crons` and, once connected,
     `pm.linear_initiative`)
   - body (the loop prompt): Objective, Measures, Cron if any, and Process. Put
     routing judgment in Process, not frontmatter.
   - **No roadmap table, no status indicators, no item lists** — tasks live in
     Linear.
3. Write `wave/<name>/MEMORY.md` — seed it with the load-bearing context from the
   Detail phase (key decisions, constraints, what's known). Short is fine.
4. Connect Linear with `lf pm init --wave <name>`. It links or creates the
   Initiative and writes `linear_initiative` into `GOAL.md`.
5. Create each measured bet with `lf pm project create`. A project is either a
   completable behavioral improvement or a standing quality frontier. Each
   belongs to this wave, has no child projects, and carries a definition plus
   proof-shaped KRs in Linear Project content.
6. Seed tasks in Linear:
   - File the opening items with
     `lf pm task create --project <project> --title "…" --notes "…"` — the urgent
     and next-step work, one task each. Tasks start in Linear, not on disk.
7. The first item you expect to build now becomes the design doc for this branch
   (`scratch/<branch>.md`).
8. Run `git add scratch/ wave/ && git commit -m "design: <branch>"`.
9. End session and tell the user what to run next:
   - `lf implement` (for the immediate item)
   - `lf ship`

Once breaking things up, be aggressive about commit boundaries—each task should be independently shippable.

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
