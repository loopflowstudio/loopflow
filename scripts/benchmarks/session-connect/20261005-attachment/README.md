# Native attachment preservation — 2026-10-05 UTC

```bash
uv run tests/e2e/codex_connect.py --lf target/debug/lf \
  --codex /Users/jack/.local/bin/codex --connect-performance 1 --output <new-dir>
```

Private Loopflow/provider Homes, owned native Codex 0.160.0 UIs and engines,
synthetic local Responses. No installed Home or unrelated client was changed.
Each results file identifies the executable hash and engine birth stamp.
These changing probes are not comparable baseline/optimized populations.

| Evidence | Result |
|---|---|
| `bootstrap-read-failure/` | Rejected UI preserved the engine/driver; ordinary UI bootstrap then failed because passive account reads were fenced. |
| `draft-probe-failure/` | Output/input response succeeded, but raw draft injection did not submit. Retained as a failed fixture attempt. |
| `draft-repair/` | Bracketed paste and observed draft text proved exact draft submission after a rejected replacement. |
| `diagnostic-lock-failure/` | The new file reader still waited behind ordinary SQLite admission. |
| `diagnostic-exit-failure/` | Early process observation still preceded output; the command timed out under the exclusive lock. |
| `diagnostic-repair/` | Timing output arrived while SQLite was locked; process accounting finished after unlocking. Native output 652 ms, input response 1,149 ms; failed replacement retained the draft and engine. |

`diagnostic-repair/timings-0.jsonl` was read while attached. The after-cleanup
file records handled interruption at 1,513 ms from process entry and 998 ms
attached lifetime. Neither number is connection readiness. The external PTY
clock starts before process spawn; the internal phase clock starts at lf entry.
The input response includes a deliberate 300 ms before Enter and synthetic tool
work. First rendered output and exact input-readiness onset remain explicitly
unavailable in the diagnostic. SIGKILL can leave attached lifetime unknown.

The ownership path was separately exercised with:

```bash
uv run tests/e2e/codex_connect.py --launch --public-connect \
  --lf target/debug/lf --codex /Users/jack/.local/bin/codex --output <new-dir>
```

`ownership-results.json` retains the assertion results and identities, omitting
its duplicated full Session event journal. This controlled protocol fixture
proved stale-client writes rejected, a sibling thread preserved, explicit
replacement preserving the engine, and current-owner exit closing an exclusively
owned engine. It does not prove native rendering or drafts. Its executable
predates the diagnostic additions; the ownership implementation is unchanged.

Normal Flow reviews launch `--mode tui` through `run::exec_prompt` and
`exec_session_with_env`, without publishing the batch Harness's relay endpoint.
The Task's exact review selector therefore still needs a first-launch ownership
revision; a conversation relay success is not evidence of review continuity.
Completed/paginated history, authenticated providers, repeated native connects,
representative baseline targets and measured speedup remain unproven. Claude and
OpenCode were not measured. Jack Heart's live reproduction was not touched.
