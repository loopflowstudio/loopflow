# Chord Model Wave Memory

## Patterns

- The chord-model wave spans the cross-layer control plane: `rust/loopflow/src/lfd/`, `rust/loopflow/src/lfd/http/`, `rust/loopflow/src/engine/`, `rust/loopflow/src/engine/builtins/`, and `python/loopflow/`.
- Chord membership lives in ordinary wave `area` entries that point at `wave/<name>/` directories. Do not reintroduce standalone chord CRUD.
- The current program is staged through the wave docs in `wave/chord-model/`. Item 02 proves live tend/vsm behavior first; later items build on that runtime slice.

## Preferences

- Favor reproducible repo-local scripts under `scripts/` for demos and validation instead of leaving ad hoc command lists in scratch docs.
- Keep scratch focused on remaining design/runtime gaps; landed work usually clears scratch, so the wave docs and code are the durable source of truth.

## Learnings

- As of 2026-03-18, `origin/main` includes algedonic signal plumbing and `LF_HOME` isolation from the redesign/chord-model work, so new chord-model changes should build on those facilities instead of re-adding local-only setup.
