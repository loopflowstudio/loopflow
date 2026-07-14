# Multi-PR Tasks with readable workspaces

## What to build

A Linear Task owns one durable Task Session and one stable worktree. An ordered
sequence of serial PRs owns the branches that advance it. Task completion is
explicit and independent from PR merge; `lf pr land -c` keeps the one-PR happy
path terse.

> im pretty sure task = multiple Prs is the right model (it also answers the question: what about tasks that arent completed via PR?), but we should have an API that is super well grooved for the 1 pr = 1 task happy case

> Not PullRequest. Just PR.

> we need some way to say "if we havent yet actually created a github PR, we should now because we actually do intend to commit this code at this point"

## Demo

```bash
lf task run W2-127 --name release-scoped-migrations
# work
lf pr land -c
```

The Task keeps one worktree. A multi-PR Task changes only the first landing call:

```bash
lf pr land --next released-upgrade-proof
# merge rotates the same worktree onto a fresh branch from origin/main

# work the next PR
lf pr land -c
```

A clean investigation Task completes without publishing an empty PR:

```bash
lf task complete W2-200 --summary "Root cause and evidence recorded in Linear"
```

## Core model

Keep two durable nouns: Task and PR. GitHub is evidence attached to a PR, not a
third lifecycle owner.

```rust
pub struct TaskSession {
    // stable identity, worktree, process, provider transcript, and directives
    pub worktree: PathBuf,
    pub workspace_slug: String,
    pub status: TaskSessionStatus,
}

pub struct TaskPr {
    pub id: TaskPrId,
    pub task_session_id: TaskSessionId,
    pub sequence: u32,
    pub slug: String,
    pub branch: String,
    pub base_commit: String,
    pub publication: Option<PrPublication>,
    pub merge_commit: Option<String>,
    pub abandoned_at: Option<OffsetDateTime>,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

pub struct PrPublication {
    pub requested_at: OffsetDateTime,
    pub after_merge: AfterMerge,
    pub next_slug: Option<String>,
    pub github: Option<GithubPr>,
}

pub struct GithubPr {
    pub number: u32,
    pub url: String,
}
```

Do not persist PR state. Derive it from durable evidence:

- `working`: no publication, merge, or abandonment evidence;
- `publishing`: publication requested, no GitHub receipt yet;
- `open`: publication contains a GitHub receipt;
- `merged`: `merge_commit` exists;
- `abandoned`: `abandoned_at` exists.

Abandonment and merge are mutually exclusive. A merge requires a GitHub receipt;
the receipt can exist only inside publication. `after_merge=complete_task` cannot
name a next slug. Empty is not a state or persisted variable: it is computed for
the active PR from the worktree and its base commit.

The nested publication makes invalid combinations unrepresentable in Rust. It
also preserves the important failure window: record intent before calling GitHub,
then attach GitHub's receipt. A crash between those writes leaves `publishing`,
which is visibly retryable instead of ambiguous.

## Persistence and store API

`task_prs` stores the nested model as constrained columns:

- `publication_requested_at` and `after_merge` are both present or both absent;
- `github_number` and `github_url` are both present or both absent;
- GitHub evidence requires publication evidence;
- merge evidence requires GitHub evidence;
- merge and abandonment cannot coexist;
- one partial unique index permits at most one PR without merge or abandonment.

The store owns these atomic boundaries:

```rust
reserve_task_session_with_pr(task, initial_pr, directive)
task_prs(task_id)
active_task_pr(task_id)
update_task_pr(pr)
settle_task_pr(pr, next_pr)
complete_task_session(task, skipped_pr)
complete_task_session_after_pr(task, merged_pr)
```

Completing a clean Task deletes its active unpublished empty PR in the same
transaction as Task completion. It never enters PR history. Published or
abandoned PRs remain observable in order. A completing merge settles its PR and
completes the Task in one transaction, so recovery cannot rotate past it.

## Lifecycle APIs

Publication is deliberately two operations around the GitHub side effect:

```rust
request_task_pr_publication(repo, after_merge, next_slug)
attach_task_github_pr(repo, github_pr)
reconcile_task_pr(store, task)
ensure_working_pr(store, task)
abandon_task_pr(repo, force)
task_complete(issue, summary)
task_snapshot(task)
```

`lf pr open`, `land`, and `submit` commit/rebase first, reject empty work, persist
the publication request, call GitHub, then attach the receipt. Retrying is
idempotent. Re-running land/submit before merge updates `after_merge`; the last
call wins.

Reconciliation adopts a manually created GitHub PR by synthesizing a Review
publication and attaching its receipt. Observed merge records the merge commit.
`CompleteTask` completes the Task; `Review` leaves it open for another PR.
Observed close records abandonment.

The runner rotates only after merge or abandonment. It fetches the remote default
branch, requires a clean stable worktree, checks out a readable fresh branch,
and atomically appends the next Working PR. An Open PR bars automatic restart;
an operator may still explicitly resume to answer review.

## Wire contract

Both Rust and Swift expose the complete decision surface without defaults:

```text
TaskDetailSnapshot
  prs: [PrSnapshot]
  active_pr: PR id?

PrSnapshot
  id, sequence, slug, branch, base_commit
  phase
  empty: bool?                 # known only for the active worktree PR
  publication:
    requested_at
    after_merge
    next_slug
    github: { number, url }?
  merge_commit
  abandoned_at
```

The UI must be able to answer: which PR is active, whether GitHub creation is
pending, and whether merging the open PR completes the Task.

## Readable naming

- First branch: `<author>/<workspace-slug>`.
- Stable worktree: `<repo>.<workspace-slug>`.
- Continuation branch: `<author>/<workspace-slug>-<pr-slug>`.
- Without `--next`, use the PR sequence number.
- Base new PRs on fetched `origin/main`, never a possibly-ahead local main.
- On collision, report the owner and require a semantic retry; do not append an
  opaque ID.

## Done when

- A one-PR Task uses `lf pr land -c` and completes only after observed merge.
- A two-PR Task keeps one worktree, rotates through two readable serial branches,
  retains both PRs in order, and completes after the second merge.
- A publishing failure remains observable and retryable without a GitHub receipt.
- A manually created or merged GitHub PR is adopted with Review disposition and
  does not implicitly complete the Task.
- A clean investigation Task completes with no PR history; dirty, committed, or
  published work rejects direct completion.
- An abandoned PR leaves the Task open and rotation starts fresh from fetched
  main.
- Concurrent start/reconcile calls produce one Task Session and one active PR.
- Rust and Swift show the active PR and whether its merge completes the Task.
- Naming collisions request a semantic retry rather than exposing an opaque ID.
- `cargo fmt`, `cargo clippy -- -D warnings`, full Rust tests, Swift tests,
  migration checks, DTO fixtures, and simulated review pass.
