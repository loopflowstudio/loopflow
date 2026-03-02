---
requires: code on branch
produces: opened/updated PR
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
   - body: usage/verification first, then summary

3. Open or update the PR with explicit fields.
   ```bash
   lf ops pr --title "<title>" --body "<body>"
   ```

## Notes

- If you only need rebase/push refresh with no message changes, use:
  ```bash
  lf ops pr --refresh
  ```
