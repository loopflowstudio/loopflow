---
status: proposed
area: cli
created_at: 2026-01-20T15:34:00
---

# Roadmap CLI: Manage Work Items from Terminal

## The Problem

The roadmap infrastructure exists (`roadmap.py`) but there's no CLI to interact with it. Users must manually create markdown files and remember the frontmatter format.

## Proposed Solution

Add `lf roadmap` commands:

```bash
# List all roadmap items
lf roadmap

# List items for an area
lf roadmap --area cli

# Create a new item
lf roadmap add cli "Model Racing" --body "Race models against each other"

# Approve an item for work
lf roadmap approve cli/model-racing

# Mark item complete (moves to _done/)
lf roadmap done cli/model-racing
```

## Implementation

Most of the logic already exists in `roadmap.py`:
- `load_roadmap()` - ✓
- `create_item()` - ✓
- `approve_item()` - ✓
- `complete_item()` - ✓

Just need CLI wiring:

1. Add `roadmap` command group to `lf/__init__.py`
2. Wire to existing `roadmap.py` functions
3. Add formatted output using `format_roadmap_list()`

## Why This Matters

- Faster workflow for managing roadmap items
- Consistent frontmatter (no manual YAML errors)
- Integrates with adaptive mode flow (agents can propose items, humans approve)
