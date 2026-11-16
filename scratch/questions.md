# LOO-936 open choices (kickoff, headless)

- History retention: 30 days for finished Processes, 90 days for Session
  history. Chosen so weekly usage and recent Task history keep working;
  either number is one constant.
- `lf history list` keeps listing finished Processes that a main record pins
  (Graph steps, ship), read from `processes` beside `process_history`. The
  simpler alternative is to list only `process_history`, which would hide
  Graph step commands from `lf history`.
- The attachment-token fence on history writes reads `loopflow.db` without
  its write lock, so a replaced agent can append one stray history row after
  a handoff. Accepted because history grants no authority.
- `output` (the last assistant message) stays copied into history: Graph
  routing, `lf run`, ci-fix and the Task view read it, and reading provider
  transcripts instead would tie routing to each provider's file format.
