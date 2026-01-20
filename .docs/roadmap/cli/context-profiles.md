---
status: proposed
area: cli
created_at: 2026-01-20T15:31:00
---

# Context Profiles: Structured Context for Large Codebases

Vision says "Context management is the hard problem—better context management = competitive advantage."

## The Problem

Current context assembly is basic: gather files from `context:` frontmatter, diff, clipboard. For large codebases, this either:
- Includes too much (overwhelming the model, hitting token limits)
- Includes too little (model lacks necessary context)

Users manually curate context. This knowledge disappears after the session.

## Proposed Solution

Introduce context profiles in `.lf/context/`:

```yaml
# .lf/context/auth.yaml
name: auth
description: Authentication system context
files:
  - src/auth/**/*.py
  - src/middleware/auth.py
symbols:
  - class:AuthService
  - function:verify_token
related: [api, database]
```

Steps can reference profiles:

```yaml
---
context: auth
# or
context: [auth, api]
---
```

## Implementation

1. `@dataclass ContextProfile` in `src/loopflow/lf/context.py`
2. `load_context_profile()` function
3. Update context assembly in `run.py` to resolve profiles
4. `lf context list` to show available profiles
5. `lf context show auth` to preview what files would be included

## Why This Matters

- Reproducible context across sessions
- Team can share context knowledge
- Scales to large codebases without manual curation
- Profiles evolve with the codebase (versioned in git)

## Open Questions

1. Should profiles support dynamic queries (files changed in last N commits)?
2. How to handle profile size estimation for token budgets?
