Compare two implementations of the same task and produce a written analysis.

## Implementation A: {{name_a}}

<diff>
{{diff_a}}
</diff>

## Implementation B: {{name_b}}

<diff>
{{diff_b}}
</diff>

## Task

Analyze both implementations and write your analysis to a markdown file under `{{output_dir}}`.

### What to cover

**Approach.** How does each implementation solve the problem? What are the key architectural differences?

**Trade-offs.** What does each approach gain and lose? Consider:
- Code complexity and maintainability
- Performance implications
- Flexibility for future changes
- Error handling and edge cases

**Recommendation.** Which implementation should be used? Be specific about why. If parts of each are better, note what to cherry-pick.

**Adaptations.** Are there pieces from the non-recommended implementation worth incorporating?

### Format

Write directly to a new markdown file under `{{output_dir}}` (pick a short descriptive name, create the folder if needed). Use markdown with clear sections. Be concise—aim for 500-1000 words. The goal is a decision document, not an exhaustive review.

In interactive mode, discuss first if you have questions, then write the file.
In batch mode, write the file immediately based on what you see.
