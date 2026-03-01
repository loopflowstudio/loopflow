# 03: Concerto Release UI

**Finish line:** Concerto shows per-repo release config and a "Release Now" button with patch/minor/major picker.

## What to build

Two surfaces:

1. **Release config** — per-repo settings for cadence (cron expression), toggle on/off. The wave configs are the source of truth; Concerto reads and writes them.

2. **Release Now** — button with version picker (patch/minor/major). Triggers `lf release <version>` in the repo's agent environment.

## Context

The decomposed ops commands are the API the agent calls:

```
lf ops release-check    -> exit 0 if changes, exit 1 if empty
lf ops release-bump     <version>
lf ops release-notes    <version>
lf ops release-tag      <version>
lf ops release-status   -> workflow status
```

Version selection is human input, not agent judgment. The "Release Now" button provides that input. Cron waves handle the automated case.

Design doc estimated ~400 LOC of Swift for config editing. "Release Now" button is the higher-value surface — config editing can be minimal (on/off toggle + cron string) since the wave YAML is simple.

## Done when

- Concerto shows release config per repo
- "Release Now" button triggers release with chosen version
- Config toggle enables/disables cron waves
