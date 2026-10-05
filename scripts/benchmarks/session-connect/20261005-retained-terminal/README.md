# Retained native Session terminal — October 5, 2026 UTC

Private Loopflow/provider Homes, Codex 0.160.0, tmux 3.7c, macOS 26.0.1 ARM64.
Real native UIs use synthetic local Responses; no account or installed Home is
required. Raw logs, screen bytes and diagnostics are in `evidence.tar.gz`, with
per-run results also collected in `results.json`. Directory names below refer to
archive members. Jack Heart's live Session was not touched.

| Population | Passed | Output median ms | Output p95 ms | Composer response median ms | Composer response p95 ms |
|---|---:|---:|---:|---:|---:|
| Review baseline | 20/20 | 86.3 | 92.0 | 87.6 | 93.7 |
| Review after lookup reduction | 20/20 | 81.7 | 89.7 | 83.5 | 97.1 |
| Conversation baseline | 20/20 | 87.7 | 95.0 | 89.4 | 99.8 |
| Conversation after lookup reduction | 20/20 | 82.7 | 86.3 | 83.9 | 86.8 |

Each population retains eight native turns (about 228–235 KB of history), one
unsent draft and one native process for twenty detach/reconnect samples. p95 is
nearest-rank. Before the lookup reduction, both endpoints' budgets were selected
as median <=100 ms / p95 <=125 ms from review-03. Both review runs meet them.
The after run removes one tmux process (`has-session`): `list-clients` establishes
existence and controller together. Output median fell 4.6 ms; input p95 rose
3.4 ms. This single population pair does not establish a reliable tail speedup.
Rust test compilation overlapped part of review-04; preserve that load difference.
No result is comparable to the old UI-replacement smoke's 652/1149 ms endpoints.

The same fixture shape, host and debug build profile are used before/after;
Session IDs, random draft markers and generated timestamps differ. Exact binary
hashes, history lengths/hashes, raw sampled terminal bytes and invocation
JSONL diagnostics are retained per run. The initial data population is fixed
within each run; sample suffixes are removed before the next detach.

Baseline diagnostics (all attached invocations, not only measured samples) have
median process-relative milestones: started 59.0 ms, store opened 61.5 ms,
lookup complete 62.2 ms, connection prepared 73.0 ms, attached observed 97.3 ms.
These separate CLI startup, store/selector reads and terminal preparation.
External PTY output can precede the polled attachment milestone. Diagnostics
explicitly leave native output/readiness unavailable; composer response is an
external upper bound, not an onset event or compositor measurement.

## Behavioral proof and limits

Native conversation and exact `task_…:…:feature:review_kickoff:0` review selectors
retain Session/capture, driver/provider generations, PID/birth and history prefix.
A rejected non-TTY attachment preserves the original UI; twenty reconnects keep
the composer draft; passive typing cannot alter it; explicit takeover detaches
the old controller before new input; final native submission contains the draft.
Review Complete fails before Ready, native `lf session ready` publishes feedback,
and completion settles the exact capture/Flow and removes the owned UI/server.
Successful runs have no cleanup errors. Stale-review replacement fencing also
has the existing Rust `review_actions_preserve_selected_attempt_across_replacement`
regression. No view gains review or Session driver authority.

The final-review fixture uses a one-review `feature` template. It does not prove
starting the full feature Flow's next worker. review-01 saved feedback but failed
to start that worker because its isolated Home had no managed Codex account.
This is retained as a failure, not converted to a pass. review-02 exposed wrong
probe assertions (agent-turn consumption vs review settlement, and an attached
event demanded from an intentionally rejected connection). review-03 fixes both.

Snapshots-02 passes the clipboard/query fixture: passive viewers use capture-pane
commands, never attached tmux clients, so only the controller receives live
queries and late snapshots contain no historical queries. This is text-query
proof, not native image attachment. Native-01 through native-06 retain earlier
selector, launch-lock/trust, observer SQL, takeover-lock and undrained-PTY failures.
Native-07 passed three shorter smoke samples. The original rejected read-only
client and raw replay evidence remains in ../20261005-terminal-transport/.

Claude/OpenCode, authenticated services, native images, paginated/very long
history and direct standalone first-launch retention are unproven. Existing
standalone live UIs without a retained transport are left unchanged and report
that they cannot attach. No Desktop or sustained-use KR follows. Passive output
holds at most one bounded snapshot and cannot stall the provider. A later repair
makes writes nonblocking with a two-second no-progress bound and best-effort
terminal restoration. review-05 proves simultaneous connects leave one controller
and SIGTERM of an unread passive view preserves the native UI; three reconnects,
exact review settlement and cleanup also pass. This is not a saturated-output
stress test or SIGKILL recovery proof. Remaining coverage is acceptance work
before delivery.
