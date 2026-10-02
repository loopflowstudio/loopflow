# v0.12.30

v0.12.30 brings a Task’s conversations, terminals, files and Flow progress into one retained workspace, making it easier to return to work and find what needs a reply. Background checks keep enrolled Tasks moving, while CI watchers handle failed recorded landings. Context previews and usage reports expose what agents receive and where tokens and time go; operators should review the Home and command compatibility changes below before upgrading.

## Return to the work, with its place preserved

Desktop now keeps Task work together across visits. Shared checkout membership brings independently started conversations into the same view, while pending reviews and independent Flows remain protected during completion and cleanup.

- Task workspaces retain Sessions, running shells, unfinished file drafts and split layouts when switching Tasks. Option-click opens a Session alongside another; hiding a pane keeps its process alive.
- Browse checkout files and save revision-checked edits without a Project or PR. Files reached through symlinks remain read-only, and drafts survive access changes.
- Use **⌘K** to find repository Waves, Tasks, Sessions and Flows, with recent destinations first. Search covers current Tasks and visited historical Tasks, not all Task history. `loopflow://task/ISSUE` links open details without starting work.
- **Needs me** and `lf session list --needs-me` surface unresolved Asks and current reviews even beneath collapsed Tasks. Confirmed exited orphan conversations leave the working set while retaining history; Task/Wave conversations, primary Sessions and pending obligations are preserved.
- `lf session ensure` returns the repository’s ongoing conversation; add `-w <wave>` for a Wave conversation. `lf session replace SESSION` starts a successor while retaining completed history.
- Flow previews fold composed templates, and Task workspaces link interactive stages to their exact conversations. Automated steps, branches and repeats remain inspectable.

## Inspect plans before starting, and reorganize without losing work

Planning lookup no longer depends on a local execution record. Existing branches and nested Waves can keep their identity and progress as work is adopted or reorganized.

- `lf task status <issue> --json` reports planning, freshness, errors and optional execution without allocating a worktree. Project-less issues remain inspectable, and failed refreshes retain the last successful observation.
- Checkout can adopt a Linear branch, reuse a dirty worktree or fetch an open PR’s branch. Repeated operations preserve the PR and captured Flow cursor. Managed continuation rechecks planning and pauses on unavailable or incompatible plans without discarding progress.
- Nested Waves inherit repository and ancestor context. Durable `id:` fields preserve Wave identity and Project/Task links across renames and reparenting; commit those fields and retain them when moving directories.
- Chapter sweeps, planning refresh and Wave sync skip Projects known to belong exclusively to other Teams. Missing or conflicting ownership still needs resolution, but unrelated foreign Projects no longer stop the repository’s work.

## Keep delivery moving between visits

Scheduled repository checks resume enrolled Tasks and settle verified merges while Desktop is closed. CI repair has a separate watcher, so delivery can be monitored from Desktop, a terminal or an installed service.

- `lf land` records delivery and returns; a Flow’s landing step still waits for verified merge before advancing. `lf task reconcile` checks Tasks and landings once, while `lf pr reconcile` checks delivery alone.
- Per-Home minute checks respect reviews, live work and bounded retries. Historical Tasks remain unenrolled until selected. Background progress and `lf task automation --json` show schedule coverage, errors, blockers and pending delivery.
- Desktop starts a CI watcher for each open repository. `lf ci watch` also supports one-pass polling, service installation and status inspection, with rate-limit backoff and duplicate-repair prevention.
- Ordinary CI repairs require both a recorded landing and an active watcher. Scheduled reconciliation records failures and settles merges but does not start those repairs; without a watcher, failed ordinary landings wait. Release-owned landings retain their repair path, and failing PRs without recorded landings are reported only.

## See and control what agents receive

Launch budgets are configurable and inspectable, and recorded context can be broken down by source. The final defaults preserve more memory while reducing scratch input; complete excerpted files remain available on disk.

- `lf context --skill implement` previews local headless input without launching a provider or refreshing Linear. Reports show original and submitted usage, effective limits and configuration sources. Overrides resolve from Wave frontmatter, repository config, personal config, then defaults.
- Memory defaults are **16,000 tokens / 128 KiB**, shared across repository, ancestor and selected Wave memory. Scratch defaults are **12,000 tokens / 96 KiB**. Total-input limits remain **64,000 tokens / 512 KiB**.
- Wave documents reach IDE launches through assembled prompts, and repeated document sources are deduplicated. `realign` consults child memories for shared lessons and curates oversized memory without shrinking files merely because they are large but within budget.
- `lf usage --context` and Desktop Session history show source breakdowns and budget overruns. Missing measurements stay unknown. Counts mix local and provider tokenizers; carried context is an estimate and is unavailable for Codex.
- `lf usage --weekly` reports input/output ratios, input size, median and p90 turn duration, and long silences within completed turns. Unfiltered reports publish the latest complete week’s metrics; Wave, Project and Task filters remain read-only.
- `lf usage --binds` compares recorded attribution with hypothetical whole-conversation attribution. Binding still leaves earlier usage with its original owner; this report does not reassign it.

## Operational notes

- **One Home, one database:** ordinary source CLI commands forward to the installed CLI and main `~/.lf` Home. An explicit `LF_HOME` selects an isolated experiment inherited by children. Each Home uses `loopflow.db`; `LF_DB_PATH` and retired `LF_CONTROL_*` overrides are removed. Existing side stores and custom database paths are not imported or migrated.
- **Experimental schemas:** use a fresh experimental Home after schema changes, including for Homes containing the removed draft receipt ledger. Published installation retains snapshot validation, released-schema migrations and interrupted-install recovery; intermediate branch schemas are not an upgrade contract.
- **CLI and JSON consumers:** commands are organized under their owners, with uniquely resolving shorthand such as `lf land`, `lf ps` and `lf top` retained. Task status JSON now uses a planning/execution envelope; Task snapshots replace `runs` and `runs_truncated` with `work`, Session records add `task_ids`, and Project-level sweep entries can have `issue: null`.
- **Background operation:** scheduling uses macOS launchd and requires the Home’s user to remain logged in. Holding a Task or disabling its schedule prevents future automatic admissions; running work and existing GitHub merge requests continue. CI repair confirmation requires GraphQL quota. Watcher status is currently available through the CLI, without a dedicated Desktop view.
- **Acceptance limits:** supplied PR evidence includes focused Rust, Swift and Python checks, native builds and a live GitHub polling pass, but does not establish a clean full matrix for the combined release. Live-provider adoption, repair and merge, installed Home acceptance, and full cross-Task retention remain unverified end to end. Context-report readers were exercised against real captures; weekly live-data validation remains outstanding.

## Small changes

- Repository conventions now live in one regular `AGENTS.md`; the repository’s `STYLE.md` and `CLAUDE.md` symlinks are removed. Providers load repository instructions natively, outside injected context.
- The migration helper reuses the Task branch’s existing draft without changing its SQL. Edit that draft in place and test the released schema against the finished draft.
- Newly started tmux servers and Session shells clear inherited execution context. Already-running tmux servers retain their previous environment.
- Contributors can inventory recorded context and replay source-removal comparisons with `scripts/context_ablation.py`. The retained pilot does not establish equivalent code quality or reliable time/cost savings; replays consume provider usage.
