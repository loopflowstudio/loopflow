# Review: tmux plugin (phases 01–03)

## What was implemented

A TPM-installable tmux plugin delivering three user-facing capabilities:

1. **Status bar segment** (`scripts/loopflow-status.sh`) — displays `[lf: <branch>]` in lf mode or `[lf: N waves | name]` in container mode. Cached with 2s TTL, bounded to <250ms cold path.

2. **Named layouts** (`scripts/layouts/{lf-dev,lf-swarm}.sh`) — two window presets that create pane arrangements with mode-aware command seeding. Graceful fallback to 2-pane on terminals <120 cols.

3. **Mode-aware keybindings** (`scripts/keybindings.sh`) — 9 bindings via `prefix+l+<key>` using a custom tmux key table. All actions route through `loopflow_dispatch` in `helpers.sh`. Every action gives feedback — no silent no-ops.

Plugin entrypoint (`loopflow.tmux`) sets option defaults, registers status interpolation, and sources keybindings. Idempotent on re-source.

**Also on this branch:** `pr.rs` fix — PR refresh now compares `origin/<branch>` SHA before and after the commit/rebase cycle instead of tracking boolean flags. More reliable detection of actual changes.

## Key choices

| Decision | Choice | Why |
|----------|--------|-----|
| Custom key table | `switch-client -T loopflow` | Avoids polluting prefix table; all bindings namespaced cleanly |
| Status via `#()` | Shell script called by tmux | Simplest integration; avoids background daemons for status |
| Cache in `/tmp/` | JSON file with TTL | No dependency on jq; portable sed parsing; atomic write via mv |
| Mode detection | `loopflow_mode()` per-action | Short-circuit cache avoids lag; explicit override respected |
| Picker fallback | `tmux display-menu` when fzf missing | Zero-dep path always works |
| Help overlay | `display-popup` (3.2+), `display-message` fallback | Popup is better UX; fallback shows compact binding summary |
| fzf in run-shell | `display-popup` wrapping | fzf needs a TTY; `_loopflow_fzf_pick` detects context and routes through popup when no TTY available |
| PR change detection | `rev_parse` before/after | Boolean flag tracking (`committed || rebased`) missed edge cases; SHA comparison is definitive |

**Alternatives rejected:**
- Tmux format variables (`#{E:...}`) for status — harder to debug, less portable across tmux versions.
- Single monolithic script — split into helpers/status/keybindings/layouts for maintainability.
- Background daemon for status updates — overkill; `#()` with cache is sufficient.

## How it fits together

```
loopflow.tmux (entrypoint)
  ├── sets tmux option defaults
  ├── registers #(scripts/loopflow-status.sh) in status-right
  └── sources scripts/keybindings.sh
        └── binds prefix+l → loopflow key table
              └── each key → run-shell "source helpers.sh && loopflow_dispatch <action>"
                    ├── mode detection (auto/lf/container)
                    ├── command dispatch per mode
                    └── feedback via display-message
```

Layouts are standalone scripts invoked from dispatch or directly. Status script is standalone, called by tmux's status-interval.

## Risks and bottlenecks

- **`pgrep` for active step detection** — scanning process table on every status refresh (when cache misses). Bounded by TTL, but could be noisy on systems with many processes. Low risk in practice.
- **`awk` subprocess in mode detection** — spawned to convert ms → seconds for timeout. Could use pure bash but decimal division isn't trivial in bash. Acceptable for now.
- **`timeout` command availability** — macOS doesn't ship GNU `timeout` by default. The code falls back to running `lfq status` without a timeout when `timeout` is missing. Acceptable since the 2s cache TTL bounds repeat calls.
- **tmux version parsing** — `sed 's/[^0-9.]//g'` on `tmux -V` output. Works for `tmux 3.4` but may break on unusual version strings (e.g., `tmux next-3.5`). Minor risk.
- **Picker in `run-shell` context** — fzf needs a TTY. The `_loopflow_fzf_pick` function detects this and routes through `display-popup` when available, or returns rc=2 to signal the caller to use a tmux-native fallback. Container mode `run` and `stop` actions that invoke fzf route through this path.
- **Cache JSON with special chars** — `loopflow_cache_write` uses `printf %s` for the text field. Wave names or branch names with double quotes would break the JSON. Low risk since git branch names and wave names don't typically contain quotes.

## What's not included

- **Phase 04: Container lifecycle** (`lfd install/start/stop`, `lf up`) — deferred to follow-up branch per design doc.
- **Rust auth/token store changes** — originally on this branch, reverted to keep scope tmux-only. Will ship on auth branch.
- **`wave/auth/` docs** — removed from this branch for the same scope reason.
- **`@loopflow_status_format` customization** — the option is documented in wave docs but not implemented. Status format is hardcoded.
- **Automated tests beyond structural checks** — `tmux-review.py` verifies plugin loads and bindings exist, but doesn't exercise interactive flows.

## Gate fixes applied

1. **README.md**: Added missing `n` (next) and `d` (land) keybindings to the table.
2. **`loopflow_show_help`**: Rewrote to use temp file instead of embedding multiline text in double-quoted tmux command string. The original approach would break on shell interpolation of newlines.
3. **wave/tmux/README.md**: Marked `scripts/lfd` and `scripts/lf-up.sh` as phase 04 artifacts.
4. **Review doc**: Fixed stale `lf-flow` reference — layout count is two, not three.
5. **`tmux-review.py`**: Fixed skipped checklist item (numbering jumped from 5 to 7).
6. **`pr.rs`**: Applied `cargo fmt` to fix formatting.
7. **`tmux-review.py`**: Removed unnecessary f-string prefix.

## Test results

- Shell syntax: all 6 scripts pass `bash -n`
- Python syntax: `tmux-review.py` passes `ast.parse`
- Python tests: 67/67 pass
- Rust: `cargo fmt`, `cargo clippy`, `cargo test --all` all pass
- E2E smoke: passes
- All scripts executable
