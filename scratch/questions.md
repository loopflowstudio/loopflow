# Open questions / blockers

## `lf op pm update --status done` and `--pr` are broken (Systems/tooling)

Closing a roadmap task fails:

```
lf op pm update --wave memory --id <id> --title "..." --status done
Error: linear request failed with status 400 Bad Request:
Variable "$teamId" of type "String!" used in position expecting type "ID".
```

- The 400 fires on the **status-transition** path (looking up the team's
  workflow state to move the task to Done) and on the **`--pr` comment** path.
- A plain `--title`/`--notes` update succeeds — only the transition/comment
  GraphQL mutations pass `teamId` as `String!` where Linear's schema now expects
  `ID`.
- **Workaround applied:** slice-1 task (`5d9d3dcc-…`,
  "memory-stream: full-fact, replayable memory stream") is marked shipped in its
  **notes** with the PR link, but its state flag is still `open`. Flip it to Done
  by hand in Linear, or after the `lf op pm` GraphQL fix lands.

This is `lf op pm` plumbing, not memory-wave work — belongs to Systems.
