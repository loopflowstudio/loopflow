# Container Mode: tmux as the front door

## Problem

The tmux plugin shipped with status, layouts, and keybindings (phases 01–03). It auto-detects container mode and dispatches commands differently based on mode. But the integration is shallow: `next` and `land` show "not yet implemented" in container mode, there's no way to bootstrap a container environment from tmux, and the status bar doesn't distinguish "daemon healthy" from "daemon missing."

Users who want container-mode orchestration (multi-wave, streaming logs, persistent daemon) currently have to manage `lfd install && lfd start` out-of-band, then hope the tmux plugin detects it. There's no guided path from "I have tmux" to "I have a running loopflow environment."

Additionally, `lfq land` enables auto-merge but never verifies GitHub accepted it. CI failures go undetected unless webhooks are configured. The loop can silently stall.

## Approach

Four deliverables, each independently useful:

### 1. `scripts/lf-up.sh` — one-command bootstrap

A shell script (callable from keybinding) that gets you from zero to working:

```bash
scripts/lf-up.sh [--detach]
```

Behavior:
1. Check if `lfd` binary exists. If not: print install instructions, exit 1.
2. Check for Docker/Podman. If neither found: print install link, exit 1.
3. Check `lfd status`. If not running: run `lfd start`, wait for health.
4. Health gate: poll `lfq status` with 250ms intervals, 15s timeout. Show progress via `tmux display-message` or stdout.
5. If `--detach`: exit 0 (container running, no layout).
6. Otherwise: check if `lf-dev` window already exists — if yes, focus it. If no, open `lf-dev` layout.

First-run experience for a user with Docker installed:
```
$ lf up
loopflow: starting lfd...
loopflow: waiting for health... (3s)
loopflow: ready — opening lf-dev layout
```

First-run for a user without Docker:
```
$ lf up
loopflow: docker not found — install Docker Desktop: https://docker.com/get-started
```

The script works **outside tmux** too (skips layout, just starts daemon). This means `lf up` can be the first command in a user's shell profile or in CI.

### 2. Complete container mode dispatch

Wire up the remaining actions in `loopflow_dispatch()`:

| Action | Current (container) | New behavior |
|--------|-------------------|--------------|
| `next` | "not yet implemented" | `lfq land <wave>` via picker (waves loop automatically) |
| `land` | "not yet implemented" | `lfq land <wave>` via picker |
| `logs` | works | no change |
| `run` | works | no change |
| `stop` | works | no change |

Both `next` and `land` dispatch to `lfq land` in container mode. For waves, landing *is* next — `lfd` reads wave state and decides whether to start another iteration. The `lf ops next` distinction (separate land + create stacked branch) only applies to worktree mode where the human drives the loop.

Also add a new action:

| Action | Key | Behavior |
|--------|-----|----------|
| `up` | `u` | Run `scripts/lf-up.sh` |

### 3. Harden `lfq land` loop reliability

The land→merge→next chain has three gaps:

**a) Auto-merge verification.** `enable_auto_merge()` in `land.rs:237` runs `gh pr merge --squash --auto` but never checks if GitHub accepted it. If the repo doesn't have auto-merge enabled, or the PR isn't eligible (draft, failing required checks, merge conflicts), this silently fails and the PR sits forever.

Fix: after `enable_auto_merge()`, query `gh pr view --json autoMergeRequest -q '.autoMergeRequest'`. If null, the auto-merge wasn't accepted — return an error with the likely cause. This is ~15 lines in `land.rs:finalize_remote()`.

**b) CI failure polling.** `poll_all_waves_ci()` exists in `hooks.rs:435` but nothing calls it periodically. The only CI failure detection path is GitHub webhooks, which require a webhook secret to be configured (returns 503 without it). Many local dev setups won't have webhooks.

Fix: add `spawn_ci_poller` to the scheduler's background triggers, alongside the existing `spawn_queue_reconciler`. Poll every 60s for active waves. ~30 lines in a new `triggers/ci_poll.rs`, plus one line in `scheduler.rs:spawn_triggers()`.

**c) Surface CI failure state.** When CI fails, an event is emitted to the event hub, but it's not surfaced in wave status. `lfq show <wave>` and the tmux status bar can't distinguish "waiting for CI" from "CI failed, loop stalled."

Fix: when a CI failure event is emitted, update the wave run's status or add a `ci_status` field that `lfq show` and the status script can read. This feeds into deliverable 4 (status bar states).

