# 05: Concerto Parent/Child UI

## What to build

Concerto's portfolio view shows parent->child edges between repo cards. Wave configuration becomes cross-repo aware — area pickers, trigger targets, and context sources understand related repos.

Trigger rename has landed across the full stack including Swift/Concerto. `Stimulus`/`stimuli` fully replaced with `Trigger`/`triggers`.

## Key interactions

### Portfolio view
- Edges rendered as visual connections between repo cards.
- Parent cards show nested or linked children.
- **Add edge**: Action on a repo card opens a picker of other portfolio repos. Picker excludes repos that would create a cycle.
- **Remove edge**: Action on an existing edge to remove it.
- Access direction visible: parent (RW) vs child (R) indicated on edges.

### Wave configuration
- **Area picker**: Shows paths from related repos using the established `repo_name:path` colon syntax — `loopflow:rust/loopflow/src/engine/` alongside local paths. The engine resolves this via `resolve_area()` matching against `RelatedRepoContext` entries.
- **Trigger targets**: When configuring listen, offer waves from related repos as targets. Uses `RepoId` instead of filesystem paths.
- **Context indicator**: Show which related repos contribute docs to the current session. Related-repo docs use `[owner/repo]` prefix in paths for disambiguation.

### Wave detail
- When a wave's session has touched files in related repos, show which repos are involved.

## Data flow

- Portfolio state (including edges) comes from lfd API.
- Edge mutations go through lfd API (stage 01 endpoints).
- LoopflowCore models gain `RepoEdge` type and `RepoId`.
- `PortfolioRepoState` exposes related repos (parents + children).

## Constraints

- Cycle detection happens server-side (lfd). Concerto reports the error.
- Works on macOS and iOS.
- Respect existing portfolio card layout — edges are additive, not a redesign.
- Area picker namespacing must be unambiguous when multiple related repos are present.

## Done when

- Portfolio view shows parent/child edges between repo cards
- User can add/remove edges through Concerto
- Cycle creation is rejected with clear feedback
- Area picker includes paths from related repos
- Trigger configuration offers waves from related repos
