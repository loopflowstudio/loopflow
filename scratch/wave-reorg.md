# Wave Reorganization — 2026-04-24

Reshape 13 waves down to 4 active (+ `old/` archive + `roadmap/` live-PR).

## Final shape

| Wave | Purpose | Absorbs |
|---|---|---|
| **root** | Root chord. Gardens the others. Item added: unify `review-open-work` with build/garden status-meeting flows. | — |
| **desktop** | Concerto macOS. Priority 1: embedded terminal as build-driver. Priority 2: native chat UX. | chatgui, chattui, macos (subset) |
| **mobile** | View-only waves + roadmap from remote lfd. Chat later. Self-contained: scope in lfd/model deps. | ios (reshape, fresh README) |
| **workflows** | Engine, providers, flows, governance UX. | lfd, model, pm, gstack, flows, runboard, macos (subset) |
| old | Archive (unchanged) | — |
| roadmap | Live worktree + PR #659 (unchanged until landed) | — |

## Item placement

### root (new item)
- **review-open-work ↔ build/garden parity.** New item inside `wave/root/`. Think through how the manual `review-open-work` step relates to `govern-coordination`, `garden-act`, and the automated build/garden status-meeting flows. What's shared, what's specific, what belongs where.

### desktop
From **macos**:
- `01-window-composition-polish.md`
- `4-terminal-tabs.md`
- `4-wave-lifecycle-ui.md`
- `4-typed-auth-ui.md`

From **chatgui** (demoted to priority 2):
- `01-animation-smoothness.md`, `02-markdown-rendering.md`, `03-conversation-history.md`, `04-composer-upgrades.md`

From **chattui** (folds into embedded-terminal priority):
- `01-step-launch-in-terminal.md`, `02-reattach.md`, `03-multi-agent-dispatch.md`

### mobile
Fresh README scoping to view-only remote-lfd. Existing `ios/` items evaluated individually — most likely drop (testflight, mac-mini, team-workflow) or rescope.

### workflows
From **lfd**: all 4 items
From **model**: all 9 items
From **pm**: all 4 items
From **gstack**: all 3 items
From **flows**: all 4 items
From **runboard**: all 4 items
From **macos** (governance UX): `01-beat-synthesizer.md`, `4-concerto-release-ui.md`, `4-calibration-view.md`, `4-portfolio-view.md`

## Deletes

- `wave/redesign/` — superseded by `root/`
- `wave/roadmap.md` — orphan top-level file
- Source wave dirs after migration: `chatgui/`, `chattui/`, `macos/`, `ios/`, `lfd/`, `model/`, `pm/`, `gstack/`, `flows/`, `runboard/`

## Config updates

- `root.yaml` area: `[wave/desktop, wave/mobile, wave/workflows]`
- `root/README.md`: refresh stale `chord-model` / `agent-embedding` references

## Worktree rotation

After wave dir migration:
- **Create:** `desktop`, `mobile`, `workflows` worktrees
- **Destroy:** `chatgui`, `chattui`, `flows`, `gstack`, `ios`, `lfd`, `macos`, `model`, `pm`, `redesign` (never had a wave dir anyway), `runboard`, `old` (no worktree needed for archive)
- **Keep:** `root`, `roadmap`, `gstack-debug`, `release` (post-release residue; harmless)

## PM-ID preservation

Several items have `linear_id` / `notion_id` / `asana_id` frontmatter. Moving the file within the repo is fine — PM IDs stay in frontmatter and providers keep their link by ID, not path. No PM sync dance required for the reorg itself.

## Execution order

1. Create `wave/desktop/`, `wave/mobile/`, `wave/workflows/` with new READMEs + yaml.
2. `git mv` items into new homes (preserve frontmatter).
3. Add root roadmap item.
4. Delete source wave dirs (11 total) + `redesign/` + `roadmap.md` orphan.
5. Update `root.yaml` area + `root/README.md`.
6. Rotate worktrees: destroy obsolete stubs, create `desktop`/`mobile`/`workflows`.
7. Commit in the root worktree.
