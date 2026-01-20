# lf add: Create new prompt files

`lf add <name>` creates `.claude/commands/<name>.md` with a starter template.

## Usage

```bash
lf add foo          # creates .claude/commands/foo.md
lf add foo -f       # overwrites if exists
```

## Implementation

**`src/loopflow/lf/run.py`:**
- `PROMPT_TEMPLATE` constant with frontmatter + `{args}` placeholder
- `add()` function: validates git repo, creates directory, writes file

**`src/loopflow/lf/__init__.py`:**
- Registered via `app.command()(run_module.add)`
- Added to `known_commands` set

## Template

```markdown
---
produces: <results>
---
Foo task.

{args}
```

Name is capitalized in the template body. The `{args}` placeholder lets users pass arguments: `lf foo: do something`.
