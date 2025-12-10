# Inline Prompts

Run ad-hoc Claude commands with full repo context, without defining a task file.

```bash
lf : "fix the typo in README.md"
lf : 'add a test for the config loader'
```

## What to build

A way to pass a string directly to Claude with loopflow's context assembly, using `:` as a signifier.

## User quotes

> "I want a way to just pass a short task to a claude that still has all the context"

> "the inline text replaces the task"

## Detection

Use `:` as the signifier for inline prompts:

```bash
lf : "fix the typo"
lf : "add tests" -c src/
```

In `main()`, check if `sys.argv[1] == ":"` and route accordingly.

## Prompt structure

Same as tasks, but inline content:

```xml
<lf:docs>
<lf:README>...</lf:README>
<lf:STYLE>...</lf:STYLE>
</lf:docs>

<lf:task>fix the typo in README.md</lf:task>

<lf:files>
<lf:file path="src/api.py">...</lf:file>
</lf:files>
```

No task name in the tag (just `<lf:task>` not `<lf:task:review>`).

## API changes

```python
def build_prompt(
    repo_root: Path,
    task: str | None = None,      # None for inline prompts
    inline: str | None = None,    # The inline prompt text
    arg: str | None = None,
    context: list[str] | None = None,
) -> str:
```

Or simpler—overload `task`:

```python
def build_prompt(
    repo_root: Path,
    task: str,                    # Either task name or inline prompt
    inline: bool = False,         # True = task is the prompt itself
    arg: str | None = None,
    context: list[str] | None = None,
) -> str:
```

## CLI routing

In `main()`:

```python
def main():
    known_commands = {"run", "pipeline", "version", "install", "doctor", "pr", "land", "--help", "-h"}

    if len(sys.argv) > 1:
        first_arg = sys.argv[1]

        # Inline prompt: lf : "prompt"
        if first_arg == ":":
            sys.argv.pop(1)  # Remove the ":"
            sys.argv.insert(1, "inline")
        elif first_arg not in known_commands:
            # Existing task/pipeline routing
            ...

    app()
```

## Flags

All existing flags work:

```bash
lf : "fix the typo" -p              # batch mode
lf : "fix the typo" -c src/api.py   # add context
lf : "fix the typo" -b fix-typo     # create branch first
```

## Done when

```bash
lf : "say hello"
# Claude responds with greeting, using repo docs as context

lf : "list the files in src/" -c .
# Claude has full repo context
```

## Autocommit

Same as tasks: `lf : "fix typo" -p` autocommits with message `lf : fix typo`.