### 4. Health-aware status bar

Enhance `loopflow-status.sh` to show daemon and wave health state:

| State | Output | Trigger |
|-------|--------|---------|
| daemon healthy, waves active | `[lf: 3 waves ▸ engbot]` | `lfq list` returns waves |
| wave landed, CI pending | `[lf: engbot ⏳ CI]` | wave has pending auto-merge |
| wave CI failed | `[lf: engbot ✗ CI]` | CI failure recorded |
| daemon healthy, no waves | `[lf: idle]` | `lfq list` returns empty |
| daemon starting | `[lf: starting…]` | `lfd status` returns starting/process exists but health fails |
| daemon unreachable | `[lf: ⚠ offline]` | `lfq status` fails and `lfd` binary exists |
| no lfd binary | `[lf: main]` | lf mode, as today |

Cache all states — the status script is called on every render tick.

The CI states (`⏳ CI`, `✗ CI`) require `lfq list --json` to include per-wave CI status from the store. This is the only `lfq` API change needed — extending the list response with a `ci_status` field.

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Build `lf up` into the Rust `lf` binary | Single binary distribution, no shell fragility | `lf up` opens a tmux layout — it's fundamentally a tmux concern. The Rust binary shouldn't know about tmux panes. Keep workspace setup in the tmux plugin. |
| Auto-start daemon on plugin source | Zero-action UX | Violates "plugin load must not auto-start a container" from wave README. Side effects on `source` are hostile. |
| Separate `lfd-tmux` wrapper binary | Clean separation | Over-engineered. Shell scripts are the right tool for glue between tmux and CLI binaries. The plugin is already pure shell. |
| Skip health gate, trust `lfd start` | Simpler | `lfd start` returns before health check passes. Opening a layout before the daemon is ready means broken first experience. The 15s gate is worth it. |
| Add `lfq next` as separate subcommand | Explicit user intent | For waves, the daemon owns the loop. Landing *is* advancing. A separate `next` command implies the user drives iteration — that's the worktree model, not the wave model. |

## Key decisions

**`lf up` is a tmux plugin script, not an `lf` subcommand.** The `lf` binary runs prompts and flows. `lf up` orchestrates tmux layouts and daemon lifecycle — that's tmux-plugin territory. If users want it on their PATH, they can symlink or alias.

**Health gate uses polling, not blocking.** `lfd start` on launchd/systemd returns immediately. The plugin polls `lfq status` at 250ms intervals (matching `@loopflow_status_timeout_ms`) for up to 15s. This gives visible progress rather than a frozen terminal.

**`prefix+l+u` keybinding for `up`.** One keypress to ensure the daemon is running and open a layout. Idempotent: if already running, focuses existing `lf-dev` window.

**Status bar degrades, never errors.** Unknown states render as `[lf]`. Parse failures fall back to cached text. Timeouts return stale cache with `~` marker. No blank status bar, ever.

**Container mode `next` = `land`.** Both dispatch to `lfq land` via picker. The daemon decides whether to loop based on wave state. No `lfq next` subcommand needed.

**Auto-merge must be verified, not assumed.** `gh pr merge --auto` can silently fail. Checking `autoMergeRequest` after enabling catches repo misconfiguration, draft PRs, and merge conflicts immediately rather than leaving the loop stalled.

**CI polling as fallback to webhooks.** Webhooks are the fast path. But local dev setups often skip webhook configuration. A 60s polling loop for active waves ensures CI failures are detected regardless.

## Scope

### In scope

- `scripts/lf-up.sh` with daemon bootstrap, Docker check, health gate, layout idempotency
- Container mode dispatch for `next` and `land` (both → `lfq land <wave>`)
- New `up` action and `prefix+l+u` keybinding
- Health-aware status bar (healthy/CI pending/CI failed/starting/offline states)
- Auto-merge verification in `land.rs:finalize_remote()`
- CI polling trigger in `lfd` scheduler (`spawn_ci_poller`, 60s interval)
- CI status surfaced in `lfq list --json` response
- Graceful error messages for missing Docker, missing `lfd`, missing `lfq`
- Update `tmux-review.py` with structural tests for new scripts and keybindings
- Update help overlay with `u` binding

### Out of scope

