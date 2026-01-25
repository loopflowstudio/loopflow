# lfd CLI Redesign

## Current State

lfd commands are flow-centric:
```
lfd loop ship src/
lfd subscribe ship src/ -p src/
lfd schedule ship . "0 9 * * *"
```

This creates agents implicitly. Concerto wants to create agents explicitly, configure incrementally, then run.

## Proposed API

Agent-centric CRUD with implicit noun (all lfd commands operate on agents).

### Core Commands

```bash
# Create
lfd create swift-falcon              # create with name
lfd create                           # create with generated name

# Configure (setters, idempotent)
lfd area swift-falcon src/           # set working area
lfd goal swift-falcon "fix lint"     # set goal (inline or preset name)
lfd flow swift-falcon ship           # set flow (default: ship)

# Run (validates config, starts agent)
lfd run swift-falcon                 # run once
lfd loop swift-falcon                # run continuously (iteration after iteration)
lfd watch swift-falcon               # run when origin/main changes in area
lfd watch swift-falcon --path tests/ # run when origin/main changes in tests/
lfd cron swift-falcon "0 9 * * *"    # run on schedule

# Manage
lfd stop swift-falcon                # stop running agent
lfd list                             # list all agents
lfd show swift-falcon                # show agent details
lfd rm swift-falcon                  # delete agent
```

### One-Shot Commands

Create + configure + run in one command:
```bash
lfd loop swift-falcon --area src/ --goal "fix lint" --flow ship
lfd watch swift-falcon --area src/                    # watch area
lfd watch swift-falcon --area src/ --path tests/      # watch tests/, work on src/
lfd run swift-falcon --area src/
```

Behavior:
- If agent doesn't exist → create it
- If `--area` provided → set it (persisted)
- If `--area` omitted but agent has saved area → use saved
- If `--area` omitted and no saved area → error

### Validation

`run`, `loop`, `watch`, `cron` all validate before starting:
- `area` must be set (required)
- `flow` uses default if not set
- `goal` is optional

### Examples

**Concerto flow (incremental):**
```bash
lfd create swift-falcon    # Concerto calls this
# ... user configures in UI ...
lfd area swift-falcon src/
lfd goal swift-falcon "improve test coverage"
# ... user clicks Run ...
lfd loop swift-falcon
```

**CLI power user (one-shot):**
```bash
lfd loop swift-falcon --area src/ --goal "fix all lint errors"
```

**Restart existing agent:**
```bash
lfd loop swift-falcon      # uses saved config
```

## Stimulus Modes

| Mode | Trigger | Use Case |
|------|---------|----------|
| `run` | Manual, once | One-off task |
| `loop` | After each iteration completes | Continuous improvement |
| `watch` | When `origin/main` has new commits in path | React to upstream changes |
| `cron` | On schedule | Daily maintenance, nightly polish |

`watch` polls `origin/main` and compares to last-seen SHA. If files in the watched path changed, triggers a run. Watched path defaults to area but can be overridden.

## HTTP API

```
POST   /agents           {name?, area?, goal?, flow?}     → create
PATCH  /agents/:id       {area?, goal?, flow?}            → update
GET    /agents                                            → list
GET    /agents/:id                                        → show
DELETE /agents/:id                                        → delete
POST   /agents/:id/run   {stimulus, cron?, path?}         → run
POST   /agents/:id/stop                                   → stop
```

- Create accepts minimal body (even empty → generates name)
- Update accepts any subset of fields
- Validation happens on `/run`, not on create/update
- CLI commands like `lfd area agent src/` map to `PATCH /agents/:id {"area": ["src/"]}`

## Database Changes

`agents` table already exists. Changes:
- `area`, `goal` columns allow NULL (unconfigured)
- `flow` has default value "ship"
- Validation moves from create-time to run-time

## Migration

Existing agents with area/goal set continue to work. New agents can be created without area/goal.
