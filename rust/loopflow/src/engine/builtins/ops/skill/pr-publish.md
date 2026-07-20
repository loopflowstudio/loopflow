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

2. Write a concise title and markdown body:
   - title: lowercase, optional area prefix
   - body answers what the reviewer is asking: What's the intention? What are the assumptions? What does it accomplish? How do I evaluate it?
     - **Try it!** — lead with this. Commands to run, what they'll see. Include metrics/results when measurable (before/after numbers, benchmarks, test outputs).
     - **Intent** — one paragraph. Why this change exists and what it accomplishes.
     - **Assumptions** — what this relies on being true.
     - **Key decisions** — non-obvious choices and why.
     - **Not included** — intentional omissions, if any.

3. Publish or refresh the PR with explicit fields. This pushes and creates or
   updates the PR, then prints its state and URL — it opens no browser.
   ```bash
   lf pr publish --title "<title>" --body "<body>"
   ```
   `lf pr open` does the same publish and then opens the PR for review; use it
   only when a human explicitly asked to see the PR.
