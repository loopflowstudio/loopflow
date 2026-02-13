---
interactive: true
requires: none
produces: scratch/<branch>.md | scratch/roadmap-proposal.md
---
Help the user dream big, detail the idea fully, then decide whether to implement or roadmap.

**Start by asking what they want to build.** Don't start writing or exploring until you understand the goal. This is a conversation.

If on main, create a feature branch first: `git checkout -b <feature-name>`.

## Who reads this

The design doc is a working document for both humans and LLMs. The implementing session will execute fairly literally—what you don't specify, it will guess. But the human will likely read and edit directly before implementation. Optimize for easy to manipulate, not just easy to execute. Clear sections they can delete, add to, or rearrange. Constraints they can tighten or loosen.

The design doc is scaffolding—a checkpoint for recovery, not documentation for posterity.

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

Either signal suggests roadmapping. Bias toward "yes it fits" when it's close—single commits are preferable. But these are heuristics, not rules. The user can override.

### Phase 4: Fork

Present the size assessment to the user and ask explicitly: **implement or roadmap?**

- "This looks like it fits in one commit—ready to implement?" or
- "This is bigger than one commit—want me to break it into a roadmap?"

This is the natural session exit point. The user's answer determines what to run next.

**If implement:**

1. Tighten the scratch doc into the standard design spec (see "Design doc sections" below)
2. Run `git add scratch/ && git commit -m "design: <branch>"`
3. End session. User runs `lf implement` next.

**If roadmap:**

1. Break the idea into staged roadmap items
2. Write `scratch/roadmap-proposal.md` following the roadmap output format:

```markdown
---
status: proposed
---

# Title

One paragraph describing what and why.

## Context

What analysis led to this proposal.

## Scope

- What's included
- What's explicitly not included

## Stages

### Stage 1: ...
### Stage 2: ...
```

3. The first stage becomes the design doc for this branch (`scratch/<branch>.md`)
4. Run `git add scratch/ && git commit -m "design: <branch>"`
5. End session. User runs `lf add-to-roadmap` next to promote remaining stages.

Once breaking things up, be aggressive about commit boundaries—each stage should be independently shippable.

## Setup

1. Run `git branch --show-current` to confirm you're on a feature branch (not `main`)
2. Check `reports/` for architecture notes, prior decisions, or context that informs this design
3. Create `scratch/<branch>.md` early—after the first exchange or two

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
