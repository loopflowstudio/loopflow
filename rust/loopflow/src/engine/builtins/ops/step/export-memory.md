---
description: Compile a wave MEMORY.md from the current base and add-stream.
produces: wave/<wave>/MEMORY.md commit
---
Compile the wave's durable memory checkpoint.

## Goal

Write a reader-optimized `MEMORY.md` for the target wave: what a fresh session,
parent, or worker needs to know to act correctly without this run's context.

## Workflow

1. Read the current base:
   ```bash
   lf memory show --wave <wave>
   ```
2. Read recent memory facts:
   ```bash
   lf memory log --wave <wave>
   ```
3. Compile the memory:
   - Keep durable conclusions, decisions, constraints, vocabulary, and gotchas.
   - Drop narrative and chronology unless the sequence itself is load-bearing.
   - Merge duplicates; when facts conflict, prefer the newer fact and preserve
     the reason if it matters.
   - Keep it tight enough to seed the next mind directly.
4. Write the compiled result:
   - Prefer `lf memory update --wave <wave> --summary "compile MEMORY.md"` when
     a wave server is live.
   - If no wave server is live, write `wave/<wave>/MEMORY.md` directly. This is
     the serverless scheduled path; do not add an offline mode to
     `lf memory update`.
5. Commit the update:
   ```bash
   lf op commit -m "export-memory: compile MEMORY.md"
   ```

## Rules

- Do not restart the wave or resident mind.
- Do not archive raw facts into `MEMORY.md`; compile them.
- Do not reset or edit the memory journal directly. A server-backed
  `lf memory update` clears the add-delta; direct file writes do not.
