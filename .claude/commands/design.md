---
interactive: true
---
Produce a short implementation spec that another LLM session can use to write a first draft.

The design doc is scaffolding—a checkpoint for recovery, not documentation for posterity. If a session crashes, the spec lets a fresh session pick up where things left off. `lf ops pr land` deletes `.design/` contents.

## Workflow

1. Run `git branch --show-current` to confirm you're on a feature branch (not `main`)
2. Create `.design/<feature-name>.md` early—after the first exchange or two
3. Write as you go, refining with each conversation turn
4. Run `git add .design/ && git commit -m "design: <branch>"` when done
5. End session. Implementation happens via `lf implement`.

Write as you go, not at the end. If the session crashes mid-design, the partial doc is still useful. Let writing inspire questions—gaps become obvious when you make things concrete.

## What makes a good design doc

**Heavy on code.** Sketch data structures, function signatures, example API calls. The code is for communication, not execution:

```python
@dataclass
class Worktree:
    path: Path
    branch: str

def create_worktree(name: str) -> Worktree:
    """Create sibling worktree at ../{repo}.{name}"""
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

## Loopflow design principles

When designing for this codebase:

- **Worktrees are first-class.** Features should work across isolated worktrees. Don't assume a single working directory.
- **Prompts are files, not config.** Task definitions live in `.claude/commands/` or `.lf/`. Don't design template systems or prompt builders.
- **Design for auto mode.** Most tasks run headless. Don't require interactive confirmation for core flows.
- **Wrap, don't reimplement.** Loopflow delegates to Claude Code and Codex CLIs. Design to pass through, not duplicate.
- **Simple data structures.** Prefer `@dataclass` over inheritance hierarchies. Prefer functions over classes when state isn't needed.

## Conversation guidance

Bias toward brevity. Ask only what's needed to start coding.

If scope is unclear, ask "what's the smallest version that's useful?" and spec that.

Completeness is not required. Wrong guesses get fixed in implementation. The goal is to not block the implementing session, not to predict everything.

## Auto mode

In auto/headless runs, do not pause to ask questions. Make the best assumption you can and append any open questions to `.design/questions.md`.

