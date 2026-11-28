# LOO-1125 questions

- Kickoff read Q2 b ("Allow realign to edit docs inline (only code/tests force
  iterate)") by file: documentation files are docs; a comment inside a source
  or test file is a code edit and goes through `implement`. Is that the line
  wanted, or may `realign` edit comments in source files?
- `optimize` ends `compress → graph: refresh → measure`, with no `implement`
  after `refresh`. A code mismatch `realign` finds there now stays in
  `scratch/plan.md`. Worth an `implement` step there, or fine as is?
- Loops have no pass cap. If `realign` iterating on every in-scope code finding
  makes `build` spin in practice, the fix is a cap or explore's Vision C guard;
  both are outside this Task.
