# Service Integration

## Problem

lfd runs as a foreground process. Users must manually start it in a terminal, and it dies when they close it. For waves to work continuously (loop, watch, cron), lfd needs to run persistently — starting on login, restarting on crash, logging to the right place.

Users shouldn't think about lfd. It should install and disappear.

## Approach

Add service management subcommands to the `lfd` binary. Use `clap` derive (already a dependency) for proper CLI parsing, replacing the current manual arg parsing. Platform-specific modules generate and manage native service definitions (launchd plist on macOS, systemd unit on Linux).

No external crate for service management. The `service-manager` crate exists but adds an abstraction layer we don't need — our platform-specific code is ~100 lines each and we want full control over plist/unit contents for observability tuning.

### CLI Changes

Replace the manual `std::env::args()` parsing in `src/bin/lfd.rs` with clap:

```rust
#[derive(Parser)]
#[command(name = "lfd", about = "Loopflow daemon")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Run the daemon (foreground). Default when no subcommand given.
    Run,
    /// Install as a system service (launchd on macOS, systemd on Linux)
    Install,
    /// Uninstall the system service
    Uninstall,
    /// Start the installed service
    Start,
    /// Stop the installed service
    Stop,
    /// Show service status
    Status,
    /// Run database migrations (postgres only)
    Migrate {
        /// Show current schema version without migrating
        #[arg(long)]
        status: bool,
    },
}
```

`lfd` with no subcommand defaults to `Run` — backwards compatible with the current behavior and with the launchd/systemd service definitions that invoke `lfd run`.

### macOS launchd

Use modern `launchctl bootstrap`/`bootout` instead of deprecated `load`/`unload`.

```
~/Library/LaunchAgents/studio.loopflow.lfd.plist
~/Library/Logs/lfd/lfd.log
~/Library/Logs/lfd/lfd.err
```

Key plist decisions:
- **KeepAlive.SuccessfulExit = false**: Restart on crash, not on clean shutdown (`lfd stop` exits 0).
- **ThrottleInterval = 5**: 5-second minimum between restarts. launchd's built-in throttling handles further backoff if the process crashes repeatedly.
- **ProcessType = Adaptive**: Lets macOS dynamically adjust resource allocation based on activity. lfd is idle most of the time but needs responsiveness when a wave triggers.
- **RunAtLoad = true**: Start on login without explicit `lfd start`.

Bootstrap domain: `gui/$(id -u)` for user-level LaunchAgents.

### Linux systemd

```
~/.config/systemd/user/lfd.service
```

Key unit decisions:
- **Restart=on-failure, RestartSec=1s, RestartSteps=5, RestartMaxDelaySec=60s**: Exponential backoff (systemd v254+). Falls back gracefully on older systemd — fixed 1s restart if RestartSteps isn't supported.
- **WantedBy=default.target**: Standard for user services.
- Print a note about `loginctl enable-linger` so the service survives logout on headless machines.

### Platform dispatch

```rust
// src/lfd/service/mod.rs
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "linux")]
mod linux;

pub fn install() -> Result<()> { ... }
pub fn uninstall() -> Result<()> { ... }
pub fn start() -> Result<()> { ... }
pub fn stop() -> Result<()> { ... }
pub fn status() -> Result<()> { ... }
```

Unsupported platforms return a clear error.

### Path centralization

Add `src/lfd/paths.rs` to centralize platform-aware paths. The existing `default_db_path()` and `default_output_dir()` in `mod.rs` move here:

```rust
pub fn data_dir() -> PathBuf       // ~/.lf (macOS + Linux for now)
pub fn db_path() -> PathBuf        // data_dir()/lfd.db
pub fn output_dir() -> PathBuf     // data_dir()/output
pub fn log_dir() -> PathBuf        // ~/Library/Logs/lfd (macOS), cache_dir/lfd/logs (Linux)
pub fn plist_path() -> PathBuf     // macOS only
pub fn service_path() -> PathBuf   // Linux only
```

Keep `~/.lf/` as the data directory for now (matches existing behavior). The roadmap doc's `~/Library/Application Support/lf/` is aspirational but breaking existing installs isn't worth it yet.

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| `service-manager` crate | Abstracts platform details, less code | Hides plist/unit contents we need to control (ProcessType, ThrottleInterval, exponential backoff). Adds a dependency for ~200 lines of straightforward code. |
| Separate `lfd-service` binary | Clean separation | Unnecessary complexity. Service management is 5 subcommands on the same binary. Users expect `lfd install`, not a separate tool. |
| Keep manual arg parsing | No new dependencies | Already have clap. Manual parsing doesn't scale to 7 subcommands with flags. Clap gives `--help`, validation, and derive macros for free. |
| Use `launchctl load/unload` | Simpler, more documented | Deprecated since macOS 10.10. `bootstrap`/`bootout` is the sanctioned path. Homebrew services already migrated. |
| XPC-based on-demand launch | macOS-native, lower resource usage | Requires Objective-C/Swift interop, XPC protocol definition, and fundamental architecture changes. lfd needs to be always-on for trigger loops anyway. |

## Key decisions

**No daemonization in Rust.** lfd runs in the foreground. launchd/systemd handle backgrounding, restart, and log capture. This is the correct pattern — forking daemons are a relic. The roadmap's "Rust release" phase principles say "Rust-only distribution" and "HTTP-only protocol" — keeping the process model simple follows that spirit.

**`lfd` defaults to `run` with no subcommand.** Backwards compatible. The service definition calls `lfd run` explicitly, but a user typing just `lfd` gets the same behavior as today.

**No `lfd restart`.** Users can `lfd stop && lfd start`. A restart command adds complexity for a rare operation. If we need it later, it's one function.

**Print actionable output.** Every command prints what it did and what the user should do next if anything. `install` prints the path it wrote and says the service will start on next login. `status` shows PID, uptime, and port.

**Idempotent install.** Running `lfd install` when already installed overwrites the plist/unit. No error, no "already installed" — just updates to match the current binary path. This matters for Homebrew upgrades where the binary path changes.

## Scope

- In scope: `install`, `uninstall`, `start`, `stop`, `status` subcommands for macOS and Linux. Path centralization. Clap migration for lfd CLI. Tests for path functions and plist/unit generation.
- Out of scope: Windows support. Socket-based activation. Log rotation (launchd/systemd handle this). Homebrew formula changes (works without them — binary path is resolved at install time).

## Done when

```bash
# macOS
lfd install     # creates ~/Library/LaunchAgents/studio.loopflow.lfd.plist
lfd start       # service starts, `lfd status` shows running
lfd stop        # service stops cleanly (exit 0, no restart)
lfd status      # shows running/stopped, PID if running
lfd uninstall   # stops service, removes plist

# Linux
lfd install     # creates ~/.config/systemd/user/lfd.service, enables it
lfd start       # service starts
lfd stop        # service stops
lfd status      # shows systemctl status output
lfd uninstall   # disables, stops, removes unit file

# Both
lfd             # runs daemon in foreground (unchanged behavior)
lfd run         # same as above, explicit
lfd migrate     # unchanged behavior
cargo test -p loopflow service  # tests pass
cargo clippy -- -D warnings     # no warnings
```
