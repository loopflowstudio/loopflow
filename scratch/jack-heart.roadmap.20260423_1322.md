---
requires: existing code
produces: code changes
---
Make the configured Asana team the canonical source of truth for the set of waves.

## Problem

Today, "what waves exist in this repo" = whatever `wave/<name>/<name>.yaml` exists in the current worktree. Two consequences bit us in yesterday's demo:

1. Different branches disagree about the wave set. On a reorg branch, Concerto sees the collapsed set; on main it sees the old set. Bootstrap creates phantom waves from whichever checkout happens to be open.
2. Stray directories (`wave/flows/` without a yaml, half-landed drafts) either get auto-bootstrapped as fake waves or require a filesystem-existence filter. Neither is stable.

The underlying bug: wave existence is a filesystem accident, not a shared fact.

## Decision

The **configured Asana team is canonical**. Every project in that team is a wave. Period. Filesystem `wave/<name>/` dirs are local editing surface, not existence proof.

Rationale:
- Matches the "Asana is source of truth for tasks within a wave" decision we already made. Extends it one level up.
- Cross-repo and cross-branch: open any worktree of a repo, see the same wave set.
- Team-scoped means a dev override is just "point at a different team in the same org" — no Asana Organization creation, no contamination.

The Asana hierarchy we depend on: `Workspace (is_organization: true) → Team → Project → Task`. Free Asana Workspaces don't have teams and need workspace-as-fence fallback, but that's deferred (commercial product, commercial Asana plan is the happy path).

## Current state (as of this branch)

A previous `lf code` run on this design doc only edited documentation; the code is unchanged. The docs in `README.md` and `docs/config.md` already describe the target state — `asana.team`, the `repos:` override block, canonical-from-Asana — but the code below doesn't match. The job is to make the code catch up to the docs (and the spec below).

**What's actually in the code right now:**

- `rust/loopflow/src/engine/config.rs:178` — `AsanaConfig` still has `pub default_team: Option<String>`. No `team` field, no serde alias. All Rust readers use `.default_team`.
- `rust/loopflow/src/engine/config.rs` (top-level `Config`) — no `repos:` override block in the deserialized type. Global/repo merge happens at the YAML-value layer (`merge_config_values`), which currently has no awareness of a `repos: { <path>: {...} }` shape.
- `rust/loopflow/src/lfd/pm/asana.rs:159` — `resolve_team_for_project_bootstrap` reads `self.config.default_team`. Not exposed publicly; only callable from bootstrap path.
- `rust/loopflow/src/ops/pm.rs:834` — `discover_waves(repo: &Path)` walks `wave/*/<name>.yaml` on disk. Returns `Vec<DiscoveredWave>` whose existence is filesystem-derived. Asana is never consulted for the wave set.
- `rust/loopflow/src/lfd/http/routes/waves.rs:188` — `list_discovered_waves_handler` calls `crate::ops::pm::discover_waves` and serves whatever the filesystem walk produced.
- `swift/LoopflowCore/State/RepoState.swift` — `bootstrapRoadmapWavesIfNeeded` and `roadmapWaveNames(in:)` still exist. They walk the local `wave/` dir and create wave-store records for any directory with a matching yaml. The "In Codebase" / `unmanagedDiscoveredWaves` branch added in this branch's earlier commits surfaces the filesystem set, not the Asana set.
- `scripts/verify_canonical_waves.py` — does not exist.

The error message in `rust/loopflow/src/lfd/pm/asana.rs:824` already mentions `asana.default_team` by name, so a partial rename will produce broken error text.

## Scope for this milestone

