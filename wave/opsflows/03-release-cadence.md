# 03: `lf release` step + automated cadence

**Finish line:** `lf release` is a step that researches changes, writes narrative notes, and calls ops commands to execute. Runs on cron — patch daily, minor monthly. Skips when empty. Concerto shows release config per repo.

## What to build

**Ops decomposition** — split today's monolithic `lf ops release` into focused commands:

- `lf ops release-notes <version>` — generate notes (LLM-powered)
- `lf ops release-bump <version>` — bump version in manifests
- `lf ops release-tag <version>` — tag and push
- `lf ops release-status` — check CI/publish status
- `lf ops release-check` — check if PRs merged since last tag (exit 1 if empty)

**`lf release` step** (no fast-path — always needs LLM):

```yaml
---
requires: none
produces: tagged release
---
```

Step prompt lists the API and guides the agent:

```
## API

lf ops release-check                              # exit 0 if PRs merged since last tag
lf ops release-notes <version> [--prev-tag TAG]   # generate notes
lf ops release-bump <version>                      # bump manifests
lf ops release-tag <version>                       # tag and push
lf ops release-status                              # check CI status
```

Agent judgment:
1. `lf ops release-check` — skip if nothing merged
2. Analyze changes, decide version (patch/minor/major)
3. Research and write narrative notes
4. Execute bump → notes → tag → push
5. Handle failures

**Cadence waves:**

```yaml
# wave/release-patch/release-patch.yaml
flow: release
stimulus:
  kind: cron
  cron: "0 2 * * *"

# wave/release-minor/release-minor.yaml
flow: release
stimulus:
  kind: cron
  cron: "0 2 1 * *"
```

Cron = floor. Manual `lf release` anytime on top.

**Concerto:**
1. Release config (per-repo) — set cadence, toggle on/off
2. Release now — button with patch/minor/major picker

## Done when

```bash
lf release          # researches, writes notes, releases
lf release          # nothing merged → skips cleanly

# Cron fires at 2am, finds merged PRs, releases automatically
# Concerto shows release config and "Release Now" button
```
