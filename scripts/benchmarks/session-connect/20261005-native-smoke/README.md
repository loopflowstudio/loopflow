# Native connection smoke evidence — 2026-10-05 UTC

```sh
uv run tests/e2e/codex_connect.py --lf target/debug/lf \
  --codex "$(command -v codex)" --connect-performance 1 --output /tmp/connect-new
```

Jack Heart requested live Session connection performance (LOO-378). These five
sequential development probes establish a native startup failure and its narrow
repair. They are not the required performance baseline or a speedup claim.

| Probe | Output ms | Input response ms | Result |
| --- | ---: | ---: | --- |
| baseline-01 | unavailable | unavailable | Folder trust screen; 45 s timeout; cleanup also failed |
| baseline-02 | unavailable | unavailable | Remote task rejected permission overrides; exit 1 at 987 ms; engine endpoint cleared |
| repair-01 | 796 | unavailable | Input batched with Enter triggered paste handling; 45 s timeout |
| repair-02 | 1572 | 2743 | Native input response; Session/thread/PID/generation retained in store |
| repair-03 | 1052 | 1699 | Native input response; same store identity and OS PID birth stamp |

The production repair builds remote resume without local permission, model or
workspace overrides. The live engine already owns those values. Ordinary
stopped resume retains its existing command. Each directory retains the original
JSON sample, raw PTY transcript and seed log. Raw PTY bytes contain terminal
escape sequences: inspect as bytes, not by printing them into a terminal.

Baseline binary: `c62ad7712dca85996d658656d7a354c67db26381638433377fcea4ede68a0971`.
Repaired binary: `a799dfe65bd574eced57808a908b0935211563235e35aafad6950ef21189db8c`.
Both use real Codex 0.160.0 app-server and native UI, a synthetic local Responses
server and disposable Homes. No configured account or installed Home was used.
One fresh owned engine and one held seed turn per invocation; no historical
population or draft. Observer polling, provider startup, synthetic tool execution
and overlapping build/static checks affect these numbers. Later probes separate
text and Enter by 300 ms; that delay is included in the input-response endpoint.
No median/p95 targets can be selected from this changing, failed population.

Review blocker: `connect_live_codex` transfers the driver before native UI startup
and finishes it even when startup fails. Baseline-02 reproduced loss of the live
endpoint. Removing one rejected argument repairs that trigger, but does not
establish the preservation contract on another startup failure. The lifecycle
needs a design revision before performance optimization or delivery. Flow-review
routing still stops native clients and launches without the relay. Its replacement
route was not actively exercised; Claude/OpenCode were not measured.

Remaining acceptance: failed-attachment preservation, Flow-review attachment,
representative retained histories and drafts, authenticated-provider checks,
comparable repeated baseline and preselected targets, per-invocation supported
phase diagnostics, and measured optimization. These probes earn no Desktop KR.
