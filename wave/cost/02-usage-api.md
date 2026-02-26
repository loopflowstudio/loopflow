# 02: Usage API

HTTP endpoints for session, wave, and global token usage aggregation.

## What to build

Three new lfd endpoints that aggregate `turn_usage` and `context_snapshot` events into queryable summaries. Powers both Concerto and lfq.

### Endpoints

```
GET /sessions/{id}/usage       → SessionUsage
GET /waves/{id}/usage          → WaveUsage (aggregate across wave's sessions)
GET /usage/summary             → grouped aggregation
    ?wave=X &flow=Y &step=Z &model=M
    &from=timestamp &to=timestamp
    &group_by=wave|flow|step|model|source
```

### Aggregation types

`SessionUsage`: sum of TurnUsage events for one session, plus ContextSnapshot if present.

`WaveUsage`: sum across all sessions in a wave's runs.

Summary endpoint: group-by aggregation for the analytics dashboard. Scans session_events filtered by dimensions, returns grouped totals.

## Done when

`curl /sessions/{id}/usage` returns token counts. `curl /usage/summary?group_by=step` returns grouped data.
