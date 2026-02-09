# Service Integration Review

## What was implemented

Service management subcommands for `lfd`: `install`, `uninstall`, `start`, `stop`, `status`. macOS uses launchd (bootstrap/bootout), Linux uses systemd user units. Clap replaces manual arg parsing. Path functions centralized in `paths.rs`.

## Key choices

| Decision | Why |
|----------|-----|
| No external crate | ~200 lines of platform code, full control over plist/unit contents |
| `dispatch!` macro for platform dispatch | Eliminates boilerplate across 5 identical function signatures |
| `lfd` defaults to `Run` | Backwards compatible with existing usage and service definitions |
| Modern `launchctl bootstrap/bootout` | `load/unload` deprecated since macOS 10.10 |
| `KeepAlive.SuccessfulExit = false` | Restart on crash, not on clean `lfd stop` (exit 0) |
| systemd `RestartSteps=5` + `RestartMaxDelaySec=60s` | Exponential backoff on v254+, graceful fallback on older |
| Idempotent install | Overwrites existing plist/unit, handles upgrades cleanly |

## How it fits together

```
lfd.rs (clap CLI) -> service/mod.rs (dispatch! macro) -> macos.rs | linux.rs
                                                              |
paths.rs (centralized platform paths) <-----------------------+
```

`paths.rs` owns all platform-aware paths (`data_dir`, `db_path`, `log_dir`, `plist_path`, `service_path`). Service modules use these for plist/unit generation and file operations. The daemon's `run_daemon()` function uses `paths::db_path()` and `paths::output_dir()` directly.

## Risks and bottlenecks

- **`current_exe()` for binary path**: Works for direct installs and Homebrew. Could break in exotic setups (symlink farms, NixOS). Acceptable for now.
- **`id -u` subprocess in `bootstrap_domain()`**: Spawns a process on every macOS service command. Negligible cost since these are one-shot operations.
- **systemd v254+ features**: `RestartSteps` and `RestartMaxDelaySec` silently ignored on older systemd. Fixed 1s restart interval as fallback is fine.

## What's not included

- Windows support (out of scope per design doc)
- Socket-based activation (unnecessary — lfd needs to be always-on for trigger loops)
- Log rotation (launchd/systemd handle this natively)
- `lfd restart` command (use `lfd stop && lfd start`)
- Integration tests for actual service install/start (requires root or service manager)
