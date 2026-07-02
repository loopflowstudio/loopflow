# Open questions / assumptions — waveagent-roadmap

Executive decisions made during headless implementation, flagged for review:

- **`roadmap:` handle format.** Supports both `asana://<project_id>` (scheme
  picks the provider directly) and a bare project id (falls back to the
  wave's `pm:` block for the provider). The design doc's phrasing ("e.g.
  `asana://<project_id>` or a project id via the existing `WavePmConfig`")
  was ambiguous between these two, so both are accepted.

- **`lf op roadmap update --status`.** `PmProvider` only exposes
  `complete_item` (no generic status enum, no reopen). Implemented `--status
  done|complete|completed` as calling `complete_item`; any other value is a
  hard error rather than silently ignored. If richer status is needed later,
  the trait needs a new method first.

- **CLI surface trimmed, not just added-to.** Removing the on-disk mirror
  meant `pm_pull`, `pm_import`, `pm_export`, `pm_push_diff`, `pm_status`, and
  `pm_try_claim` had no remaining purpose (their only job was
  reading/writing `wave/<name>/*.md` mirror files) and were deleted along
  with their CLI subcommands, not just left dead. `lf op pm init` is the one
  survivor — kept as directed ("leave `pm init` as-is"), simplified to drop
  local-item linking since that depended on the now-deleted mirror reader.

- **`lf op ingest`'s PM auto-refresh removed.** It previously called
  `pm_pull`/`pm_try_claim` before picking a local item. Since PM-backed waves
  no longer have local mirror files to refresh, that logic is gone; PM-backed
  waves now rely on the agent calling `lf op roadmap` directly (per the
  updated `govern/step/ingest.md`), not on the `lf op ingest` fast path.

- **Deleted every top-level `wave/<name>/N-*.md` file**, not just
  PM-connected ones (per the design doc's literal instruction). This leaves
  `desktop`, `goals`, `root`, and `workflows` waves with an empty backlog
  until either new local items are authored or a `roadmap:` handle is wired
  up. `wave/<name>/items/*.md` (release, website) is a different, deeper
  path and was left untouched — the design doc's pattern only matched files
  directly under `wave/<name>/`.

- **`wave/<name>/<name>.yaml` files** (desktop.yaml, mobile.yaml, root.yaml,
  release.yaml, website.yaml, workflows.yaml) appear to be dead — no Rust or
  Python code reads them (config lives in `GOAL.md` frontmatter instead).
  Left untouched since removing them is out of scope for this PR, but worth a
  follow-up cleanup pass.

- **No wave in this repo has a `roadmap:` handle set yet.** This PR ships the
  plumbing (`lf op roadmap`, `lf op roadmap update`); actually pointing a
  wave (e.g. `goals`) at a live Asana project is a follow-up — someone needs
  to create the project and set `roadmap: asana://<id>` in that wave's
  `GOAL.md`.
