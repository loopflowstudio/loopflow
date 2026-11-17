# Open questions

- A bare Graph hands every step the original message, so steps after kickoff
  re-answer it: in the fixture run, concept-review, write-docs, implement,
  compress, qa and pr-publish each replied "the line is already there".
  Should later steps get the plan instead of the request? Left as is.

- The step commit's `lf commit --step` child now takes the run's `-a`, so with
  `-a` on the command line it overrides `commit-message`'s own `agent:`
  preference (as `-a` does for every agent step of a Graph). The alternative is
  using `-a` only when `commit-message` resolves no agent. Took the simpler,
  consistent one.
- `lf process show <graph>` lists each step commit under its skill's label
  (`jot succeeded` twice per step): the step row records only the Graph node.
  Left as is; a distinct label means a stored field and a DTO change.
