# 03: Listen Authoring

Wire listen stimuli into wave schema files and explore richer inter-wave communication beyond "source completed, fire me."

## What exists after this

Users can declare `listen` stimuli in wave schema files (`.yaml`), not just via the API. Optionally, a listen-triggered wave gets context about *what* its source did (PR content, changed files) rather than just knowing it ran.

## What Phase 01–02 established

Phase 01 added `StimulusKind::Listen` and `source_wave_id` to the stimulus model, persisted in both SQLite and Postgres. The Python client supports `add_stimulus(..., kind="listen", source_wave_id="infra")`. Phase 02 shipped chord CRUD so waves now have first-class named groups.

The listen stimulus currently fires when the source wave completes. The listening wave starts a normal run — it doesn't know *what* the source did, only that it ran.

## What to build

### Schema file support

Currently `parse_schema_stimulus` handles `once`, `loop`, `watch`, and `cron`. Add `listen`:

```yaml
stimulus:
  kind: listen
  source: infra  # wave name or ID
```

### Source context injection (optional/exploratory)

When a listen-triggered wave starts, inject context about the source wave's last run:
- PR title and summary from the source's most recent run
- Changed file list
- Optionally full diff content (configurable depth)

This is the essence of the old "listen step" concept, adapted to the flat model. Instead of a special step in a chord iteration, it's context assembled when a listen stimulus fires.

### Terminology cleanup

Finish the sidecar → listening naming cleanup wherever stale names remain in the codebase. Phase 01 covered the data model but display strings, comments, and variable names may still reference the old terminology.

## Open questions

- Should source context injection be a separate phase? It adds scope beyond schema authoring.
- What context depth levels make sense? (title-only / summary / full diff)
- Should the listening wave see only its direct source, or all waves it listens to?

## Done when

- `listen` stimulus kind accepted in wave schema YAML files
- Schema-defined listen stimuli are persisted correctly with `source_wave_id`
- Stale sidecar/voice terminology cleaned up
- Source context injection designed (and optionally implemented)
