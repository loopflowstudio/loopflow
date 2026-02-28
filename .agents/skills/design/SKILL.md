---
name: design
description: Help the user dream big, detail the idea fully, then decide whether to implement or plan as a wave.
loopflow: true
---
Help the user dream big, detail the idea fully, then decide whether to implement or plan as a wave.

**Start by asking what they want to build.** Don't start writing or exploring until you understand the goal. This is a conversation.

If on main, create a feature branch first: `git checkout -b <feature-name>`.

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

Either signal suggests breaking into a wave plan. Bias toward "yes it fits" when it's close—single commits are preferable. But these are heuristics, not rules. The user can override.

### Phase 4: Fork

Present the size assessment to the user and ask explicitly: **implement or wave plan?**

- "This looks like it fits in one commit—ready to implement?" or
- "This is bigger than one commit—want me to break it into a wave plan?"

This is the natural session exit point. The user's answer determines what to run next.

**If implement:**

1. Tighten the scratch doc into the standard design spec (see "Design doc sections" below)
2. Run `git add scratch/ && git commit -m "design: <branch>"`
3. End session and tell the user to run `lf implement`

**If wave plan:**

1. Break the idea into staged wave items
2. Choose a wave name and create `wave/<name>/`
3. Write `wave/<name>/README.md` using the wave content model:
   - `## Vision` — from the Dream phase conversation
   - `## Goals` — concrete objectives from the Detail phase
   - `## Risks` — unknowns and failure modes surfaced during detailing
   - `## Metrics` — numeric measurements (counts, percentages, durations), not qualitative indicators
   - Include `### Not here` under Vision when scope boundaries are important
4. Write `wave/<name>/<name>.yaml`:
   - `flow`: default `ship-wave` unless user asks for something else
   - `area`: inferred from the files/directories discussed (default `["."]`)
   - `direction`: inferred from conversation perspective (optional)
   - `stimulus`: ask if needed; omit for manual runs
5. Write roadmap files as `wave/<name>/01-*.md`, `02-*.md`, ... — one stage per file
6. The first stage becomes the design doc for this branch (`scratch/<branch>.md`)
7. Run `git add scratch/ wave/ && git commit -m "design: <branch>"`
8. End session and tell the user what to run next:
   - `lf implement` (for stage 1)
   - `lf ship-wave`

Once breaking things up, be aggressive about commit boundaries—each stage should be independently shippable.

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

## Design doc sections

When the idea fits in one commit (~1000 words max):

- **What to build** — One sentence. What exists after this that doesn't exist now.
- **Data structures** — Core types, sketched in code.
- **Key functions** — Signatures with one-line intent.
- **Constraints** — What would require rewriting if guessed wrong.
- **Done when** — Verification command and expected output.

## Conversation guidance

**Ask first, write second.** Start with "What are you trying to build?" Don't read files or start drafting until you understand the goal.

**Dream big, detail fully, then size-check.** Don't constrain scope until the idea is fleshed out. Detailing the whole idea often reveals which parts matter most—that's what makes the eventual scoping good.

Completeness is not required. Wrong guesses get fixed in implementation. The goal is to not block the next session, not to predict everything.