**In:**
- `asana.default_team` renamed to `asana.team` across Rust, Python, Swift, docs. It's the canonical fence — everyone reads it, it's not optional anymore. Add `#[serde(alias = "default_team")]` so existing configs keep parsing; alias removal is a future PR.
- `discover_waves()` replaced: reads projects from `asana.team` via `list_managed_projects(team_id)` instead of walking `wave/<name>/<name>.yaml` on disk. Filesystem walk becomes a fallback IF Asana is unreachable, fed from a cache at `.lf/cache/workspace/projects.json`.
- `GET /v0/waves/discovered` route returns Asana-backed list. Wire shape unchanged.
- Concerto's `RepoState.refreshDiscoveredWaves` consumes the new shape. (Originally scoped as "no Swift-side change needed — DTO stable"; the demo proved otherwise — see the correction in the RepoState section below for the actual Swift changes required.) `bootstrapRoadmapWavesIfNeeded` and `roadmapWaveNames(in:)` are deleted; users create new waves via Concerto's "New Wave" affordance, which creates the Asana project first.
- `~/.lf/config.yaml` supports per-repo overrides:
  ```yaml
  repos:
    /Users/jack/src/loopflow.roadmap:
      asana:
        team: <dev-team-gid>
  ```
  Override merges on top of `.lf/config.yaml`, affects both Concerto and CLI.
- Team autocreate: if `asana.team` is unset and the workspace is an org, find/create a "Loopflow" team and write the GID back to the config file we resolved from (`.lf/config.yaml`), unless the user override is active in which case we write to `~/.lf/config.yaml`.
- `scripts/verify_canonical_waves.py` — runnable walkthrough that proves the override and canonical paths.
- Tests: precedence + autocreate + override merge. Unit-level; mock the Asana API boundary.

**Out** (defer to follow-up milestones):
- Reading repo config from `origin/main` tree instead of current worktree. Branches can still disagree about `asana.team`; we accept that for now. Bigger git-plumbing change.
- Concerto UI for the override. Editing `~/.lf/config.yaml` by hand is acceptable for this milestone.
- Strict mirror sync (`lf op pm export --strict`, `pull --strict`). Existing per-wave pull/export stays soft.
- Free-tier Asana workspace fallback (no teams). Error out with clear message if `is_organization: false`.
- Local `wave/<name>/` dirs that aren't in Asana — "local draft" decoration. Render them as plain filesystem items for now; polish UI later.

## What changes, file by file

