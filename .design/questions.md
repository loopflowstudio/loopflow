# Open Questions

## Prompt frontmatter for default options

The `debug` task currently requires users to pass `-v` to include clipboard content:

```bash
lf debug -v
```

The user suggested adding frontmatter support to prompt files so tasks can specify defaults:

```markdown
---
paste: true
---
Debug an error using the stacktrace or error message from clipboard.
```

**Questions:**

1. Should this be a general frontmatter system for all prompt options (`paste`, `interactive`, `context`, etc.) or just `paste`?

2. Should it reuse the existing YAML frontmatter parser from `maestro/markdown.py` or should prompts stay plain markdown?

3. How should frontmatter defaults interact with CLI flags? (CLI wins? Error?)

**Current state:** Token profile now includes clipboard content when `-v` is passed. The frontmatter feature was not implemented in this iteration.
