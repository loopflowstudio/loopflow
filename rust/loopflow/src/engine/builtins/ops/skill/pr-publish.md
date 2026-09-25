---
requires: code on branch
produces: published/updated PR
---
Generate a PR title/body, then call the mechanical ops command.

## Goal

Write reviewer-friendly PR copy with agent judgment. Use ops only for execution.

## Workflow

1. Inspect branch changes.
   ```bash
   git log origin/main..HEAD --oneline
   git diff origin/main...HEAD --stat
   ```

2. Write the title and body using the authorship contract below.

3. Publish or refresh the PR with explicit fields. This pushes and creates or
   updates the PR, then prints its state and URL — it opens no browser.
   ```bash
   lf pr publish --title "<title>" --body "<body>"
   ```
   `lf pr open` does the same publish and then opens the PR for review; use it
   only when a person explicitly asked to see the PR.

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
