# Review: Workflow Docs & Wave Guide

## What was implemented

Three docs changes that reframe loopflow's documentation as a progressive journey (try it → build features → waves → remote) and add the previously undocumented wave authoring guide.

- **`docs/getting-started.md`** — Restructured from a flat feature list into a progressive journey. Sections renamed (Quick Fix → Try It, Feature Workflow → Build Features, Waves → Scale with Waves). Added demo repo, named flows, custom steps, Go Remote, and tmux Plugin sections. Setup Paths demoted to subsection under Install.

- **`docs/wave-authoring.md`** (new) — End-to-end guide from creating a wave to monitoring an auto-looping backlog. Covers: Creating a Wave (Concerto/lfq/Python API), Wave Directory structure (README, numbered items, config YAML), Drafting Content (`lf design` vs. hand-written), The Auto-Loop lifecycle (ingest → kickoff → build → update-wave), Running and Monitoring (Concerto/lfq/Python API/stimulus types), and a Worked Example using `wave/infra/`.

- **`docs/index.md`** — Added "The Journey" routing table between the quick-start examples and "Why Flows?" All existing content preserved. Next links expanded to include wave-authoring.md.

- **`docs/waves.md`** — Added wave-authoring.md to Next links.

- **`docs/_config.yml`** — Removed nonexistent `agents.md` from header_pages, added `wave-authoring.md`.

- **Wave items** — `wave/docs/01-setup.md` (completed) and `wave/docs/02-docs.md` (ingested) removed.

## Key choices

**Progressive journey over parallel paths.** The docs follow actual adoption: CLI tinkering → manual step chains → waves → remote. Concerto appears at the wave escalation point, not as a separate track.

**Three-layer distinction (lf / lfq / Concerto).** `lf` is manual mode (no lfd). `lfq` and Concerto talk to lfd. Wave authoring docs are Concerto-native with lfq CLI equivalents. `lf design` is positioned as drafting, not conducting.

**Evolving features handled lightly.** Listen stimulus mentioned but not over-explained. Remote/auth kept brief. No references to unshipped features (direction aliases, cross-repo areas, sandbox details, voice control, cost analytics).

**`agents.md` removed from nav.** The file didn't exist — the config reference was stale.

## How it fits together

`index.md` serves as the entry point with a routing table pointing to `getting-started.md` (progressive journey) and `wave-authoring.md` (wave deep-dive). `getting-started.md` progresses from install through try-it, build-features, waves, remote, and tmux. The waves section links forward to `wave-authoring.md` for the full guide. `wave-authoring.md` links back to `waves.md` for stimulus details and `getting-started.md` for broader context. All pages cross-link to each other and the reference pages.

## Risks and bottlenecks

- **Content accuracy not verified.** The design doc explicitly defers accuracy checking to sprint 03. Command syntax, API signatures, and wave directory conventions are based on tribal knowledge in the prompts — not validated against running code.
- **`waves.md` not in header_pages.** Pre-existing condition, not introduced by this branch. The page is reachable via cross-links but not in the site nav. Worth considering in a follow-up.

## What's not included

- Concerto feature docs or tutorials
- Gemini CLI documentation
- README.md changes (handled in sprint 01)
- Content accuracy verification (sprint 03)
- `waves.md` addition to header_pages (pre-existing gap)