- `lfd update` command (daemon concern, not tmux plugin)
- Auto-discovery of repos under `repos_root` (daemon concern)
- `lfd uninstall` from tmux (destructive, belongs in CLI)
- Container naming configuration (daemon config, not tmux)
- New layouts beyond `lf-dev` and `lf-swarm`
- `lfq` repo scoping changes (already supported at API level, works as-is)

## Implementation plan

### 1. `scripts/lf-up.sh` (~80 lines)

```bash
#!/usr/bin/env bash
# lf-up.sh — bootstrap loopflow: ensure daemon running, open layout

source "$(dirname "$0")/helpers.sh"

main() {
    local detach=false
    [[ "$1" == "--detach" ]] && detach=true

    # Step 1: Check for lfd
    if ! command -v lfd &>/dev/null; then
        echo "loopflow: lfd not found — install: curl -fsSL https://... | sh"
        return 1
    fi

    # Step 2: Check for runtime (docker or podman)
    if ! command -v docker &>/dev/null && ! command -v podman &>/dev/null; then
        echo "loopflow: docker not found — install Docker Desktop: https://docker.com/get-started"
        return 1
    fi

    # Step 3: Start daemon if not running
    if ! lfq status &>/dev/null; then
        _lf_up_msg "starting lfd..."
        lfd start
    fi

    # Step 4: Health gate
    _lf_up_health_gate || return 1

    # Step 5: Open layout (unless --detach)
    if [[ "$detach" == true ]]; then
        _lf_up_msg "ready"
        return 0
    fi

    if _lf_up_in_tmux; then
        # Idempotent: focus existing lf-dev window or create new one
        if tmux list-windows -F '#{window_name}' | grep -q '^lf-dev$'; then
            tmux select-window -t lf-dev
        else
            "$LOOPFLOW_DIR/scripts/layouts/lf-dev.sh"
        fi
    else
        _lf_up_msg "ready (not in tmux — run inside tmux for layout)"
    fi
}

_lf_up_health_gate() {
    local attempts=0 max_attempts=60  # 60 × 250ms = 15s
    while (( attempts < max_attempts )); do
        if lfq status >/dev/null 2>&1; then
            _lf_up_msg "ready"
            return 0
        fi
        (( attempts++ ))
        sleep 0.25
    done
    _lf_up_msg "timeout waiting for lfd health — check: lfd status"
    return 1
}

_lf_up_msg() {
    if _lf_up_in_tmux; then
        tmux display-message "loopflow: $1"
    fi
    echo "loopflow: $1"
}

_lf_up_in_tmux() {
    [[ -n "${TMUX:-}" ]]
}

main "$@"
```

### 2. Dispatch updates in `helpers.sh` (~20 lines changed)

Add `up` case. Update `next` and `land` for container mode:

```bash
next)
    if [[ "$mode" == "container" ]]; then
        local wave
        wave="$(loopflow_pick_wave)" || return 1
        tmux send-keys "lfq land '$wave'" Enter
    else
        # ... existing lf mode code
    fi
    ;;
land)
    if [[ "$mode" == "container" ]]; then
        local wave
        wave="$(loopflow_pick_wave)" || return 1
        tmux send-keys "lfq land '$wave'" Enter
    else
        # ... existing lf mode code
    fi
    ;;
up)
    if _lf_up_in_tmux; then
        tmux send-keys "'$LOOPFLOW_DIR/scripts/lf-up.sh'" Enter
    else
        tmux display-message "up: only available inside tmux"
    fi
    ;;
```

### 3. Keybinding registration (~2 lines)

Add to `scripts/keybindings.sh`:
```bash
bind_loopflow_key u "up"
```

### 4. Auto-merge verification in `land.rs` (~20 lines)

After `enable_auto_merge()` in `finalize_remote()`, verify it was accepted:

```rust
// In finalize_remote(), after enable_auto_merge():
enable_auto_merge(repo_root, &message.title, &message.body)?;

// Verify auto-merge was accepted
if !verify_auto_merge_enabled(repo_root)? {
    return Err(OpsError::Message(
        "auto-merge was not accepted by GitHub — check that auto-merge is enabled \
         for this repo and the PR is not a draft".to_string()
    ));
}
```

```rust
fn verify_auto_merge_enabled(repo: &Path) -> OpsResult<bool> {
    let mut cmd = Command::new("gh");
    cmd.arg("pr")
        .arg("view")
        .arg("--json")
        .arg("autoMergeRequest")
        .arg("-q")
        .arg(".autoMergeRequest")
        .current_dir(repo);
    match run_command(&mut cmd) {
        Ok(output) => {
            let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
            Ok(!text.is_empty() && text != "null")
        }
        Err(_) => Ok(false),
    }
}
```

