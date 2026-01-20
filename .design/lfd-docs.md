# lfd Command Reference Page

## What to build

Add `docs/lfd.md` — a command reference page for the lfd daemon CLI, matching the style of `docs/lf.md` and `docs/lfops.md`.

## Context

We have:
- `docs/loops.md` — conceptual overview of loops, goals, iterations
- `docs/triggers.md` — conceptual overview of subscribe and schedule
- `docs/lf.md` — command reference for `lf`
- `docs/lfops.md` — command reference for `lfops`

Missing: `docs/lfd.md` — command reference for `lfd`. Users need a quick reference for flags and options without reading the conceptual docs.

## Content Structure

Follow the pattern from `lfops.md`: command name, one-line description, usage example, options table.

### Commands to Document

From `src/loopflow/lfd/__init__.py`:

| Command | Purpose |
|---------|---------|
| `lfd serve` | Run daemon in foreground |
| `lfd install` | Install launchd service |
| `lfd uninstall` | Remove launchd service |
| `lfd start` | Start multiple loops |
| `lfd loop` | Start continuous loop |
| `lfd flow` | Run single iteration |
| `lfd subscribe` | Watch paths on main |
| `lfd schedule` | Run on cron schedule |
| `lfd status` | Show loop status |
| `lfd stop` | Stop a running loop |
| `lfd prs` | Show PRs from a loop |
| `lfd rm` | Remove loop and history |
| `lfd list-goals` | Show available goals |

### Flags Reference

```
lfd loop <goal>
  -a, --area        Area override (pathset)
  -l, --limit       PR limit override
  --merge-mode      pr | land
  -f, --foreground  Run in foreground

lfd flow <goal>
  -p, --project     Project/prompt file
  -v, --paste       Include clipboard
  -r                Area override

lfd subscribe <pathset> <goal>
  -r                Area override

lfd schedule "<cron>" <goal>
  -p, --project     Project file
  -r                Area override

lfd start [goals...]
  -a, --all         Include waiting loops

lfd stop <loop-id>
  -f, --force       Force kill (SIGKILL)

lfd prs <loop-id>
  -n, --limit       Number to show (default: 10)

lfd rm <loop-id>
  -f, --force       Skip confirmation
```

## File to Create

```
docs/lfd.md
```

## Jekyll Frontmatter

```yaml
---
layout: default
title: lfd Command Reference
---
```

## Navigation

Add to `docs/_config.yml` header_pages if not already present.

## Constraints

- Match existing doc style (see `lfops.md`)
- One-line command descriptions
- Options in tables, not prose
- Link to `loops.md` and `triggers.md` for conceptual background
- No duplicate content from conceptual docs

## Done When

1. `docs/lfd.md` exists with all 13 commands documented
2. Each command has: description, usage, options table (if applicable)
3. Links to `loops.md` and `triggers.md` in "See Also"
4. Builds without errors: `cd docs && jekyll build` (or just check markdown validity)
