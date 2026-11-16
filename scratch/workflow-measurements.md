# Intended live workflow measurements

“ok, keep wroking on getting all the intended workflows measured”
“not the job of this task to fix it”

Collect ordinary-use app journal observations for app launch, Task selection,
Session return, new conversation, first response, filtering, file opening,
reconnect, Start and Complete. Run against the live store on the live Machine;
keep repository/build/source and failed attempts separate. Report app-process
start separately from an external launch request if the latter clock is absent.
No latency optimization, shipping approval, or quiet-host prerequisite.

First repair: launch currently ends at sidebar rows without input readiness;
process-start records are excluded by the report. Session return has no captured
input. Task selection needs its original interaction retargeted to its Session,
including already-focused retained terminals. Then new conversation, file input,
reconnect and actual first-response visibility. Do not substitute PTY echo,
spinner, publication callback or transcript completion for a response word.

Delete the sidebar-only launch usable endpoint. Reuse the terminal focus/control
checks, actual captures and shared journal. Keep missing workflows on the charts.

Checks before live collection:
- `uv run pytest python/tests/test_desktop_timings.py -q` — 14 passed; Ruff passed.
- Decoder tests (`cargo test -p loopflow-execution --lib --features test-support harness::codex_decode::tests:: --jobs 2`, LF_* cleared) — 19 passed; `cargo fmt` and `cargo clippy --all-targets --jobs 2 -- -D warnings` passed.
- Shared/Mac interaction, launch, response visibility and file tests — 48 passed.
- Final interaction/launch/response checks after reconnect gating — 21 passed.
- Desktop build after moving launch observation to the actual focused window's terminal display callback — passed.

Live probe LOO-929 was reopened for these trials, without a Graph run. The
owned checkout contains `scratch/performance-probe.txt` as a plain text editor
fixture. Restore completion after collection. New Start/Complete timings from
its earlier run remain separate by build. No native figures from the new hooks yet.
