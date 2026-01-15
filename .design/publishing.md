# PyPI Documentation Update

## What to build

Update pyproject.toml metadata and README.md so the PyPI page presents loopflow clearly to potential users.

## Current state

The PyPI page shows:
- Description: "Arrange LLMs to code in harmony" (vague)
- README content from README.md (good but could improve)
- Missing: project URLs, better keywords, clearer classifiers

## Data structures

No new data structures. This is metadata-only.

## Key changes

### pyproject.toml

```python
[project]
description = "Run LLM coding agents from reusable prompt files"

keywords = [
    "llm", "claude", "codex", "gemini", "ai", "coding", "cli",
    "agents", "automation", "workflow", "prompts"
]

classifiers = [
    # Keep existing, add:
    "Operating System :: MacOS",
    "Typing :: Typed",
]

[project.urls]
Homepage = "https://loopflowstudio.github.io/loopflow/"
Repository = "https://github.com/loopflowstudio/loopflow"
Documentation = "https://loopflowstudio.github.io/loopflow/"
Issues = "https://github.com/loopflowstudio/loopflow/issues"
Changelog = "https://github.com/loopflowstudio/loopflow/releases"
```

### README.md (first section only)

Update the opening to hook PyPI browsers:

```markdown
# Loopflow

Run LLM coding agents from reusable prompt files.

```bash
lf review          # run .lf/review.lf
lf ship            # pipeline: implement → review → test → commit → PR
```

Write prompts as markdown files. Chain them into pipelines. Run them across isolated worktrees while you work on something else.

Supports Claude Code, OpenAI Codex, and Google Gemini CLI.
```

Keep the rest of the README as-is—it's already solid.

## UI changes

None. This is PyPI metadata only.

## Constraints

- README.md must remain valid for both PyPI and GitHub display
- Demo GIF reference (`![Loopflow demo](demo.gif)`) won't work on PyPI but that's acceptable
- Keep README under ~3000 words for PyPI readability

## Done when

```bash
# Verify pyproject.toml is valid
uv run python -c "import tomllib; tomllib.load(open('pyproject.toml', 'rb'))"

# Check the description matches
grep -q "Run LLM coding agents from reusable prompt files" pyproject.toml
grep -q "project.urls" pyproject.toml
```

After publishing:
- PyPI page shows "Run LLM coding agents from reusable prompt files"
- Links to GitHub, docs, and issues appear in sidebar
