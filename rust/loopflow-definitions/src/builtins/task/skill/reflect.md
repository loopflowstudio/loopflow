---
requires: a Task that people steered, corrected or reviewed
produces: edits to this repository's skills or agent guide, or nothing
---
Turn what people had to say during this Task into instructions, so nobody has to say it twice.

**Read what people said, not what agents said.** The Task's conversations,
review comments on the PR, direction sent while steps ran:

```bash
lf task detail <ID>                 # comments, PR, Graphs, current Session
lf session history --task <ID>      # the Task's conversations
gh pr view --comments
```

For each correction or redirection, ask one question: would a better
instruction have produced this the first time?

**Leave most of it.** Most feedback is about this change and teaches nothing
general. A lesson is worth writing when the same miss would happen on the
next Task.

**Put the lesson where the step that missed it will read it.**

- One skill's method goes in this repository's addition to that skill,
  `.lf/skills/<name>.md`. A file with `extends: lf:<name>` (or `extends: <skill>`)
  in its frontmatter appends to the parent, so write only the lesson. Copy the
  whole skill with `lf skill customize <name> --full` only to change what it
  already says.
- A rule for every agent in this repository goes in the agent guide.

Write it as the instruction the step needed, in the skill's own voice, not as
a story about this Task. Don't name who said it.

**Remove as much as you add.** Before adding a line, read the file you are
editing and delete what the new line replaces, what no longer holds, and what
the skill would do anyway. A skill that only grows stops being read.

With nothing said by a person, or nothing general in it, change nothing and
say so in one line. Otherwise list each edit with the words that prompted it.
