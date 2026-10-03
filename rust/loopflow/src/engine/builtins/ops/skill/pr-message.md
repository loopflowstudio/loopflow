Generate a PR title and body for the changes on this branch.

Review the diff against the PR base. Do not ask questions; make context-backed
assumptions where necessary.

## Authorship

Write for someone returning after time away. Name the concrete improvement in the
title. Open the body with one short paragraph of one or two sentences explaining
what was difficult before
and what this PR makes easier or newly possible for users, operators, or maintainers.
They should understand the benefit without running a command or opening another document.

Keep the promise within what this PR delivers, even when the Task has a larger ambition.
Use familiar product language and concrete verbs. An area prefix is useful only when
it helps recognition. Preserve proper names and command spelling.

Follow with **What changes**: a short paragraph or a few bullets describing the
meaningful change. Include implementation detail only when it helps review. Put **Why it
matters** after that, and omit it if the summary already explains the consequence.
Include material risks or limitations when needed. Keep automated test and lint
results in **Checks** when useful, or link to CI. Finish with **Try it** when there
is a useful walkthrough: describe a concrete user action and the visible result
that demonstrates the benefit. Tests, test commands, and test results never belong
in this section. Distinguish suggested steps from behavior actually observed;
label simulations and remaining limits. Omit the walkthrough when it adds nothing.

Use only the sections the change needs. A small change may need only a short summary
and a useful walkthrough. Rewrite around the current diff when scope changes; remove superseded
explanation instead of appending a diary. Loopflow supplies Task identity and merge
consequences from durable state; do not invent or repeat those facts.

Link related work where you explain its relevance. In prose and PR bodies, use
`[Title · Task ID or PR number](known URL)` on first mention; shorten later references
when unambiguous. State the relationship, such as builds on, supersedes, or verified by.
Use known URLs and preserve cited decisions and evidence somewhere that survives shipping.
In operational lists, put the ID first: `[Task ID or PR number · Title](known URL)`.

## Output format

Return a structured response with **title** and **body** (Markdown).

Example title: `Switch Tasks without losing unfinished input`

Example body (illustrative; adapt the walkthrough to the actual change):

```markdown
Switching Tasks used to lose unfinished terminal input. Returning to a Task
now restores its terminals and draft input so you can pick up where you left off.

## What changes

Each Task retains its terminal layout and unfinished input while you visit
other work.

## Try it

Type an unfinished command in a Task terminal, switch Tasks, then return.
Your original terminal and unfinished input should still be present.
This is a suggested walkthrough; it has not been performed for this example.
```
