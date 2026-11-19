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
- `docs/config.md` "Raw Diff" said "Point to Git inspection commands rather
  than inlining a frozen patch", and that is what ran: the gathered diff was
  counted but never delivered. The section now goes in the `changes`
  reference file the context block lists, read on demand and written once at
  launch. Was a launch-time copy what the old sentence meant to avoid?
- With `diff_files` and no `diff`, the `changes` file still says `git diff
  main...HEAD`, whatever the default branch is. Left alone.
- qa, 2026-11-18: with `--diff patch` a haiku agent never opened the `changes`
  file in three runs; it ran its own `git diff`, and on a branch with only
  uncommitted work it answered "adds nothing beyond `main`". The file is in
  the context listing by path only. Put the section in the first turn, as
  `--docs` bodies are, or keep it a listed file? Left as a listed file; the
  docs now say so.
- An uncommitted change now lists untracked files, marked "no patch: read the
  file". Their contents are not inlined. Inline small ones?
