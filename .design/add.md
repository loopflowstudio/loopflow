# lf add: Create new prompt files

**What to build:** A `lf add <name>` command that creates `.claude/commands/<name>.md` with a starter template.

## Data Structures

No new types needed. The command is stateless—it creates a file and exits.

## Key Functions

```python
# src/loopflow/lf/run.py

def add(
    name: str = typer.Argument(help="Name for the new prompt (becomes filename and topic)"),
    force: bool = typer.Option(False, "-f", "--force", help="Overwrite if exists"),
):
    """Create a new prompt file at .claude/commands/<name>.md"""
```

## Behavior

```bash
lf add foo          # creates .claude/commands/foo.md
lf add foo -f       # overwrites if exists
```

**File creation:**
- Target: `.claude/commands/<name>.md`
- Creates `.claude/commands/` directory if missing
- Errors if file exists (unless `-f`)
- Must be in a git repo (uses `find_worktree_root()`)

**Template content:**
```markdown
---
produces: <results>
---
<Name> task.

{args}
```

The `{args}` placeholder lets users pass arguments: `lf foo: do something specific`.

**Output:**
```
Created .claude/commands/foo.md
```

## CLI Registration

In `src/loopflow/lf/__init__.py`:
```python
app.command()(run_module.add)
```

Add to `known_commands` set:
```python
known_commands = {"run", "pipeline", "inline", "cp", "add", ...}
```

## Constraints

- **No topic inference.** The name is the filename. Don't try to parse "add-user-auth" into a description.
- **Minimal template.** Just frontmatter + placeholder. Users will edit it anyway.
- **Portable location.** Always `.claude/commands/`, not `.lf/`. This is the preferred location per docs.

## Done When

```bash
cd /tmp && git init test-repo && cd test-repo
lf add review
cat .claude/commands/review.md
# Shows template content

lf add review
# Error: .claude/commands/review.md already exists

lf add review -f
# Overwrites successfully
```
