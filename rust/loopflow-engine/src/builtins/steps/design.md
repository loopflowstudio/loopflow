---
interactive: true
requires: none
produces: scratch/<branch>.md
---
Produce a short implementation spec that another LLM session can use to write a first draft.

If on main, create a feature branch first: `git checkout -b <feature-name>`.

## Who reads this

The design doc is a working document for both humans and LLMs. The implementing session will execute fairly literally—what you don't specify, it will guess. But the human will likely read and edit directly before implementation. Optimize for easy to manipulate, not just easy to execute. Clear sections they can delete, add to, or rearrange. Constraints they can tighten or loosen.

The design doc is scaffolding—a checkpoint for recovery, not documentation for posterity.

## Workflow

1. Run `git branch --show-current` to confirm you're on a feature branch (not `main`)
2. Check `reports/` for architecture notes, prior decisions, or context that informs this design
3. Create `scratch/<feature-name>.md` early—after the first exchange or two
4. Write as you go, refining with each conversation turn
5. Run `git add scratch/ && git commit -m "design: <branch>"` when done
6. End session. Implementation happens separately.

Write as you go, not at the end. If the session crashes mid-design, the partial doc is still useful. Let writing inspire questions—gaps become obvious when you make things concrete.

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

## Required sections (~1000 words max)

- **What to build** — One sentence. What exists after this that doesn't exist now.
- **Data structures** — Core types, sketched in code.
- **Key functions** — Signatures with one-line intent.
- **Constraints** — What would require rewriting if guessed wrong.
- **Done when** — Verification command and expected output.

## Conversation guidance

Bias toward brevity. Ask only what's needed to start coding.

If scope is unclear, ask "what's the smallest version that's useful?" and spec that.

Completeness is not required. Wrong guesses get fixed in implementation. The goal is to not block the implementing session, not to predict everything.
