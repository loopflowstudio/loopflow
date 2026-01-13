This task produces a short implementation spec that another LLM session can use to write a first draft of a feature.

The design doc is scaffolding. It's a checkpoint for recovery, not documentation for posterity. If a session crashes, the spec lets a fresh session pick up where things left off. After implementation, `lf review` writes the review under `.design/`. `lf pr land` removes the `.design/` contents.

## Workflow

1. Create a design doc under `.design/` (pick a short descriptive name, create the folder if needed) early — after the first exchange or two
2. Keep refining the doc as the conversation continues
3. Commit with message `design: <branch>` when done
4. End session—implementation happens in a separate `lf implement` invocation

Write as you go, not at the end. The doc is a living artifact during the conversation. If the session crashes mid-design, the partial doc is still useful. Let writing it inspire new questions—gaps become obvious when you try to make things concrete.

**Important:** Design docs live under `.design/` and are auto-included in the prompt. If you're on `main`, create a branch first with `lf start <name>`.

## What makes a good design doc

From the style guide: "Design around data structures and public APIs. Aim for a 1:1 mapping between real-world concepts and their representation in code."

The design doc should be **heavy on code**—not working code, but code that makes ideas concrete. Sketch data structures, function signatures, example API calls. The code is for communication, not execution.

```python
# This doesn't need to run. It shows the shape.
class Worktree:
    path: Path
    branch: str

def create_worktree(name: str) -> Worktree:
    """Create sibling worktree at ../{name}"""
    ...
```

**Quote the user generously.** When the user says something that captures intent, constraint, or priority—copy it verbatim into the doc. These quotes anchor what matters and prevent drift.

> "I want to be able to spin up a fresh LLM and hand it the spec"
> — captured during design conversation

## The spec should cover (~2000 words max):

- **What to build** — One sentence. What exists after this is done that doesn't exist now.
- **Data structures** — The core types. Sketch them in code.
- **APIs** — Key functions or endpoints. Signatures with brief intent.
- **Constraints** — What would cause a rewrite if guessed wrong. Dependencies, patterns to match, things to avoid.
- **Done when** — How to verify it works. A command to run, output to see.

## Conversation guidance

Bias toward brevity. Ask only what's needed to start coding.

Exploring uncertain ideas is fine—the doc has room (~2000 words). But keep narrowing toward a buildable thing. If scope is unclear, ask "what's the smallest version that's useful?" and spec that.

When the user says something important, confirm it and note that it will be quoted in the doc.

Completeness is not required. Wrong guesses get fixed in implementation. The goal is to not block the implementing session, not to predict everything.

## Auto mode

In auto/headless runs, do not pause to ask questions. Make the best assumption you can and append any open questions to `.design/questions.md`.

