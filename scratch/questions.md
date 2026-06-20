# Open questions / assumptions

Current to the steps-as-skills milestone. See `steps-as-skills.md` for the plan.

- **Skill provenance + cleanup.** Generated skills are written into the user's
  `~/.claude/skills` and `~/.agents/skills`. How do we mark them as
  loopflow-generated (frontmatter marker? a `loopflow/` sub-namespace?) so we can
  prune stale ones without clobbering a user's own skills? Assume: a marker +
  prune-by-marker, and confirm before the first global write.
- **Sync trigger.** When does the sync run — on every launch (always fresh, small
  cost), an explicit `lf op sync-skills`, or a watch? Assume: sync-on-launch for
  correctness, plus an explicit command for setup/debugging.
- **`description` source.** Using the step's one-line summary (first line after
  frontmatter) as the skill `description`. Confirm every builtin step has a usable
  summary line; backfill where missing.
- **Headless skill scope.** Verified project-local skills fire under headless exec;
  global (`~/.claude/skills`, `~/.agents/skills`) is the same vendor mechanism but
  not separately verified headless. Confirm during build.
- **Auto-invocation is out of scope.** The seed is always explicit `/step`, so
  model-auto invocation by description is neither used nor tested. Revisit only if
  an autonomous wave must fire a perspective skill without naming it.
