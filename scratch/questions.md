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
- `--diff patch` moves the section into the first turn, after the request
  (plan, choice 5). qa, 2026-11-18: as a listed `changes` file a haiku agent
  never opened it in three runs, and on a branch with only uncommitted work it
  answered "adds nothing beyond `main`". Keep it a listed file instead?
- `docs/config.md` "Raw Diff" said "Point to Git inspection commands rather
  than inlining a frozen patch". On `main` the gathered diff was counted and
  never delivered. Did that sentence mean never put patch text in a Skill's
  prompt, or only not to rely on it staying current? If the first, the section
  should carry the list and the command without the patch for Skills.
- With `diff_files` and no `diff`, the `changes` file still says `git diff
  main...HEAD`, whatever the default branch is. Left alone.
- An uncommitted change now lists untracked files, marked "no patch: read the
  file". Their contents are not inlined. Inline small ones?
- The change block counts toward the 122,880-byte terminal argument cap
  (`agent::validate_terminal_turn`), as the clipboard does. A list of several
  thousand files, or a patch near 15,000 tokens beside a large clipboard,
  could refuse an interactive launch. Not bounded here.