### 5. CI polling trigger (~40 lines)

New file `rust/loopflow/src/lfd/triggers/ci_poll.rs`:

```rust
pub fn spawn_ci_poller(
    store: SharedStore,
    event_hub: EventHub,
    github: GitHubConfig,
    cache: Arc<Mutex<HashSet<String>>>,
    cancel: CancellationToken,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(60));
        loop {
            tokio::select! {
                _ = cancel.cancelled() => {
                    tracing::info!("ci_poller shutting down");
                    break;
                }
                _ = interval.tick() => {
                    let token = match github.token() {
                        Some(t) => t,
                        None => continue, // no token, skip
                    };
                    match poll_all_waves_ci(&store, &event_hub, &token, &cache).await {
                        Ok(n) if n > 0 => tracing::info!(emitted = n, "ci_poller found failures"),
                        Err(err) => tracing::warn!(error = %err, "ci_poller failed"),
                        _ => {}
                    }
                }
            }
        }
    })
}
```

Add to `scheduler.rs:spawn_triggers()`:
```rust
triggers::spawn_ci_poller(store.clone(), event_hub.clone(), github.clone(), ci_cache.clone(), cancel.clone()),
```

### 6. Status bar health awareness (~20 lines changed in `loopflow-status.sh`)

In the container mode branch, distinguish healthy/CI/starting/offline:
```bash
generate_container_status() {
    if ! lfq status >/dev/null 2>&1; then
        if lfd status 2>/dev/null | grep -q 'starting\|running'; then
            echo "[lf: starting…]"
        else
            echo "[lf: ⚠ offline]"
        fi
        return
    fi

    local json
    json="$(lfq list --json 2>/dev/null)" || { echo "[lf: idle]"; return; }

    # Check for CI failures first
    local ci_failed
    ci_failed="$(echo "$json" | grep -c '"ci_status":"failed"')" || true
    if [[ "$ci_failed" -gt 0 ]]; then
        local failed_wave
        failed_wave="$(echo "$json" | grep '"ci_status":"failed"' | head -1 | sed 's/.*"name":"\([^"]*\)".*/\1/')"
        echo "[lf: $failed_wave ✗ CI]"
        return
    fi

    # Check for CI pending (auto-merge waiting)
    local ci_pending
    ci_pending="$(echo "$json" | grep -c '"ci_status":"pending"')" || true
    if [[ "$ci_pending" -gt 0 ]]; then
        local pending_wave
        pending_wave="$(echo "$json" | grep '"ci_status":"pending"' | head -1 | sed 's/.*"name":"\([^"]*\)".*/\1/')"
        echo "[lf: $pending_wave ⏳ CI]"
        return
    fi

    # Normal: show wave count and active wave
    # ... existing logic
}
```

### 7. Help overlay update (~1 line)

Add `prefix+$prefix+u  start/bootstrap` to the help text.

### 8. Test updates in `tmux-review.py` (~20 lines)

- Verify `lf-up.sh` exists and is executable
- Verify `u` keybinding registered in loopflow table (now expect 10 bindings)
- Verify help overlay contains `u` binding

## Done when

- `scripts/lf-up.sh` bootstraps daemon and opens layout: `./scripts/lf-up.sh` from a tmux session with Docker running starts `lfd`, waits for health, opens `lf-dev`
- `prefix+l+u` triggers bootstrap from a keybinding
- `prefix+l+n` and `prefix+l+d` work in container mode (pick wave, run `lfq land`)
- `lfq land` verifies auto-merge was accepted — returns error if GitHub rejected it
- CI failures are detected within 60s even without webhooks configured
- Status bar shows CI state (`⏳ CI`, `✗ CI`), `starting…` during daemon startup, and `⚠ offline` when daemon is down
- Help overlay shows all 10 keybindings (was 9)
- `tmux-review.py` passes with new structural checks
- All existing tests still pass

Wave success criteria advanced:
- "all keybindings operate (or fail-soft) in both modes" — `next` and `land` now work in container mode
- "`lf up` gives a usable workspace on first run" — bootstrap script with health gate delivers this
- "install + first successful action in <2 minutes" — `lf up` with warm Docker: ~5s
