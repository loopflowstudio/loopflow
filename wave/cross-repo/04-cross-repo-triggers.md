# 04: Cross-Repo Triggers

## What to build

Waves can listen to waves in related repos (parents or children). The trigger `source_repo` field references a `RepoId` instead of a raw filesystem path. The edge graph validates that the relationship exists.

## Key changes

- `Trigger.source_repo` accepts a `RepoId` (GitHub `owner/repo`) instead of a filesystem path.
- When resolving a trigger source, lfd checks the edge graph to confirm the repos are related.
- Both directions work: a child wave can listen to a parent wave, and a parent wave can listen to a child wave.

## Behavior

1. Wave config specifies `source_repo: loopflowstudio/loopflow` (a `RepoId`).
2. When lfd evaluates the trigger, it resolves the `RepoId` to a local path via the store.
3. lfd validates that an edge exists between the wave's repo and the source repo (in either direction).
4. If valid, the wave receives events from the source wave as it does today with local waves.

## Constraints

- No edge -> no listen. Can't listen to an unrelated repo.
- Direction doesn't matter for listen — both parent->child and child->parent edges allow listening in either direction.
- If the source repo isn't on disk, the trigger is inactive (same as today when `source_repo` path doesn't exist).
- Backward compatible: existing `source_repo` paths continue to work but are deprecated in favor of `RepoId`.

## Done when

- A wave can listen to waves in a related repo using `RepoId`
- Edge graph validates the relationship
- Both parent-listens-to-child and child-listens-to-parent work
- Missing source repos are handled gracefully
