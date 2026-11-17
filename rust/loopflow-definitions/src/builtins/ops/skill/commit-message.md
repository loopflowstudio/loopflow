Write the commit message for the staged changes.

Read the diff and say what changed. Don't ask questions; assume and proceed.

The message describes the change, not the step that made it: "Read stored
outputs as inputs to later jobs", not "implement pass 3". Follow the
repository's own commit conventions where its agent guide gives them. No
attribution lines.

## Output format

- **title**: one line someone scanning history would understand.
- **body**: a sentence to a paragraph on why, and what to know before touching
  this again. An empty string when the title says it all.
