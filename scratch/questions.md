# Open questions / blockers — memory-stream

## BLOCKER: `lf op pm update --status done` is broken (Linear API)

Closing the shipped `memory-stream` roadmap item
(`5d9d3dcc-80b2-439d-8771-d679e0191c30`) fails:

```
Error: linear request failed with status 400 Bad Request:
Variable "$teamId" of type "String!" used in position expecting type "ID".
```

Both the status-close and the `--pr` PR-comment paths hit it. The mutation
sends `teamId` as GraphQL `String!` where Linear's schema now expects `ID`.
This is a client bug in loopflow's Linear provider, not a data problem.

Impact: the `memory-stream` slice shipped (PR #823) but its roadmap item stays
`open` because the wave can't close it. Reads (`lf op pm show`) work fine.

Owner: this is `lf op pm` tooling, not Memory-wave code — belongs to whoever
owns the pm provider. Fix the GraphQL variable type, then close the item with:

```
lf op pm update --wave memory --id 5d9d3dcc-80b2-439d-8771-d679e0191c30 \
  --title "memory-stream: full-fact, replayable memory stream" \
  --status done --pr https://github.com/loopflowstudio/loopflow/pull/823
```
