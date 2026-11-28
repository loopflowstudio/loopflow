# Questions

- LOO-1127: "a little more ambitious" was read as wider scope (the whole
  change in its area), a worklist of what the branch added, and licence to
  reshape; not as a Graph change or wider local test runs. The wording for review is in
  [the builtin skill](../rust/loopflow-definitions/src/builtins/task/skill/compress.md).
- Where compress puts a cut it won't take: `## Deferred findings` when a plan
  exists, questions in `scratch/questions.md` (concept review's 1a, made for
  review). The alternative keeps compress out of `scratch/plan.md`.
- A plan-less run (standalone `lf compress`) reports deferred cuts in the
  reply, and questions too unless `scratch/questions.md` exists.
