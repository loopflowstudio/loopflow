# 04: Concerto Parent/Child UI

## What to build

Concerto's portfolio view shows parent→child edges between repo cards. Users can add and remove edges. Wave views show which repos a wave touches.

## Key interactions

- **Portfolio view**: Edges rendered as visual connections between repo cards. Parent cards show nested or linked children.
- **Add child**: Action on a repo card opens a picker of other portfolio repos. Select one to create an edge. Picker excludes repos that would create a cycle.
- **Remove child**: Action on an existing edge to remove it.
- **Wave detail**: When a wave's session has touched files in child repos, show which repos are involved.

## Data flow

- Portfolio state (including edges) comes from lfd API.
- Edge mutations go through lfd API (same endpoints from stage 01).
- LoopflowCore models need `RepoEdge` type and portfolio state updated to include edges.

## Constraints

- Cycle detection happens server-side (lfd). Concerto just reports the error if an edge would create a cycle.
- Works on macOS and iOS (portfolio view exists on both platforms).
- Respect existing portfolio card layout — edges are additive, not a redesign.

## Done when

- Portfolio view shows parent/child edges between repo cards
- User can add/remove child relationships through Concerto
- Cycle creation is rejected with clear feedback
