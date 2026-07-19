# Slice 3 repair: remove the deleted live-memory contract from current docs

## Problem

The implementation is file-only, but current documentation still advertises
the deleted server API:

- `rust/loopflow/src/wave/README.md` lists memory SSE events and `/memory`
  read/write/log routes;
- `swift/Loopflow/Services/WaveChatClient.swift` says Wave SSE carries memory
  frames;
- `wave/intelligence/MEMORY.md` describes live memory replay, server ownership,
  `lf memory add|update`, and the deleted routes as current architecture.

The last file is active prompt truth, not historical release documentation.

## Required behavior

- Current README and source comments describe only direct `MEMORY.md` reads and
  the surviving Wave chat/runtime events.
- Historical migration/release records may retain old names when clearly
  historical.
- Do not recreate a route, event, command, replay log, or compatibility alias.
- Do not edit `wave/*/MEMORY.md` directly. Loopflow operating rules currently
  call it server-owned. Determine and report the compliant curation path; if no
  path can update this branch after the write API deletion, leave that one file
  untouched and record the exact architectural/tooling contradiction in
  `scratch/questions.md`.

## Done when

- [ ] Wave README advertises no memory event or `/memory` route.
- [ ] Swift Wave chat comments advertise no memory frame.
- [ ] Current docs/source searches contain no live-memory API claim.
- [ ] The stale `wave/intelligence/MEMORY.md` content is either curated through
      the allowed API into this worktree or recorded as an exact unresolved
      blocker without bypassing server ownership.
- [ ] Focused docs/static checks and relevant memory/Swift tests pass.
- [ ] `scratch/feedback-runtime-review.md` records the repair honestly.
