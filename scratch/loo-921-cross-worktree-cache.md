# LOO-921: cross-worktree Rust cache on the normal path, and the real edit-to-test loop

## Problem

LOO-650 (#1951, #1952, #1954) split the crates so unchanged ones can be reused
across worktrees, and proved it with a private cache. The Machine never got it:
plain `cargo` in a new worktree still compiles every workspace crate. And the
only numbers are `cargo build --bin lf` at one job; nobody has timed a new
worktree through a passing focused test.

Task brief, kept: install a pinned cache binary at a stable Machine path, select
it in personal Cargo configuration, keep Machine-only settings out of CI, keep
other personal settings; verify the daemon executable, inventory consumption and
a new-root hit after natural idle; no per-compile daemon restart. Then measure
new-worktree-to-passing-focused-test for app and execution edits plus repeated
edits, compare incremental off with an opt-in repeated-edit mode, and pick
defaults from what was measured.

Direction comment (Session 15a1f7f6, requester Jack Heart, 2026-11-16): "Respect
the repository one-Cargo-command-at-a-time rule; report concurrent-worktree
capacity as unmeasured unless a separate permitted experiment exists. Do not run
simultaneous Cargo commands for the concurrency sketch in the brief." Sequence:
LOO-921 → LOO-922 (fresh-worktree cost) → LOO-923 (execution siblings); after
this Task is verified and finished, `lf task checkout LOO-922` if it has no
checkout, then `lf ship LOO-922` only if `lf ps --json` shows no live Graph for
it. Results travel in LOO-922's own brief/scratch.

## Demo

```bash
uv run python scripts/build_cache.py install   # once per Machine
git worktree add ../loopflow.demo -b demo && cd ../loopflow.demo
uv run python scripts/build_cache.py status
# wrapper  ~/.local/share/loopflow/sccache/ca81ec2a…/bin/sccache
# daemon   pid 4242, same executable, up 3m
# roots    61 loaded; this checkout loaded
# Rust     hits 258, misses 1
cargo test -p loopflow-execution --lib skills::catalog::   # workspace libraries arrive from the cache
```

Before the daemon has restarted, `status` says so instead: `this checkout loads
at the daemon's next idle exit`, exit 3.

The measured half is `scripts/benchmarks/crate-rebuild/results/loo-921.html`: a
table of new-worktree-to-passing-test and repeated-edit medians per mode.

## What the Machine has now (read 2026-11-16, before any change)

- `~/.cargo/config.toml` is exactly `[build] rustc-wrapper = "/opt/homebrew/bin/sccache"`, `jobs = 2`. No `[env]`.
- Running daemon: `/opt/homebrew/bin/sccache`, started 09:22:11, `Base directories (none)`, Rust 94 hits / 342 misses (21.6%) for its lifetime. It also serves other repositories (two `managym` compiles in flight).
- `~/.cache/loopflow/sccache.json` holds 57 roots and nothing reads it: only `scripts/dev-lf` passes `SCCACHE_CONF`, and the stock binary has no Rust basedir support.
- No sccache config file at the default location; no `SCCACHE_*` in the shell.
- `rustc 1.98.1 (Homebrew)`, 16 CPUs, 128 GiB, load average 48/39/38, 60 worktrees.
- `core.hooksPath` is `/Users/jack/src/loopflow/.git/hooks`; `post-checkout`, `post-commit`, `post-merge`, `post-rewrite` there are February `lfd` notifiers that write to a socket that no longer exists.
- `/tmp/loo650-sccache` is an unpacked tarball (no `.git`), debug build, sha256 `9d864b14…`. It lives in `/tmp` and will not survive.

## Risk findings

**The two binaries print the same version.** Stock and pinned both answer
`sccache 0.19.0`. Identity is the executable path (which carries the revision)
and the running daemon's path, never `--version`.

**The pinned revision is fetchable.** `mozilla/sccache@ca81ec2a2566efc623692741440d99c710268841`,
committed 2026-11-14, "rust: hash listed env-deps verbatim with basedirs". The
latest release is still v0.19.0 (2026-10-24), so Homebrew cannot supply it.
`default = ["all"]` pulls every cloud backend; local disk needs none.

**A pinned client cannot share the stock daemon.** Client-side mode speaks
`StorageHandshake`/`StorageGetRaw`/`StoragePutRaw`, which 0.19.0 lacks. Both
default to TCP 4226, so after the config flips every new compile would reach the
old daemon, and each attempt keeps it from idling out. The pinned daemon gets
its own socket (`SCCACHE_SERVER_UDS`); the stock one drains and exits by itself.

**Where each setting is read** (`src/config.rs`, `src/compiler/rust.rs`,
`src/server.rs` at the pinned revision):

| Setting | Read by | When |
|---|---|---|
| `basedirs` (`SCCACHE_CONF` file) | daemon | once, at start |
| `SCCACHE_MAX_FRAME_LENGTH` | daemon (and client) | at start |
| `SCCACHE_BASEDIRS_VERBATIM_ENV_DEPS` | each compile's client env | every compile |
| `SCCACHE_CLIENT_SIDE`, `SCCACHE_SERVER_UDS`, `SCCACHE_IGNORE_SERVER_IO_ERROR` | client | every compile |

Cargo's `[env]` table reaches rustc, build scripts and the wrapper, and does not
override a variable the caller already set. So personal `[env]` covers plain
`cargo`, and an explicit `SCCACHE_CONF`/`SCCACHE_DIR` still wins.

**Nothing registers a new worktree today.** Roots are written only by
`scripts/dev-lf`. `lf` creates Task checkouts with `git worktree add`
(`rust/loopflow-execution/src/ops/task.rs`, `git/mod.rs`) and there is no
repository setup hook in `.lf/config.yaml`. Git runs `post-checkout` after
`git worktree add`, in the new worktree.

**Natural idle does not happen on this Machine.** The pinned daemon ran 1h45m
with no 300 s gap between cache-file mtimes (Concept review 2), and 4 of 64
roots stayed unloaded. `SCCACHE_IDLE_TIMEOUT` is read only at daemon start, so
no configured value reaches the running daemon without one stop.

**Daemons cannot share a cache directory usefully.** `LruDiskCache::get` answers
from the in-memory index built at start, so a second daemon never sees the
first's later writes. This blocks the per-worktree-daemon alternative below; it
also means the pinned daemon ignores what the stock daemon writes while both
live, which costs nothing because their keys differ anyway.

**What a stop costs (pinned source, not run).** In client-side mode the client
compiles (`commands.rs:729`, `do_compile_client_side`); the daemon only answers
storage RPCs. `--stop-server` kills no compile; a compile still running after
the 10 s `SHUTDOWN_TIMEOUT` loses its cache write and `RecordStats`. The
`loopflow` rlib alone takes 17 s. Unverified: what a client connecting during
those 10 s sees.

**A warm fresh root is still slow, and it is not the workspace crates.** LOO-650
candidate `warm_root`: 258 Rust hits, 1 miss (the `loopflow` rlib, 17 s), all
seven workspace compilers total about 27 s, yet 445 s wall and 410 CPU-seconds.
The rest is outside the cache: proc-macros, build scripts, `bin` and `cdylib`
crates. LOO-922 owns reducing it; this Task's receipts name where it goes.

**The test loop builds more than `--bin lf`.** `cargo test -p loopflow --lib`
enables execution's and storage's `test-support` dev features, so those
libraries compile under a second feature set, and each test executable links.
Unmeasured until this Task; the receipts must show it.

## Approach

### 1. Activation: one Machine command

`scripts/build_cache.py` becomes the one entry point, with verbs:

- `install` builds the pinned revision with
  `cargo install --git https://github.com/mozilla/sccache --rev <REV> --locked --no-default-features --root ~/.local/share/loopflow/sccache/<REV>`
  (wrapper disabled for that build), runs the result once, then edits
  `~/.cargo/config.toml`: `build.rustc-wrapper` to the new absolute path, and an
  `[env]` table with `SCCACHE_CONF`, `SCCACHE_SERVER_UDS`
  (`~/.cache/loopflow/sccache-<rev8>.sock`), `SCCACHE_CLIENT_SIDE=1`,
  `SCCACHE_BASEDIRS_VERBATIM_ENV_DEPS`, `SCCACHE_MAX_FRAME_LENGTH`,
  `SCCACHE_IGNORE_SERVER_IO_ERROR=1`. Every other key, table and comment stays
  byte for byte (`jobs = 2` on this Machine). The config changes only after the
  binary has run. It adds a marked block to the repository's `post-checkout`
  hook (resolved through `git rev-parse --git-path hooks`) that runs the new
  checkout's own `build_cache.py refresh` in the background and can never fail a
  checkout. It refreshes the inventory. It stops no daemon. Re-running is a
  no-op; `--revision` moves the pin.
- `refresh` is today's inventory function, unchanged in behavior.
- `status` prints wrapper path and whether it is the pin, the daemon on the
  pinned socket (pid, executable, uptime, configured idle timeout), roots the
  daemon loaded against roots on disk and whether this checkout is among them,
  Rust hit and miss counts, cache size against its cap. It refreshes the list
  first. `--json` for the harness. Exit 0 when this checkout's root is loaded by
  the pinned daemon; 3 when it is listed and waits for an idle exit; 1 when the
  wrapper is missing or not the pin.

The revision constant lives once, in `build_cache.py`.

`scripts/dev-lf` calls `refresh` and then plain `cargo`. The `SCCACHE_*`
variables it injected now come from `[env]`, so the exec-wrapper form
(`build_cache.py <build command>`) is deleted. `--isolated-build` is unchanged.

Opt-in isolated cache: `SCCACHE_DIR=<dir> SCCACHE_SERVER_UDS=<sock> cargo …`
with the same config file; the measurement harness uses exactly this, which is
its test.

### 2. Measurement: `scripts/benchmarks/crate-rebuild/test_loop.py`

Real linked worktrees from `git worktree add`, the Machine's own Cargo
configuration (pinned wrapper, its `jobs`), and the isolated cache above so
counters belong to the run. Seed both incremental settings outside the samples:
changing the setting changes third-party artifact keys too. Per cohort,
mode order alternating:

| Sample | cache mode (`incremental = false`) | repeated-edit mode (`CARGO_INCREMENTAL=1`) |
|---|---|---|
| new worktree, app edit → `cargo test -p loopflow --lib <filter>` passes | ✓ | ✓ |
| 2 further app edits in that tree → same test | ✓ | ✓ |
| execution edit in that tree → `cargo test -p loopflow-execution --lib <filter>` | ✓ | ✓ |
| 2 further execution edits | ✓ | ✓ |
| new worktree, execution edit → execution test | ✓ | — |

Three cohorts minimum, five if the Machine allows; edits are unique per cohort,
as in `run.py`. Each worktree and target is removed after its samples. `--resume`
retains completed groups; an incomplete group restarts with distinct edits and
a fresh target, retaining previous raw attempts under `abandoned-attempts/`.

Each sample records: wall seconds; client CPU-seconds (`/usr/bin/time -l`);
largest single process RSS; peak summed RSS of the Cargo process tree (1 s
`ps` walk); daemon CPU-seconds delta, reported beside client CPU and not folded
in; exact Rust hit/miss/non-cacheable deltas for the command; per workspace
compiler crate, kind (`lib`, `--test`, `bin`), seconds, and hit/miss where no
other invocation overlapped; test executables linked; Cargo `--timings` unit
durations for everything outside the workspace; jobs, toolchain, load average
before and after. A `--test` rustc time includes its link; the report says so.

`report.py` gains the test-loop table and writes `results/loo-921.{json,html}`
plus the gzipped raw receipts, alongside LOO-650's.

### 3. Defaults, decided by the numbers

- `incremental`: stays `false` unless repeated-edit mode has the lower median
  for *new worktree plus three edits* in both app and execution rows. Either
  way `scripts/testing.md` gets the opt-in line (`CARGO_INCREMENTAL=1`, workspace
  crates then skip the cache) with the measured numbers.
- `SCCACHE_IDLE_TIMEOUT`: stays at sccache's 600 s unless the Machine check shows
  a registered root still unloaded when its first Cargo command starts; then
  `install` sets the smallest sampled value that would have loaded it. **This
  rule fired.** The passive mtime sample (Concept review 2) bounds candidates
  (12 gaps ≥120 s, 4 ≥180 s, none ≥300 s in 1h45m), but any value waits for a
  daemon start that does not come; the choice between shapes a/b/c is open.
- Concurrent-worktree capacity: reported as unmeasured. The report carries
  per-loop peak tree RSS and CPU-seconds as inputs, not a budget.

## Alternatives

- **One daemon per worktree, basedir = its own root** (no inventory, no idle
  wait; in-repo `[env]` with `relative = true` can even supply the root).
  Blocked: daemons do not see each other's cache writes, and it puts
  Machine-only settings in a file CI reads.
- **Restart the daemon when a worktree is created.** Loads the root at once.
  Compiles still running 10 s after the stop lose their cache write; none is
  killed. Planned as the next step if roots waited past their first build; that
  condition is met, and it is shape b below, unanswered.
- **Refresh from `scripts/test.py` and `dev-lf` only.** No hook, but the first
  `cargo test` in a new worktree misses every workspace crate, which is the
  problem.
- **Write the inventory to sccache's default config path, no `SCCACHE_CONF`.**
  Reaches daemons started outside Cargo, but the frame limit and verbatim list
  are environment-only, so `[env]` is needed regardless.
- **Wait for an upstream release / Homebrew.** No date; the pin is one constant.

## If it goes wrong

`rustc-wrapper` names a file that is gone and every Rust build on the Machine,
in every repository, fails at once. Guards: the binary lives outside `/tmp` and
cache directories, `install` runs it before touching the config, `status` exits
1 on it, and `RUSTC_WRAPPER= cargo …` stays the documented escape.

The 10 GiB cap evicts faster than worktrees return (each root's `loopflow` rlib
is its own entry). `status` shows occupancy; the cap is not changed here.

A hit carries the first checkout's debug-source paths. Already documented;
`--isolated-build` is the answer and does not block this.

## Surfaces

| File | Change |
|---|---|
| `scripts/build_cache.py` | `install`, `refresh`, `status`; exec-wrapper form deleted |
| `scripts/dev-lf` | `refresh`, then `cargo` |
| `python/tests/test_build_cache.py`, `test_dev_lf.py` | config edit keeps foreign keys and comments; hook block added once and fail-soft; `status` classification from canned daemon JSON; dev-lf no longer injects `SCCACHE_CONF` |
| `scripts/benchmarks/crate-rebuild/test_loop.py`, `report.py`, `README.md`, `results/loo-921.*` | new measurement and its report |
| `scripts/benchmarks/crate-rebuild/root_probe.py` | imports `refresh` as before; checked still to pass against the pinned binary |
| `scripts/testing.md`, `scripts/README.md` | "Optional compilation cache" rewritten around `install`/`status`; test-loop numbers; opt-in modes |
| `~/.cargo/config.toml`, hooks dir, `~/.local/share/loopflow/sccache/` | Machine state written by `install`, not committed |

Not touched: `.cargo/` in the repository (none exists), CI workflows,
`.lf/config.yaml` and skills (#1843), `scripts/benchmarks/desktop-performance`
(#1950), `Cargo.toml` profiles unless the incremental rule above fires.

## Delete — do not maintain

- `build_cache.py main()`'s exec-wrapper path, its `SCCACHE_BASEDIRS_VERBATIM_ENV_DEPS`
  merging, and `refresh`'s returned environment dict once `root_probe.py` and
  the harness take the variables from one exported constant.
- The paragraph in `scripts/testing.md` saying ordinary Cargo "does not run this
  inventory refresh" and "Neither path installs a wrapper".
- `/tmp/loo650-sccache` is left alone; nothing may reference it.
- Status's hardcoded `idle_timeout_seconds`, `idle_seconds`, `idle_note` and
  the repeated "idle time unmeasured" copy: removed; retain configured timeout
  reporting and distinguish next-start configuration from running state.

## Forbidden outcomes

- Any `SCCACHE_*` or wrapper setting in a file CI reads.
- A second inventory writer or a second place that names the pinned revision.
- `install` stopping, restarting or signalling a daemon, or rewriting unrelated
  lines of `~/.cargo/config.toml` or an existing hook.
- A hit claimed from the shared daemon's counters while another client's
  requests fell in the same window.
- Two Cargo commands at once; a published concurrency budget.

## Remaining implementation acceptance

- Idle work: the passive gap sample exists (mtime table, Concept review 2).
  Timeout installation, any hook stop and any change to exit 3 wait for the
  a/b/c answer. Done: status refresh, configured-timeout reporting, the
  shared-cache cap limit in report and docs, and the docs' busy-Machine and
  stop-cost wording.
- Three warm paired cohorts (2, 3 and 4), then `results/loo-921.{json,html}` and
  gzipped raw receipts, default selection and measured documentation.
- The production shared-daemon new-root check after a natural idle exit. The
  Task checkout is loaded; `/tmp/loo921-machine-root` remains unloaded.
- Gate/CI owns `documented_commands::` and the broader suites; local Rust builds
  cannot currently run within the existing disk reserve.

**Stopped for disk capacity (2026-11-16).** `df -h /tmp` reported 28 GiB free
while the benchmark ran, below the repository's 32 GiB reserve, and later 26 GiB
with its compilers stopped, then 21 GiB at the final check. Only the interrupted
experiment target was removed.
The private daemon and experiment Cargo process group are stopped. The shared
Machine daemon was not signalled. The driver now checks the existing policy
before and during builds, stopping only its own process group on shortage.

Retained source snapshot: `20be2558e7e70c013226568bd8131cef27129118`.
Receipts and all earlier attempts: `/private/tmp/loo921-test-loop`.
Private cache and linked sources: `/private/tmp/lf921-ickrwrsz`.
Both directories have `.cleanup-keep`; the private cache is about 639 MiB.
Four corrected calibration commands passed (cache app/execution, incremental
app/execution). No warm measured row completed. Cohort 1 is calibration and is
excluded, including its earlier unseeded incremental and native-cache bypass
attempts. No comparative report or new default has been produced.

Resume once build headroom exists above the reserve:

```bash
uv run python scripts/benchmarks/crate-rebuild/test_loop.py --output /tmp/loo921-test-loop --cohorts 4 --resume
```

The completed seeds and immutable source snapshot are retained. Resume restores
only the interrupted group to a fresh target and gives its edits a new suffix.

## Acceptance at gate

| Command | Expected | Runs |
|---|---|---|
| `uv run pytest python/tests/test_build_cache.py python/tests/test_dev_lf.py` | pass | gate, CI |
| `uv run python scripts/build_cache.py status --json` in this checkout | exit 0; wrapper is the pin; daemon executable equals wrapper; this root in loaded roots | gate, this Machine |
| `uv run python scripts/benchmarks/crate-rebuild/root_probe.py --sccache <pinned> --output /tmp/lf-root-proof` | `passed.json`; miss before idle, hit after | gate, this Machine |
| `uv run python scripts/benchmarks/crate-rebuild/probe.py --sccache <pinned> --output /tmp/lf-cache-proof` | passes: semantic edit misses, unchanged crates hit, each binary reports its own checkout | once, slice 2 |
| Machine check: `git worktree add` a new root, wait for `status` exit 0 without touching the daemon, then `cargo test -p loopflow-execution --lib skills::catalog:: --jobs 1` | test passes; at least one unshared-window hit and no unshared-window miss among model, definitions, storage, terminal, execution libraries; daemon pid changed by itself | once, slice 2 |
| `scripts/dev-lf --isolated-build --version` | prints this checkout's build identity; no cache request | once, slice 2 |
| `results/loo-921.json` | ≥3 cohorts per row, every field in §2 present or marked unmeasured with the reason | gate reads the file; no rerun |

CI runs only the pytest line; the rest need this Machine's cache and are run
once here, with command and result kept in the PR body.

## Done when

A new worktree on this Machine gets unchanged workspace libraries from the cache
with plain `cargo`, `status` says which daemon and which roots without anyone
reading process lists, and the report states what a new worktree and a repeated
edit cost through a passing test, in both modes, with the default chosen from
those rows.

## Idle sample (stock daemon, superseded)

Stock daemon, 10:13–10:18, `compile_requests` polled every 5 s: one 204 s gap,
none of 600 s in its 56 minutes. Polling resets idle itself (below), so this
cannot set a timeout; the mtime sample in Concept review 2 replaces it.

## Docs written ahead of the code (2026-11-16)

Intended behavior, edited in place; implementation reads these from this checkout:

- `scripts/testing.md` § "Shared compilation cache" (replaces "Optional compilation cache")
- `scripts/README.md` § "Developer build cache"
- `scripts/benchmarks/crate-rebuild/README.md` § "Edit-to-test loop", and the pin now named only by `build_cache.py`
- `AGENTS.md`, one line: the personal Cargo config is written by `build_cache.py install`

Details the docs settled that the sections above left unstated; change the doc
or the code, not both ways:

- `status` prints five lines (`wrapper`, `daemon`, `roots`, `Rust`, `cache`);
  `roots` reads `N of M loaded`.
- `status` exits 0 when no pinned daemon is running and this root is in the
  list: the next build starts one with it.
- `report.py --test-loop <dir>` is the flag that writes `results/loo-921.*`.
- Turning the cache off is a hand edit of `~/.cargo/config.toml`; no `uninstall` verb.
- The docs link `results/loo-921.html` and quote no numbers. Slice 4 adds the
  measured rows to `scripts/testing.md`.

Found while writing, not in Surfaces: `scripts/disk_cleanup.py` `_check_sccache`
runs `sccache --show-stats` from PATH. After `install` that is the Homebrew
binary on TCP 4226, so it reads (or starts) a stock daemon, not the pinned one,
and the 10 GiB cap check stops describing the daemon that writes the cache.

## Implementation revision (2026-11-16)

Pinned `ServerInfo` has no last-request timestamp. Status retains executable,
PID, uptime, loaded roots and counters; `idle_seconds` is null with a reason.
The advertised idle countdown is removed from docs. Root probes observe actual
process exit. No daemon patch or observer service is added. Installation names
package `sccache` explicitly (its repository contains multiple binary packages).

Checks: root probe and real-crate probe passed with the installed pin (receipts
`/tmp/loo921-root-proof` and `/tmp/loo921-cache-proof`); focused Python activation,
dev-entry, cohort validation and third-party inventory: 30 passed.

The first incremental sample exited before `rustc -vV`: the pin requires
`SCCACHE_ALLOW_INCREMENTAL=1`. Installation now supplies it. With
`CARGO_INCREMENTAL=0` it changes no compilation path; the completed cache-mode
group is retained. The failed incremental attempt remains with the raw receipts.

## Calibration correction (2026-11-16)

The initial cache-mode fresh app row had 300 Rust hits / 2 misses. Switching
`CARGO_INCREMENTAL` produced 0 hits / 296 misses in the first incremental app
row, plus 7 incremental compiles. A seed in only cache mode did not warm the
other Cargo artifact graph. Cohort 1 is calibration, retained in the receipts
and excluded from all medians. The driver now seeds application and execution
tests in both modes. Three warm paired cohorts are 2, 3 and 4; no default is
chosen from the cold incremental row.

The installed cc-rs source recognizes `RUSTC_WRAPPER` basenames (`sccache`,
`cachepot`, `buildcache`, `kache`). The first driver used `compiler-wrapper`,
bypassing native caching. Both initial seed attempts and cohort 1 are excluded
from the warm comparison, with receipts retained. The final driver uses
`sccache`, matching `run.py`; both seeds are rebuilt before cohorts 2–4.

Latest production observation: pinned daemon PID 35015 still had only the older
root list after 1h19m uptime; `/tmp/loo921-machine-root` returned status 3. No
shared-daemon natural-idle/reuse claim is made. The 600-second idle default and
`incremental=false` remain unchanged pending the remaining checks and warm data.

Corrected native-cache seed counters: cache application warm-up recorded 273
C/C++ and 109 assembler misses; incremental application warm-up then hit all
273 C/C++ and 109 assembler entries (296 Rust hits). Both execution warm-ups
had 2 C/C++ and 10 Rust hits. The final wrapper therefore exercises cc-rs's
normal native-cache route, not just Rust. These are calibration counters, not
measured cohort results.

`check_architecture.py`: passed. `report.py --test-loop` on the retained dataset:
rejects the missing three warm cohorts before writing a report.

## Concept review (2026-11-16) and polling correction

Read at review: pinned pid 35015 up 01:38:30, `60 of 64 loaded`,
`/tmp/loo921-machine-root` exit 3, cache `10.0 GiB of 10.0 GiB`. The client
takes basedirs only from the daemon's handshake (`cache/ipc_storage.rs`), so
§3's idle rule fired. Its steps 3–6 are done (status reads configured timeout,
unknown-only fields deleted, status refreshes first, `--help` names three
verbs, report states the cap limit). Step 1's counter polling was dropped:
pinned `src/server.rs:954–960` resets idle on every RPC, including `GetStats`
and the `Storage*` calls, so polling prevents the exit it measures. Step 2
(install a timeout) waits on the a/b/c answer below.

## Concept review 2 (2026-11-16 12:11): idle exit cannot load a root on this Machine

**A passive gap sample exists without touching the daemon.** Every cache write,
and every hit served through `get_file` (`lru_disk_cache/mod.rs:427`,
`set_file_times`), leaves an mtime under `~/Library/Caches/Mozilla.sccache`.
Requests that touch no file (handshake, a miss's lookup, stats, `RecordStats`)
are absent, so a gap between mtimes is an upper bound on a true idle gap.

Read at 12:11, pinned pid 35015 started 10:25:55 (up 1h45m), 1018 file
timestamps since its start, 3805 files in the cache:

| Gap at least | 30 s | 60 s | 90 s | 120 s | 180 s | 240 s | 300 s | 600 s |
|---|---|---|---|---|---|---|---|---|
| Count | 56 | 30 | 19 | 12 | 4 | 2 | 0 | 0 |

Longest: 249.1 s from 11:36:49, 240.8 s from 10:27:21, 206.4 s from 11:31:54.

What that settles:

- No timeout of 250 s or more would have fired in 1h45m. 600 s is not a
  mechanism here; "exits after ten minutes without a request" is true and has
  not happened. `60 of 64 loaded` was unchanged at 12:10.
- A shorter value is bounded, not shown: 120 s had at most 12 chances, 180 s at
  most 4, and each count may be zero.
- **No timeout change reaches the running daemon.** `SCCACHE_IDLE_TIMEOUT` is
  read at daemon start, and this daemon holds 600 s with no gap to exit in. Any
  value `install` writes waits for an exit that the sample says does not come.
  Every way forward begins with one stop of the shared daemon, which the brief,
  the docs and Forbidden outcomes all withhold.

**What a stop costs:** now under Risk findings; the earlier server-side
wording overstated it.

**The distinction people must learn for no benefit is listed/loaded.** It
exists only because the load waits for an idle exit. Three shapes, the choice
between them unanswered:

- *a. One stop by hand, then a short timeout.* `install` writes
  `SCCACHE_IDLE_TIMEOUT` (120 s by the table); someone runs `--stop-server`
  once. Listed/loaded, exit 3 and the "next idle exit" line stay. Whether roots
  then load is watched passively: the daemon pid changing by itself (`ps`, no RPC).
- *b. Registration stops the daemon.* The hook block stops the pinned daemon
  when its checkout is not loaded; the next compile starts it with the list.
  Listed becomes loaded within seconds; the idle-timeout work, exit 3's waiting
  meaning and the "loads at next idle exit" copy are deleted. Cost per new
  worktree: cache writes of compiles in flight past 10 s. This is the
  Alternatives entry "the next step only if the idle sample … shows roots
  waiting past their first build"; that condition is met.
- *c. Neither.* Keep 600 s. Docs say a busy Machine may never load a new root,
  and the Task's "new-root hit after natural idle" is reported as not reached.

Its direction items 1–3 (docs busy-Machine and stop-cost wording, plan
rewrites) are done. Timeout installation, the hook stop and any change to exit
3 wait for the answer.

Unchanged: one command to install, own socket, hook registration, incremental
rule, serial Cargo, three warm cohorts once disk is above the 32 GiB reserve
(21 GiB at 12:10).
