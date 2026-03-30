# Stage 3: Sync tooling

Build `lf ops workstyle sync` to pull latest prompts from garrytan/gstack and re-convert. Users control when prompts update — sync is explicit.

## What to build

**`lf ops workstyle sync <name>`**:
1. Read `workstyle.yaml` for source repo and ref
2. Clone/fetch the repo to a cache (`~/.lf/cache/workstyles/gstack/`)
3. Run the converter on all SKILL.md files
4. Write updated steps to `.lf/steps/gstack/` and refresh any extracted direction artifacts
5. Update `workstyle.yaml` with new `last_sync` and `last_commit`

**`lf ops workstyle sync --all`**: sync every workstyle with a remote source.

**`lf ops workstyle diff <name>`**: show what changed upstream since last sync without applying.

**`lf ops workstyle list`**: show installed workstyles, their source, and sync status.

```bash
$ lf ops workstyle list
NAME     SOURCE                  LAST SYNC            STATUS
gstack   garrytan/gstack@main   2026-03-27 12:00     3 commits behind
lfjack   builtin                —                     current
vsm      builtin                —                     current

$ lf ops workstyle diff gstack
office-hours.md  | 12 ++--
review.md        | 34 +++++++---
qa.md            | 8 ++

$ lf ops workstyle sync gstack
Fetching garrytan/gstack@main...
Converting 29 skills...
Updated 3 steps, 25 unchanged.
Synced to commit abc1234 (2026-03-26).
```

## Data structures

```yaml
# .lf/steps/gstack/workstyle.yaml
name: gstack
description: "Garry Tan's sprint factory"
source:
  repo: garrytan/gstack
  ref: main
  last_sync: 2026-03-27T12:00:00Z
  last_commit: abc1234def5678
steps:
  prefix: gstack
  path: ./
flows:
  - sprint
  - plan-manual
  - review
```

## Sync cache

```
~/.lf/cache/workstyles/gstack/     # shallow clone of garrytan/gstack
  .git/
  office-hours/SKILL.md
  review/SKILL.md
  ...
```

Shallow clone, fetch-only. Never modify the cache.

## Known state from stage 1

- The converter (`python/loopflow/workstyle/convert.py`) rewrites cross-step references (`/plan-*` → `gstack:<step>`) and skill-file paths (`~/.claude/skills/gstack/.../SKILL.md` → `.lf/steps/gstack/*.md`) during conversion. Re-sync must re-run these rewrites.
- The converter strips retro analytics/eureka telemetry instructions. If upstream adds new telemetry patterns, the strip rules may need updating.
- `plan-design-review` was renamed to `design-review` and the original `design-review` audit skill became `design-audit`. Sync must preserve this mapping to avoid overwriting.
- The committed `.lf/steps/gstack/*.md` files are generated output — any converter change must be followed by re-running conversion to keep them aligned.

## Done when

1. `lf ops workstyle sync gstack` pulls latest and converts
2. `lf ops workstyle diff gstack` shows upstream changes
3. `lf ops workstyle list` shows all installed workstyles
4. Re-syncing after upstream changes updates only changed steps
5. Re-syncing refreshes extracted direction content when the upstream style doc changes
6. Sync completes in <30s on a warm cache
