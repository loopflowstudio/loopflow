# LOO-551 assumptions (kickoff, October 21, 2026)

- **A1 — display projection name.** `FlowGraph` (the drawn shape of a compiled
  definition) becomes `CompiledGraph` with wire field `compiled`, since "graph"
  now names the definition itself. Simpler alternative kept open: type
  `GraphDiagram`, field `graph` — rejected because Swift already uses
  `FlowDiagram` for the view that becomes `GraphDiagram`.
- **A2 — superseded** by Jack's design review: cron renames with everything else.
- **A3 — capture manifests keep `"flow"`** on disk (and in observed
  `session_events` payloads), following the existing `exec` precedent.
- **A4 — Wave memory** (`.lf/wave/*/MEMORY.md`) is not edited by this Task.
