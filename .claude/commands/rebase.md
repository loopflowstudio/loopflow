Rebase this branch onto main.

## Process

1. Run `git diff main...HEAD` and note the intent of this branch before rebasing
2. Run `git fetch origin main`
3. Run `git rebase origin/main`
4. If there are conflicts:
   - Review what this branch was trying to accomplish
   - For code central to the branch's intent, preserve the branch's changes
   - For code outside the branch's core purpose, defer to main
   - Continue the rebase after resolving
5. Run `git push --force-with-lease` to update origin

## Auto mode

In auto/headless runs, do not pause to ask questions. Make the best assumption you can and append any open questions to `.design/questions.md`.

