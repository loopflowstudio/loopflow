# GitHub CI Hook

Automatically fix CI failures on wave PRs. When GitHub Actions fails on a wave's PR branch, lfd spawns a fix agent on an ephemeral worktree, pushes the fix, and cleans up.

## What to build

Two trigger paths feed the same fix pipeline:

1. **Webhook** (`POST /v0/hooks/github`) — GitHub pushes `check_run` completed+failure events to lfd in real-time
2. **Poll** — on Concerto startup (or explicit API call), lfd checks CI status for all wave PR branches via GitHub REST API

When a CI failure is detected and matched to a wave, lfd spawns a fix agent on an ephemeral worktree. This is the first case of wave parallelism — the main flow keeps running while the fix agent works alongside it.

## Data structures

```rust
// GitHub webhook payload (subset we care about)
#[derive(Debug, Deserialize)]
struct GitHubCheckRunEvent {
    action: String,                    // "completed"
    check_run: CheckRun,
    repository: GitHubRepository,
}

#[derive(Debug, Deserialize)]
struct CheckRun {
    id: u64,
    name: String,
    head_sha: String,
    status: String,                    // "completed"
    conclusion: Option<String>,        // "failure", "success", etc.
    pull_requests: Vec<CheckRunPR>,
    html_url: String,                  // link to the check run (logs)
}

#[derive(Debug, Deserialize)]
struct CheckRunPR {
    number: u32,
    head: CheckRunRef,
}

#[derive(Debug, Deserialize)]
struct CheckRunRef {
    #[serde(rename = "ref")]
    branch: String,
    sha: String,
}

#[derive(Debug, Deserialize)]
struct GitHubRepository {
    full_name: String,                 // "owner/repo"
}
```

