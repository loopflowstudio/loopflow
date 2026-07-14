## Try it!

Start a Task with a stable semantic worktree name:

```bash
lf task run INF-123 --name release-scoped-migrations
```

Ship one PR and continue the same Task in the same worktree:

```bash
lf pr land --next released-upgrade-proof
lf status --json
```

The snapshot shows ordered `prs`, the `active_pr`, and the next branch after the first PR merges. Mark the final PR explicitly:

```bash
lf pr land -c
```

For clean investigation or documentation work with no commits:

```bash
lf task complete INF-123 --summary "Recorded the root cause"
```

No empty GitHub PR is opened.

## Intent

Let one concrete Task advance through multiple small, serial PRs without replacing its worktree, provider history, or supervision identity. Keep Task completion explicit and make the current PR's completion disposition visible to both CLI and Mac users.

## Assumptions

- Every Task PR targets `main`.
- A Task has at most one active PR; merged and abandoned PRs remain ordered history.
- GitHub is the current forge integration, so the publication receipt is GitHub-specific even though publication intent is not.
- Branch rotation occurs only after the active PR is merged or abandoned and the worktree is clean.

## Key decisions

- Model only Tasks and PRs; do not add a Delivery domain.
- Derive PR phase from publication, GitHub, merge, and abandonment evidence.
- Nest the optional GitHub receipt inside publication so impossible state combinations do not cross the wire or enter SQLite.
- Persist publication intent before GitHub side effects, then attach the GitHub receipt on success.
- Keep emptiness separate from phase and skip empty PRs instead of opening them.
- Settle a completing PR and complete its Task atomically.
- Send ordered `prs`, `active_pr`, and `after_merge` through the Rust/Swift DTO so the UI can say when a PR completes the Task.

## Not included

- Parallel Task PRs.
- Non-`main` PR targets.
- Empty GitHub PR creation.
- Forge integrations beyond GitHub.

