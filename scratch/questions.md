# Open choices (d92894)

Made headless at kickoff; each is reversible. Detail in `plan.md`, "Choices
made for review".

- Should a change that fits (at most 100,000 bytes) still be inlined whole,
  or should the writer always read through the command, even for a two-line
  commit? Planned: inline whole.
- The importance-ordered fallback has no launch that would use it: every
  harness lf starts can run `git diff`. Planned: not built. Build it anyway?
- Notes added under `scratch/` appear in the PR writer's stat, marked as
  removed before publish. Leave them out instead?
