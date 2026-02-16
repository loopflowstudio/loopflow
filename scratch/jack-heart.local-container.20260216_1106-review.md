# Review: Container lfd as local option

## What was implemented

Added `mode` field to `lfd.yaml` that selects a strict operational profile (`native` or `container`). `lfd install/start/stop/status` now dispatch based on `(service_manager, runtime_backend)` resolved from the mode, supporting both native binary and Docker Compose backends behind a single CLI.

Key additions:
- **Config resolution**: `RawLfdConfig` → `LfdConfig` via `ModeProfile`, enforcing that profile-owned fields (`storage`, `executor.type`, `runtime_backend`, `service_manager`) cannot be overridden in YAML or env vars
- **Compose service backend** (`compose.rs`): generates `~/.lf/docker-compose.yml` from config, manages Docker lifecycle, parses `docker compose ps --format json` for status
- **Dual-run protection**: `install` and `start` detect conflicting backends via plist/unit file inspection, fail with actionable errors unless `--force`
- **Status contract**: structured output (`manager: launchd (running)`, `backend: compose`, per-service health)
- **Store abstraction** (`StorageConfig`, `open_store`, `migrate_store`): unified store creation/migration path used by both `lfd serve` and `lfd migrate`
- **Worktree pruning**: branches with no commits beyond merge target are now marked as `fresh/prunable`

## Key choices

1. **Strict profiles over soft defaults.** Mode is not a defaults layer — profile-owned fields are locked. This prevents "almost container mode" misconfigurations. Env vars like `LFD_STORAGE` are hard-rejected when set.

2. **Plist/unit file inspection for backend detection.** Rather than persisting mode state in a separate file, the installed service file itself is the source of truth. Compose backend is detected by `docker compose` strings in the unit content. Simple and requires no new state files.

3. **Service manager owns lifecycle in both modes.** Compose mode doesn't use `docker compose up -d`. Instead, launchd/systemd runs `docker compose up` (foreground), getting crash recovery, boot persistence, and log management for free.

4. **Compose file is always regenerated.** No merge/diff of existing files. User customization goes in `docker-compose.override.yml`.

## How it fits together

```
lfd.yaml (mode: native|container)
    → RawLfdConfig.resolve() → LfdConfig (with locked profile fields)
        → service::dispatch() matches on (ServiceManager, RuntimeBackend)
            → macos/linux module handles install/start/stop/status
                → compose module handles Docker lifecycle when RuntimeBackend::Compose
```

The binary entry point (`lfd.rs`) loads config once, then either dispatches to service management (install/start/stop/status/migrate) or runs the HTTP server. The `StorageConfig` enum bridges config → store creation.

## Risks and bottlenecks

- **Docker socket availability at boot.** launchd's `ThrottleInterval: 10` handles retries when Docker isn't ready yet, but there's no explicit wait-for-docker logic. If Docker takes >30s to start, lfd may exhaust retries.
- **Compose file format coupling.** The rendered compose file uses string interpolation, not a YAML library. If the template grows more complex, this becomes fragile. Current scope is fine.
- **Backend detection heuristics.** Checking for `<string>docker</string>` + `<string>compose</string>` in plist XML is correct but brittle if the plist format changes. The unit tests cover this, but it's worth noting.

## What's not included

- **Port conflict probing.** The design doc mentions probing port 2486 before start. This PR relies on backend conflict detection (plist/unit inspection) but doesn't probe the port directly.
- **`lfd uninstall` does not remove postgres data volumes.** This is intentional per the design doc — explicit `docker volume rm` is a separate action.
- **No `docker-compose.override.yml` documentation.** The override file is supported in `compose_files()` but not documented yet.
- **Async store traits** (`WaveStateStore`, `ExecutionStore`, `StoreAdmin`). These are defined on `Store` but the HTTP layer still uses the sync `RunStore` trait via `SharedStore`. Migration to async traits is a separate effort.
