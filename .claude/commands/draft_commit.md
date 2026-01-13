Draft a commit message for landing this branch.

## Process

1. Run `git log main..HEAD --oneline` to see commits on this branch
2. Run `git diff main...HEAD` to see all changes
3. Write a commit message to `.lf/COMMIT`:
   - First line: concise summary (50 chars or less)
   - Blank line
   - Body: what this branch accomplished and why
4. Done. Tell the user to review `.lf/COMMIT` and run `lf land` when ready.

## Style

Summarize the branch, not individual commits.

```
feature: add user authentication

Users can now log in with email/password. Sessions expire after 24h.
Includes login page, session middleware, and logout endpoint.
```

## Auto mode

In auto/headless runs, do not pause to ask questions. Make the best assumption you can and append any open questions to `.design/questions.md`.

