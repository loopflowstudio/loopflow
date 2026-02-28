# 03: `lf release` step + automated cadence

**Finish line:** `lf release` is a step that researches changes, writes narrative notes, and calls ops commands to execute. Runs on cron — patch daily, minor monthly. Skips when empty. Concerto shows release config per repo.

## What to build

**Ops decomposition** — split today's monolithic `lf ops release` into focused commands that the step prompt exposes as its API:

```
lf ops release-check                              # exit 0 if PRs merged since last tag
lf ops release-notes <version> [--prev-tag TAG]   # generate notes (LLM-powered)
lf ops release-bump <version>                      # bump manifests
lf ops release-tag <version>                       # tag and push
lf ops release-status                              # check CI status
```

**`lf release` step** (no fast-path — always needs LLM).

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

Cron = floor. Manual `lf release` anytime on top. Set up in the loopflow repo as the first consumer.

**Concerto:**
1. Release config (per-repo) — set cadence, toggle on/off
2. Release now — button with patch/minor/major picker

## Context from sprint 02

Sprint 02 shipped narrative release notes within the existing `lf ops release` path. Key decisions that affect this sprint:

- **`MergedPr` now carries diff stats** (`additions`, `deletions`, `changed_files`) from `gh pr list --json`. The decomposed `lf ops release-notes` command inherits this.
- **Full PR bodies, no truncation.** The 400-char limit was removed. If prolific releases (50+ PRs) cause prompt size issues, that's a release frequency problem — which this sprint's cron cadence directly addresses.
- **Previous `RELEASE_NOTES.md` is fed as context.** As notes accumulate over many releases, may need truncation to last N releases to avoid prompt bloat.
- **Prompt quality is empirical.** The narrative template reads well but hasn't been tested on a real multi-PR release yet. This sprint's first cron-triggered release will be the real test.
- **No tests for `format_pr_for_prompt()`.** Acceptable for sprint 02's scope, but as this sprint decomposes release into sub-commands, consider testing the formatting pipeline.

## Done when

```bash
lf release          # researches, writes notes, releases
lf release          # nothing merged → skips cleanly

# Cron fires at 2am, finds merged PRs, releases automatically
# Concerto shows release config and "Release Now" button
```