```rust
// New event variant
Event::CiFailure {
    wave_id: LfdId,
    wave_run_id: LfdId,
    pr_number: u32,
    branch: String,
    commit_sha: String,
    check_name: String,
    logs_url: String,
    timestamp: OffsetDateTime,
}
```

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum WaveRunKind {
    Main,
    Sidecar,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum SidecarKind {
    CiFix,
}

impl WaveRun {
    fn is_main(&self) -> bool {
        self.run_kind == WaveRunKind::Main
    }
}
```

```rust
// GitHub config — stored alongside wave or in lfd config
// Waves need to know their GitHub remote to match webhooks
struct GitHubConfig {
    webhook_secret: String,            // HMAC-SHA256 shared secret
    token: Option<String>,             // PAT or GitHub App token for polling API
}
```

```rust
// Generic cleanup target so one janitor can handle all parallel run types.
#[derive(Debug, Clone, PartialEq, Eq)]
struct EphemeralWorktree {
    path: String,
    owner_kind: EphemeralOwnerKind, // fork | sidecar
    owner_id: String,               // fork_run.id or wave_run.id
}
```

## Key functions

### Webhook receiver

```rust
// POST /v0/hooks/github
// Verifies HMAC-SHA256 signature, parses check_run event, matches to wave, emits CiFailure
async fn github_webhook_handler(
    State(state): State<HttpState>,
    headers: HeaderMap,
    body: Bytes,
) -> ApiResult<Json<Value>>
```

Signature verification:
- Read `X-Hub-Signature-256` header
- Compute `HMAC-SHA256(webhook_secret, raw_body)`
- Compare with constant-time equality
- Reject on mismatch (401)

Matching logic:
- Parse the `check_run` event
- Skip if `action != "completed"` or `conclusion != "failure"`
- Extract PR branch names from `check_run.pull_requests`
- Look up all waves with active PRs (from `WaveRunSnapshot.pr.branch`)
- Match by `repository.full_name + branch` (not branch-only)
- Emit `CiFailure` event for each matched wave
- Deduplicate in-memory by `(wave_id, commit_sha)` for the current lfd process lifetime (no DB persistence in v1)

### CI poller (on-demand)

```rust
// Called on Concerto startup or via POST /v0/waves/{id}/check-ci
// Polls GitHub API for check run status on a wave's PR branch
async fn poll_ci_status(wave: &Wave, run: &WaveRun) -> Result<Vec<CiFailure>>
```

Uses `GET /repos/{owner}/{repo}/commits/{sha}/check-runs` where SHA comes from the PR's head commit. Requires a GitHub token (PAT or app token).
v1 behavior: one-shot polling on startup (plus explicit endpoint), not periodic background polling.

### Wave-to-GitHub matching

```rust
// Derive GitHub owner/repo from wave's local repo path
// Parse the `origin` remote URL to extract "owner/repo"
fn github_repo_from_local(repo_path: &Path) -> Option<String>
```

This avoids storing GitHub repo info separately — just read the git remote.

### Fix agent spawner

```rust
// On CiFailure event:
// 1. Create ephemeral worktree from the PR branch
// 2. Build clipboard/context payload from failure metadata
// 3. Run fix agent with the existing debug prompt flow (no CI-specific prompt fork)
// 4. Push fix commit to PR branch
// 5. Clean up ephemeral worktree
async fn spawn_ci_fix_agent(
    store: &SharedStore,
    executor: &WaveExecutor,
    failure: &CiFailure,
) -> Result<()>
```

Ephemeral worktree naming: `{repo}.{wave_name}.ci-fix.{short_id}` — siblings to the main wave worktree, easy to identify and clean up.

Fix-agent prompt input should include: check URL, check name, failing SHA, and branch (more context, minimal token cost), but these are examples of context fields, not hard assumptions in the prompt contract.

### Debug prompt contract (shared path)

- Reuse the existing `lf debug` prompt path for both:
  - automatic CI-failure runs
  - ad-hoc/manual debug usage
- Do not introduce a second CI-specific debug prompt.
- CI metadata is optional enrichment. If unavailable, debug should still work from plain clipboard/error text.
- Any CI-specific formatting should be additive wrappers around the same underlying debug prompt behavior.

### Unified startup janitor

```rust
// Runs on startup (and safe to call periodically later).
// Cleans stale ephemeral worktrees across fork + sidecar runs.
async fn run_worktree_janitor(
    store: &SharedStore,
    repo_roots: &[PathBuf],
) -> Result<JanitorReport>
```

Janitor approach:
- Discover active ephemeral worktrees from DB:
  - fork runs (`fork_runs`) still pending/running
  - sidecar wave runs (`wave_runs`) with `run_kind=sidecar` and active status
- Enumerate git worktrees under each repo root.
- Remove stale paths that match ephemeral naming patterns but are not active in DB.
- Keep this as one janitor function so future sidecars reuse the same cleanup path.
- Reuse existing fork cleanup behavior (`cleanup_fork`) as the happy-path cleanup; janitor handles crash leftovers.

## Constraints

**Webhook secret must be configured.** No unsigned webhooks accepted. The secret is stored in lfd config (not per-wave — one endpoint handles all repos).

**One fix agent per failure.** If CI fails multiple checks, each gets its own CiFailure event, but we deduplicate by commit SHA + wave — only spawn one fix agent per commit that fails.

**Fix agents don't block the main flow.** This is the key parallelism change. Today `get_active_wave_run` is used to prevent concurrent runs. Fix agents are sidecar runs, not main runs.

Store `run_kind` on `WaveRun` (the run is the execution object). For sidecars, add optional `sidecar_kind` (set to `ci_fix` for this feature). All existing "active wave run" logic should filter to main runs.

**Ephemeral worktrees are cleaned up.** After the fix agent completes (success or failure), remove the worktree. Also run a startup janitor to clean stale ephemeral worktrees from killed/crashed runs. Janitor should be shared across fork/sidecar parallelism.

**GitHub token for polling is optional.** Webhooks work without a token. Polling requires one. Degrade gracefully — if no token, skip polling and rely on webhooks, and log one warning at startup.

**Parse origin remote, don't store GitHub repo separately.** The wave already has a local `repo` path. Derive `owner/repo` from the git remote URL at runtime.

**Prompt reuse over specialization.** Treat CI fields as example inputs to the same debug pipeline, not assumptions that always exist.

## What to defer

- **Fetching full CI logs from GitHub** — v1 passes URL + check name + SHA + branch to the agent. Structured log extraction (via check run annotations API) is a refinement.
- **Auto-registering webhooks** — manual `gh api` setup is fine for now.
- **Multiple fix attempts** — if the fix agent fails, don't retry automatically. Let the next CI failure (after the failed fix push) trigger a new attempt.
- **Parallel agent scheduling** — for now, fix agents can just grab a scheduler slot like any other run. Proper parallel scheduling (dedicated sidecar slots, priority) comes later.

## Done when

1. `POST /v0/hooks/github` accepts a GitHub `check_run` webhook, verifies signature, and logs the matched wave
2. A wave with an open PR that fails CI gets a fix agent spawned on an ephemeral worktree
3. The fix agent pushes to the PR branch and the ephemeral worktree is cleaned up
4. Concerto startup triggers a CI poll for all waves with open PRs
5. Startup janitor removes stale `*.ci-fix.*` and stale fork worktrees left by killed runs
6. CI-triggered auto-fix and manual `lf debug` both go through the same debug prompt path and remain functional
7. `cargo test` passes with webhook signature verification tests

## User quotes (intent anchors)

- "I dont think we need to persist this. in general though if we run the fix agent, hopefuly it submits a diff, at which point we should know we dont need to worry about the old sha, just the latest one"
- "Just one shot for now, were assuming that webhooks should work and want to know if it doesnt"
- "Log once?"
- "there's main and then there's a bunch of sidecars ... if we have full runnKind then were definitely going to want an .isMain() bool"
- "i think more info is probably better here, its not a lot of tokens"
- "yes, we want cleanup. we want to look at how forking works and try to make sure we can have one janitor that works with all our paralellism and run types"
- "I think we should just use the debug prompt btw, and make sure the debug prompt works still for both these automatic CI failures as well as more adhoc use cases - use the specifics here as examples not assumptions"
