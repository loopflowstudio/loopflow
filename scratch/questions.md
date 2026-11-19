# Open choices (d92894)

Made headless at kickoff and concept review; each is reversible. Detail in
`plan.md`, "Choices made for review", and `concept-review.html`, "Questions".

- The patch goes in whole under 15,000 tokens, the rule `context: {diff:
  true}` and the Task workspace block already use. Kickoff had planned 100,000
  bytes. Or never inline, even for a two-line commit?
- `context: {diff: true}` moves onto the same `Change` type in this Task, and
  loses its 20-file and 800-line limits. Leave it for its own Task instead?
- The importance-ordered fallback has no launch that would use it: every
  harness lf starts can run `git diff`. Planned: not built. Build it anyway?
- Notes added under `scratch/` appear in a branch change's file list, marked
  as cleared at land. Leave them out instead?
- The commit writer always skips permission prompts; the PR writer follows
  `yolo`. Both now have to run a command on large changes. Left as is. Give
  the PR writer the same floor?
- AGENTS.md says `lf pr submit` runs `lf scratch clear`; in code only
  `ops/land.rs:171` and `lf scratch clear` itself do. Which is meant?
