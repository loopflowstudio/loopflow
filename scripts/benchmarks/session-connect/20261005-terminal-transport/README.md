# Retained terminal comparison — 2026-10-05 UTC

```bash
uv run python tests/e2e/terminal_transport.py --output /tmp/terminal-comparison
```

The final trial (`trial-12/results.json`) completed on macOS 26.0.1 ARM64,
tmux 3.7c. Each trial owns a private socket, empty tmux configuration, temporary
Home, canned terminal application and presentation PTYs. No provider, account,
installed Loopflow Home or existing Session participates. The raw-PTY arm is a
transcript-replay counterexample, not an implemented attachment broker.
Trial 12 runner SHA-256:
`46d6d750ac74c2428eec47ebcc8f0f0e2fe6f8c6bdc6bdc8abf78e72b2e6286b`.

| Behavior | tmux presentation | Raw PTY transcript |
| --- | --- | --- |
| Truecolor | Preserved `38;2;17;93;201` output | Preserved output bytes |
| Bracketed paste | Exact input bytes reached fixture | Exact input bytes reached fixture |
| Late display | History and draft text replayed | History and draft text replayed |
| Detach/reconnect | Three reattachments; same fixture PID; exact retained draft submitted | Not implemented as a multi-client transport |
| Passive typing | Ignored | Requires an owner-side fence |
| Explicit transfer | Old viewer fenced; new viewer typing accepted | Not implemented |
| Independent view size | A 60-column passive view retained the 100×30 application PTY | One PTY size; no view renderer |
| Clipboard queries | Live query diverted to passive viewer; its reply did not reach application | Replays historical query; an unfenced reply reaches application |
| Cleanup | Owned server and fixture absent; no cleanup errors | Fixture terminated and reaped |

The initial controller answered a clipboard query successfully. After input on
the passive viewer, an independently triggered application query appeared only
on the passive viewer. Its synthetic reply was not observed in application input.
After explicit control transfer, the new controller's query/reply worked.
See `tmux-controller-query.bin`, `tmux-passive-query.bin` and
`tmux-provider-input.bin` in trial 12. These are clipboard **text protocol**
observations, not proof of a native provider's image-paste failure.

Raw replay includes `OSC 52` from an earlier live exchange. The probe submits a
synthetic response to that replayed query and observes it in the real application
PTY. A relay must suppress historical queries and fence terminal responses as
well as keystrokes. This does not demonstrate a vulnerability in an existing
Loopflow relay; that presentation owner has not been built.

Neither experiment proves full keyboard protocols, native image decoding,
paginated provider history, original Session/driver identity, exact Flow review
completion, authenticated providers or compositor presentation. The fixture
stores a canned draft in process memory; it is not Codex's composer. Width
evidence establishes a retained application size, not independent reflow.
There are no latency samples, median/p95 targets or speedup claims.

## Trial history

- 01: completed early comparison; clipboard query support was not enabled;
  pane target returned an empty dimension. These observations were inconclusive.
- 02: invalid clipboard option name; startup failed. Cleanup ran, but this
  early runner did not retain partial cleanup results in JSON.
- 03: enabled `get-clipboard request` and corrected pane target. Query originated
  from controller input, so it did not test a recently active passive viewer.
- 04–05: `send-keys` refused the most recent read-only client. Replaced that
  observer action with a fixture-owned query trigger, independent of client input.
- 06–07: passive-query counterexample; trial 07 also verifies the initial
  controller's clipboard response before testing the passive viewer.
- 08–10: takeover probe failed because `refresh-client -f !read-only` left the
  target read-only. Trial 10 records flags and application bytes. No takeover
  success is inferred from the command's exit status.
- 11: `switch-client -r` performed the transfer; positive/negative input proofs
  passed. The original controller is made read-only first, avoiding two writers.
- 12: adds explicit fixture-process cleanup observation. All asserted transport
  behaviors passed; clipboard routing remains a contrary observation.

All trial JSON is retained. Detailed byte artifacts remain for trials 07 and
10–12. The revised observer invalidates cross-trial performance comparison.

Review finding: plain read-only attachment does not establish terminal-response
ownership; raw transcript replay does not establish safe late presentation.
The production deletion cut must retain a screen state separately from live
queries and bind replies to the current controller before replacing native resume.
