# 01: `lf land` + worktree rotation

**Finish line:** `lf land` runs `lf ops land` as fast-path. `lf ops land` includes worktree rotation — rename shortname to full-path, check wave, create next. `lf ops wt prune` skips shortname worktrees.

## What to build

**`lf ops land` additions (Rust):**

After landing the PR, add rotation:

1. Detect worktree situation (not-a-worktree / full-path / shortname)
2. If shortname: `git worktree move` to full-path derived from branch name
3. Check `wave/<shortname>/` for remaining items
4. If items: `git worktree add` new shortname on fresh branch
5. If not: done

**`lf ops wt prune` update (Rust):**

Shortname worktrees always protected. Full-path worktrees prune if merged.

**`lf land` step (new prompt):**

```yaml
---
fast-path: lf ops land
---
```

Agent prompt for failure cases. Lists the ops API:

```
lf ops land [--local] [--create-pr] [--no-lint]
lf ops wt move <worktree> <new-path>
lf ops wt create <name> [--base BRANCH]
lf ops wt list [--format json]
```

**`fast-path` feature (Rust):**

New step frontmatter field. Runner checks for `fast-path`, runs the command first. If exit 0, step completes without agent. If non-zero, agent session starts with failure output as context.

## Done when

```bash
lf land  # happy path: fast, no agent
# → PR lands, repo.mobile → repo.mobile.20250225_1122, new repo.mobile if wave has items

lf land  # failure: agent resolves
# → agent reads error, handles it

lf ops wt prune --dry-run
# → lists full-path worktrees, skips shortname
```
