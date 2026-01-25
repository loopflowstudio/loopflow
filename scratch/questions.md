# Questions from lfd CLI Redesign Implementation

## Open Questions

1. **Backward Compatibility**: The old flow-centric commands (`lfd loop ship swift/`) no longer work. Should we add backward-compatible aliases or is the clean break intentional?
   - Note: `create_agent()` no longer reuses agents by area (name-only).

2. **Watch Path Override**: The design mentions `--path` option for `watch` command to override watch path from area. This is implemented but not tested. Should we add integration tests for watch stimulus with path override?

3. **Default Values for Existing Agents**: When loading existing agents from the database with NULL area/goal, they're returned as `None`. Should we provide default values in some contexts (e.g., for display purposes)?

4. **Migration**: The design mentions "Existing agents with area/goal set continue to work." This is true since we handle NULL gracefully. Should we add a migration script to update the schema_version in the baseline?

## Decisions Made

1. **Area Required for Run**: Validation happens at run-time - `area` must be set, `goal` is optional, `flow` defaults to "ship".

2. **Agent Resolution**: Agents can be resolved by name or ID. Name is checked first (scoped to repo), then ID (globally).

3. **One-Shot Pattern**: Commands like `lfd loop agent --area src/` will create the agent if it doesn't exist, set the area, and start the loop - all in one command.

4. **HTTP API**: Added PATCH /agents/:id for updates, POST /agents/:id/run with validation, POST /agents/:id/stop for stopping.
