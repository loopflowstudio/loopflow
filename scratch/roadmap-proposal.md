---
status: proposed
---

# Concerto: Worktree Browser & Roadmap Launcher

## Context

The Concerto landing screen is being simplified (Stage 1, separate PR). These two features build on that foundation to complete the onboarding and recovery experience.

## Scope

Two features that give Concerto awareness of what's on disk and what's planned.

## Approach

### Stage A: Worktree Browser

Sidebar section showing existing git worktrees. Browse what's on disk, upgrade to waves.

- Rust: `GET /worktrees` endpoint (calls existing `list_worktrees()`, cross-references wave store for `has_wave`)
- Swift/LoopflowCore: `WorktreeInfo` model + `listWorktrees()` service method
- Swift/Concerto: Sidebar worktree section (permanent, below waves)
  - Branch name, path, merged/prunable status
  - "Upgrade to wave" per row (uses existing `POST /waves` with `worktree` field)

### Stage B: Roadmap → Waves

Roadmap items declare wave specs. Concerto reads them and offers batch launch.

- Wave spec in roadmap frontmatter (`wave:` block with flow, direction, area, owner)
- Prompt changes: `lf roadmap`, `lf add-to-roadmap`, `lf iterate` produce wave specs as part of output
- Sprint files (`.lf/sprint.yaml`) for defining subsets to launch
- Rust: `GET /roadmap/launchable` endpoint (parse frontmatter, cross-reference active waves)
- Swift/Concerto: Roadmap launcher view (checkboxes, batch launch, planned/running/done status)

### Ownership

Owner is defined in the roadmap wave spec, not in the daemon or hosted layer:

```yaml
---
status: proposed
wave:
  flow: ship
  direction: [product-engineer]
  area: [rust/loopflow/src/harness/]
  owner: alice
---
```

The roadmap is the coordination artifact. Hosted loopflow reads the owner field from the repo rather than maintaining a separate assignment system.