Each entry shows **current** (what's in the code now) and **target** (what to write). Don't skip a file just because the docs say it's done.

### Rust

**`rust/loopflow/src/engine/config.rs`**

Current (`AsanaConfig`, ~line 178):
```rust
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct AsanaConfig {
    #[serde(default)]
    pub workspace: Option<String>,
    #[serde(default)]
    pub default_team: Option<String>,
}
```

Target:
```rust
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct AsanaConfig {
    #[serde(default)]
    pub workspace: Option<String>,
    #[serde(default, alias = "default_team")]
    pub team: Option<String>,
}
```

Update every reader of `.default_team` → `.team` in this crate. The repo-grep that should return zero hits after the change: `rg "\.default_team\b" rust/`.

Add a top-level `repos: HashMap<String, PartialConfig>` (or equivalent) to `Config`, deserialized but flattened away by the merge layer before normal config consumers see it. The `repos: <abs-path>:` block in `~/.lf/config.yaml` is matched by exact path against the resolved repo root and merged on top of repo `.lf/config.yaml` before the global defaults. Add a unit test that asserts `repos:` precedence (`merge_config_values_repo_overrides_picks_matching_repo`).

**`rust/loopflow/src/lfd/pm/asana.rs`**

Current: `resolve_team_for_project_bootstrap` (private, around line 150) reads `self.config.default_team`, calls `find_or_create_team("Loopflow", workspace)` if unset.

Target:
- Rename to `resolve_team`. Make it `pub` on `AsanaClient`.
- Behavior: return `self.config.team` if set; else require workspace + `is_organization: true`; else find or create a `Loopflow` team in the workspace and return its GID.
- After autocreate, surface the new GID via the return value AND emit a side-channel (callback or a returned `WroteTeamGid { gid, target: ConfigSource }`) so the caller can persist it. Don't write to disk from inside `AsanaClient` — that's the caller's responsibility.

Update the error message in `rust/loopflow/src/lfd/pm/asana.rs:824` to say `asana.team` (not `asana.default_team`).

**`rust/loopflow/src/ops/pm.rs`**

Current (`discover_waves`, line 834): walks the filesystem.

Target signature stays `pub fn discover_waves(repo: &Path) -> OpsResult<Vec<DiscoveredWave>>` but the body becomes:

1. Resolve the repo's effective config (incorporating `~/.lf/config.yaml` `repos:` override).
2. If `asana.team` is unset and the workspace is an org → call `AsanaClient::resolve_team` (autocreate). On success, persist the new GID to whichever config file produced the resolution (repo `.lf/config.yaml` if that's where we got it, else `~/.lf/config.yaml`). On failure, return the error.
3. Call `AsanaClient::list_managed_projects(team_id)`. Map each project to a `DiscoveredWave { repo, wave_name: project.name, provider: Asana, project_id: Some(project.gid) }`.
4. Persist the result to `.lf/cache/workspace/projects.json` (atomic write).
5. On Asana failure: read the cache. Mark each `DiscoveredWave` with a `stale: true` flag (extend the struct). Return the cached list. If the cache is missing, return the error.

The filesystem walk that's there now does NOT remain as a fallback — drop it. The filesystem is the editing mirror, not the source of existence.

Tests:
- `discover_waves_returns_asana_team_projects` — mocked Asana returns 2 projects, `discover_waves` returns 2 entries with matching names + GIDs.
- `discover_waves_falls_back_to_cache_on_api_failure` — first call succeeds and writes cache; second call with mocked API failure reads the cache and returns `stale: true`.
- `discover_waves_autocreates_team_when_unset` — config has no `asana.team`, mocked workspace is org, mocked `find_or_create_team` returns "team-99", assert `discover_waves` succeeds AND the persisted config now has `team: team-99`.

**`rust/loopflow/src/lfd/http/routes/waves.rs`**

`list_discovered_waves_handler` (line 188) — no signature change. The body still calls `crate::ops::pm::discover_waves` so it inherits the new behavior. Add `stale: bool` to the JSON each entry emits, defaulting to `false` when fresh.

### Swift

**`swift/LoopflowCore/State/RepoState.swift`**

Delete `roadmapWaveNames(in:)` and `bootstrapRoadmapWavesIfNeeded`. The autobootstrap path goes away — discovered waves come from the lfd `/v0/waves/discovered` route, which is now Asana-backed.

~~`refreshDiscoveredWaves` and `unmanagedDiscoveredWaves` stay as-is. The DTO is stable.~~

**Correction (demo findings):** the DTO *is* stable, but the Swift consumption was not. Required Swift-side changes:
- `RepoState.refreshDiscoveredWaves` scoped discovered waves with raw `URL.path()` vs the lfd-canonicalized DTO path → every wave filtered out. Now compares `normalizedFilePath` both sides.
- `URL/String.normalizedFilePath` did not strip trailing slashes (`/repo/` ≠ `/repo`); hardened the shared helper so all repo-path comparisons agree.
- Connect-time `addRepo` can fail silently on a still-starting bundled daemon; the discovered handler iterates *registered* repos, so the UI showed "No waves yet" until something else registered the repo (~minutes). `refreshDiscoveredWaves` now re-asserts the idempotent registration before listing.
- Sidebar split In Flight / Ready into two sections and `ordered` was `active + idle`, so a freshly-started wave hopped sections and reordered. Collapsed into one stable-order section.
- Portfolio cards counted managed waves only and were blind to Asana-discovered waves; `PortfolioRepoState` now fetches discovered and reflects them in the count/empty-state/list.

**`swift/Concerto/Platform/macOS/Views/WaveSidebar.swift`**

Replace any "Connecting roadmap waves…" copy with "Loading waves from Asana…" tied to the discovered-waves fetch in-flight state.

### Docs

The docs were updated by the prior failed run and are already in their target state. Verify they match the implementation after the code changes land. Specifically:
- `README.md` already shows `team:` and the `repos:` override block. Confirm no stragglers.
- `docs/config.md` already describes the override. Confirm.
- `docs/wave-authoring.md` — verify references match.

Repo-grep that should return zero hits after this milestone: `rg "default_team" -- 'rust/' 'swift/' 'python/' 'README.md' 'docs/'`. (The serde alias keeps the string in the source for one struct; that hit is allowed, document it inline.)

## Done when

1. `rg "\.default_team\b" rust/ swift/ python/` returns no hits except the `#[serde(alias = "default_team")]` line.
2. `cargo test --all` passes, including these new tests in `rust/loopflow/src/ops/pm.rs`:
   - `discover_waves_returns_asana_team_projects`
   - `discover_waves_falls_back_to_cache_on_api_failure`
   - `discover_waves_autocreates_team_when_unset`
   - And in `rust/loopflow/src/engine/config.rs`:
   - `merge_config_values_repo_overrides_picks_matching_repo`
3. `swift test --package-path swift` passes. `bootstrapRoadmapWavesIfNeeded` no longer exists.
4. `scripts/verify_canonical_waves.py` runs end-to-end (see below) and exits 0.
5. Manual: launch Concerto on this repo (`uv run python scripts/concerto-dev.py run-debug`). Delete `wave/root/` locally → "root" still appears in the sidebar (Asana-backed). Add a bogus `wave/fake/fake.yaml` locally → "fake" does NOT appear (not in Asana). Set a per-repo override in `~/.lf/config.yaml` pointing at a different team → sidebar reflects the override team.

## Verification script

Create `scripts/verify_canonical_waves.py`. Single command, end-to-end. Treat the script as the executable spec:

```python
# scripts/verify_canonical_waves.py
#
# Usage: uv run python scripts/verify_canonical_waves.py
#
# Verifies the configured Asana team is canonical for the wave set.
# Requires: lfd running, valid Asana auth, repo at the script's CWD.
```

Steps the script must perform, in order:
1. Snapshot current `~/.lf/config.yaml` and `.lf/config.yaml`. Restore on exit (atexit / try-finally).
2. Read the repo's current `asana.team` GID (call it `repo_team`). Assert it's set; if not, exit with an instructive error.
3. Find or create a fresh test team in the same workspace (e.g. `loopflow-verify-<timestamp>`). Capture its GID (`override_team`).
4. Write `~/.lf/config.yaml` with a `repos:` block pointing this repo's `asana.team` at `override_team`.
5. Call `lf op pm list` (capture stdout). Assert the output reflects projects in `override_team`, not `repo_team`. If `override_team` is empty, the assertion is "no waves listed."
6. Remove the override from `~/.lf/config.yaml`.
7. Call `lf op pm list` again. Assert the output reflects projects in `repo_team`.
8. Delete the test team (cleanup).
9. Restore snapshots.

Fail loudly with a numbered step on any assertion failure.

Add an entry for the script in `TESTING.md` under "Validation Scripts".

## Risk and rollback

- **Worst case**: autocreate misfires and writes the wrong GID into `.lf/config.yaml`. Mitigation: never write to repo config when the user override is active; in that case write to `~/.lf/config.yaml`. When writing to repo config, leave it as an unstaged change so Jack can review.
- **Asana outage**: fallback to cached project list with `stale: true`, same pattern as existing `/waves/{id}/roadmap` stale-cache fallback (`rust/loopflow/src/lfd/http/routes/waves.rs:1599`).
- **Rollback**: revert the branch. The pre-change filesystem walk is preserved in git history; restoring the old `discover_waves` body is one commit.

## Why this scope and not more

The "read repo config from origin/main" change is the bigger structural shift — it requires git plumbing in the config loader and has implications for offline behavior, fetch freshness, and fresh-clone UX. Landing the Asana-canonical part first gives us the observable win (wave set matches Asana reality, no filesystem accidents) without the git resolution work. Branches disagreeing about `asana.team` is rare in practice (you'd only hit it if someone intentionally edited the config on a branch), and the per-repo override covers the dev-workflow case that matters most.
