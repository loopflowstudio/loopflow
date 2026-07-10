# Open questions & assumptions

## Linear PM reconciliation deferred — stale `lf`

`lf pm show` / `lf pm sync --plan` both 400 against Linear from this branch:

```
Variable "$projectId"/"$teamId" of type "ID!" used in position expecting type "String!".
```

The deployed binary is `lf 0.10.1`; the fix for this exact GraphQL type mismatch
already landed on `main` (commit `d078d1dd`). The stale local binary can't reach
the wave's Linear project, so this update-wave pass could **not** reconcile
tasks (close shipped, correct drift, file new work).

**Assumption:** shipped `wave-controls` work (Stop verb, empty-thought filter,
follow-at-bottom, attempt-failure presentation, voice-stack removal) should have
its Linear tasks closed with the merged PR link. Do it after redeploying `lf`:

```bash
lf pm show                       # confirm the query works post-redeploy
lf pm task done --id <id> --pr <PR url>
```
