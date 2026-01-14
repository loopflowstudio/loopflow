# UI Explorations

Adds `lf ops commit` command, clipboard support for token analysis, improved test guidance in polish prompt, and reference documentation files on agent orchestration workflows.

## Review

**Verdict:** Needs work

### Code issues

1. **Merge conflict markers in working tree** (`src/loopflow/cli/commit.py`). The file contains unresolved `<<<<<<< HEAD` markers. The committed version is clean, but the working tree has conflicts that need resolution.

2. **Test mismatch with implementation** (`tests/test_commit.py`). Tests expect behaviors that don't match the committed implementation:
   - Tests check for "Staging changes" output, but committed code doesn't print that
   - Tests use `-m` flag which doesn't exist in the committed implementation (which uses `-a/-A` for add/no-add instead)
   - `test_commit_with_custom_message` asserts `mock_gen.assert_not_called()` but the mock is never assigned to `mock_gen` in that test path

3. **Test uses unused pytest fixture** (`tests/test_commit.py:15-19`). The `mock_repo` fixture creates a temp directory but only `test_commit_with_no_changes` uses it. Other tests shadow it with `mock_repo = Path("/fake/repo")`.

### Style notes

- `tokens.py:52`: The docstring was simplified per style guide, which is correct.
- New `files/*.md` documentation is extensive reference material. Not code, so no style concerns.

## Design notes

### Open question: Prompt frontmatter

The `debug` task currently requires users to pass `-v` to include clipboard content. A proposed alternative: frontmatter in prompt files to specify defaults:

```markdown
---
paste: true
---
Debug an error using the stacktrace or error message from clipboard.
```

Questions to resolve:

1. General frontmatter system for all options (`paste`, `interactive`, `context`, etc.) or just `paste`?
2. Reuse existing YAML parser from `maestro/markdown.py` or keep prompts as plain markdown?
3. How should frontmatter defaults interact with CLI flags? (CLI wins? Error?)

**Current state:** Token profile includes clipboard when `-v` is passed. Frontmatter not implemented.

### lf ops commit

Adds standalone commit command with LLM-generated messages. Options:
- `-p/--push`: Push after commit
- `-a/-A/--add/--no-add`: Stage all changes (default: yes)

The committed implementation differs from what the diff shows in `<lf:diff>` (which appears to be from a different version). The README documents `-p` and `-m` flags, but committed code has `-p` and `-a/-A`.
