# Multi-PR Tasks review

## What was implemented

Task Sessions now keep one stable worktree across an ordered sequence of PRs. Each PR owns its branch, base commit, publication intent, optional GitHub receipt, merge evidence, and abandonment evidence. A PR merge either returns the Task to review for another PR or completes the Task; clean investigation work can complete without opening an empty PR.

The CLI exposes the distinction through `lf pr land --next <slug>`, `lf pr land -c`, and `lf task complete`. Task snapshots send ordered `prs` and `active_pr` to Swift, including whether merging a published PR completes the Task.

## Key choices

- Kept two domain nouns: Task and PR. There is no separate Delivery model.
- Derived `working`, `publishing`, `open`, `merged`, and `abandoned` from durable evidence instead of storing another mutable state label.
- Nested the GitHub receipt under publication, making GitHub-without-publication unrepresentable in Rust and invalid in SQLite.
- Kept worktree emptiness orthogonal to phase. It is observable for the active PR but is not another lifecycle state.
- Persisted publication intent before calling GitHub. A failed `gh` call therefore leaves a retryable `publishing` PR with its completion disposition intact.
- Settled a completing PR and its Task in one SQLite transaction. Rotation also settles the old PR and inserts the next one atomically.
- Kept `after_merge` on the Rust/Swift wire as one string enum so the UI can answer “does this PR complete the Task?” without an extra wrapper type.

## How it fits together

`TaskSession` owns stable execution identity, worktree, provider transcript, and Task completion. Ordered `TaskPr` rows own branch-level progress. `ops::task` reconciles GitHub evidence into those rows and rotates the same worktree only after the active PR settles; the store enforces one active PR per Task.

`lf status --json` projects the full PR history into the shared DTO. The Mac detail pane links published PRs and labels any PR whose merge completes the Task.

## Risks and bottlenecks

- Reconciliation polls `gh`; merge recognition is eventual when no Task process is active.
- Legacy merged Tasks migrate with `legacy-unknown` as their merge commit because the previous schema did not retain the SHA.
- Rotation requires a clean worktree and an unused local/remote branch name; collisions stop with a recovery error instead of guessing.
- The migration rewrites nested persisted event payloads. Its populated-database test now runs with production foreign-key enforcement enabled.

## What's not included

- Parallel active PRs for one Task.
- PR targets other than `main`.
- Opening empty GitHub PRs; empty working PR rows are removed when a Task completes directly.
- Provider-neutral forge receipts; publication is generic, while the attached receipt deliberately models GitHub because `gh` is the current integration.

## Validation

- `uv run python scripts/test.py --all`: PASS (Python, Rust fmt/clippy/nextest, website, Swift, E2E, macOS app test build).
- `uv run python scripts/check_migrations.py`: PASS; two shipped migrations unchanged and `0.11.001_task_prs` ordered correctly.
- Rust coverage proves ordered atomic rotation, completing-merge atomicity, empty-PR skipping, publication failure recovery, manual GitHub adoption, and derived PR phases.
- Rust/Swift DTO fixture proves `active_pr`, GitHub identity, and `after_merge = complete_task` cross the wire.

