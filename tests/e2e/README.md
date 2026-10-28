# E2E tests

Shell-based workflows for CLI and worktree behavior.

```bash
tests/e2e/test_smoke.sh
```

Long-running workflow tests for mechanical `lf` commands:

```bash
tests/e2e/test_full_cycle.sh
tests/e2e/test_sync_safety.sh
```

Exercise repository planning reconnect through real work-watch and Graph commands on Linux:

```bash
cargo test -p loopflow --test planning_reconnect_tests
```

Requires `uv`, Python and OpenSSL. The fixture uses an isolated Machine/store and
local HTTPS proxy with synthetic Linear state and credentials. Its CA is trusted
only by CLI children through `SSL_CERT_FILE`; macOS platform TLS ignores that
setting, so reconnect executes only on Linux. The Rust harness still compiles on
macOS; its tests are explicitly ignored there, not reported as behavioral passes.
Include its Python command calls when renaming Task controls; Rust CLI parsing
tests alone cannot catch stale commands in the Linux-only fixture.
Changes to Task completion or Graph storage must run both reconnect proofs on
Linux, including a disposable container when developing on macOS. Completion
uses `task complete --reason`; retained execution is read from `task_graphs`.
Include this fixture when changing Work storage or receipt field names: its SQL
must read canonical Task fields, not retired provider JSON. To prove pending saves
across a closed connection, make the peer unavailable during the save; short CLI
commands attempt delivery even with Desktop closed.
`task_abandonment` and `local_planning` cover
portable local decisions and deletion, retry identity and retained execution in
both connection modes without provider access. No installation or live provider is used.

Exercise Linear expiry and rejection through an explicit experimental CLI:

```bash
# Inside a disposable Linux container with a freshly built candidate:
LF_HOME="$(mktemp -d)" uv run tests/e2e/linear_oauth.py --lf target/debug/lf
```

Give the container `--add-host api.linear.app:127.0.0.1`, Python, Git, and
`uv`. The experiment needs a registered repository. Never mount a real Machine
or credentials into this container: the fixture replaces its Linear row
and seeds planning data in the explicit disposable store.

The fixture serves synthetic Linear HTTPS on port 443 with a temporary CA
trusted only by its CLI children. It enters through the installed launcher and
asserts fresh planning, encrypted rotation, bounded replay, and rejected-refresh
preservation with and without a snapshot. Its JSON receipt identifies the CLI
digest and installation. This proves the installed Linux path against simulated
provider responses; live Linear behavior and the operational reliability window
remain separate evidence. No production endpoint override is needed.

Exercise first installation and recovery inside a disposable Ubuntu container:

```bash
uv run --no-project --python 3.14 /fixture/install_bootstrap.py
```

Copy `tests/e2e/install_bootstrap.py`, `release/install.sh`, and a Linux candidate
CLI to `/fixture/install_bootstrap.py`, `/fixture/install.sh`, and
`/fixture/bin/lf`. Build the CLI with release provenance in a separate
disposable source snapshot; a snapshot without Git history names its migration
order with `LOOPFLOW_MIGRATION_ORDER` (see `scripts/test_task_installation.py`).
The runtime container needs curl, OpenSSL, CA certificates, Git, useradd,
runuser, and uv; run the check as root without host Machine mounts. Optionally copy
a checksum-verified older released pair into `/fixture/prior` to exercise the
external-installer transition.

The script creates separate OS accounts and a local HTTPS release endpoint.
It exercises the real CLI, verified shell installer, store creation, repeat
installation, missing-CLI repair, checkout preservation, and recovery after
a forced activation failure. Transport is simulated; this does not replace a
public release-channel demo. Discard the container afterward.

## Native Codex terminal capture

```bash
uv run python scripts/test_network.py uv run python tests/e2e/codex_terminal.py \
  --lf target/debug/lf --codex /path/to/native/codex
```

Use a native Codex binary supporting app-server and `--remote`. The headless PTY
fixture uses disposable provider and Loopflow homes, a loopback model endpoint, and an
independently authored wrapper supplying trust plus SessionStart/Stop hooks.
Only the wrapper's hooks are pre-trusted, and stale persisted decisions sit at
Loopflow's hook IDs; Codex skips untrusted thread hooks silently, so context in
the model request is the proof Loopflow's own hooks ran. It checks immediate
skill startup, deletes the native context binding before reconnect to prove
restoration from the saved Session, then requires fresh scratch context, a steer
saved between turns delivered at the next prompt, two retained completed turns,
provider cleanup, and preserved user settings.
Add `--context-repo /path/to/loopflow-checkout` to run the built-in `demo` with
copied agent instructions, scratch and Infrastructure Wave context. Wave context
is supplied as scratch in the isolated store, without a planning provider binding.
Add `--cmux-wrapper /path/to/cmux-codex-wrapper` to exercise the cmux entry
script with an isolated home, a nonexistent socket and computer-use attachment
disabled. It cannot report into existing panes.
The rollout must contain exactly one conversation, two turns and two tool calls.
It uses no account or login. This proves protocol launch composition; live account
configuration and actual cmux tab tracking remain separate acceptance checks.
On reconnect, the fixture types a Unicode draft, fails a stand-in cmux title
refresh, resizes the PTY, edits and submits the draft. It checks the native model
request and retained diagnostic file, and rejects raw Loopflow warnings in the
terminal. Add `--transport-failure` for a separate case: a disposable Unix socket
proxy disconnects the renderer without a WebSocket close handshake while the
draft is visible. The same PTY reconnects, accepts another edit, and submits the
exact Unicode draft once. Native history must retain one conversation, two turns
and two tools. `session_launch_contract` covers the warning output boundary with
deterministic Claude, Codex and OpenCode stand-ins.

Add `--startup-failure before-thread`, `after-thread`, or `first-turn` to drop
only the disposable renderer's connection at native startup protocol boundaries.
Each case checks the retained Session diagnostic's operation, direction and
AgentProcess, reads it through `lf history show SESSION --events`, and rejects
raw diagnostic output in the PTY. These probes use real Codex with a local model.
`after-thread` withholds the frame carrying the new thread's ID: Codex then shows
"Reconnect failed" and ignores `/exit`, so the probe leaves with Ctrl+C and checks
that the printed `lf resume` continues the same thread without repeating its input.
The other phases stop after diagnostic readback. Leaving stops Codex, so a first
turn already accepted in that window ends interrupted and may lose its input.

## Released capture-history preservation

```bash
uv run python tests/e2e/capture_history.py \
  --released-archive /tmp/lf-aarch64-apple-darwin.tar.gz \
  --candidate target/debug/lf
```

Supply the matching v0.13.3 CLI archive from its published release. The fixture
checks its pinned SHA256 before extraction, creates a temporary Machine with stub
providers, and exercises ordinary public commands with both binaries. It preserves
capture paths/bytes, native identity, usage and released review feedback through
candidate reads, replay, resume, review settlement and nested Process ancestry.
`runs/` remains the one opaque capture root; no migration or installation runs.
Provider and terminal transport are simulated, so this is not installed acceptance.

Pair this with `session_lifecycle_tests`' interruption, nested Task attribution and
review replacement cases and `session_cli_tests`' stale actor/identity cases.
Full affected verification belongs to gate. Do not run installation preflight or
promotion fixtures on a host account merely by overriding HOME/LF_HOME: installation
uses getpwuid. The three tests named in LOO-370's current isolation steer remain
isolated-CI owned until PR #1444's disposable-account runner is integrated.
