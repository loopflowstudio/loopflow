# Stage 3: Sync tooling

Build `lf ops workstyle sync` to pull latest prompts from garrytan/gstack and re-convert. Users control when prompts update — sync is explicit.

## What to build

**`lf ops workstyle sync <name>`**:
1. Read `workstyle.yaml` for source repo and ref
2. Clone/fetch the repo to a cache (`~/.lf/cache/workstyles/gstack/`)
3. Run the converter on all SKILL.md files
4. Write updated steps to `.lf/workstyles/gstack/steps/`
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
Converting 28 skills...
Updated 3 steps, 25 unchanged.
Synced to commit abc1234 (2026-03-26).
```

## Data structures

```yaml
# .lf/workstyles/gstack/workstyle.yaml
name: gstack
description: "Garry Tan's sprint factory"
source:
  repo: garrytan/gstack
  ref: main
  last_sync: 2026-03-27T12:00:00Z
  last_commit: abc1234def5678
steps:
  prefix: gstack
  path: steps/
flows:
  - sprint
  - plan-manual
  - review
voice: voice.md
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

## Done when

1. `lf ops workstyle sync gstack` pulls latest and converts
2. `lf ops workstyle diff gstack` shows upstream changes
3. `lf ops workstyle list` shows all installed workstyles
4. Re-syncing after upstream changes updates only changed steps
5. Sync completes in <30s on a warm cache
