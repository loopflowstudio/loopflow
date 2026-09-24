# Scroll during planning refresh — iteration 22

The existing desktop command now measures `scroll_refresh` under
`hierarchy_interaction_ms`. It uses the same 8/256-Task fixture populations and
three retained PTYs. A 300-point viewport makes both lists scrollable; other
scenarios retain their 800-point viewport.

The fixture transport holds one roadmap response while Podium's existing reader
refreshes. The native scroll view moves to the final Task. Captured text first
proves that original row is visible; releasing the response changes Task labels
through the normal query/decode/projection path. The endpoint requires the updated
final Task label, unchanged settled viewport, exact final Task identity, unchanged
Task selection and the same Session records. The following workspace scenarios
still verify all original surfaces, actual input focus, draft submission and
companion PTY response. No production reader, pane owner or state was added.

## Review finding

The first probe compared the requested bottom offset with the post-refresh offset.
That failed for the large lazy list: the request was 13425, while the settled
viewport was 12522 and the correct final Task was visible. Lazy row measurement
corrected the estimated content height. This is an invalid probe assumption,
not evidence that planning refresh moved the viewport.

The corrected probe captures the original destination while the response remains
held, then compares its settled offset across refresh. It retains both sets of
captured labels and offsets in each attempt. The first verification's observer
cost is recorded separately; total capture readiness includes that intermediate
capture/OCR. The short corrected trial passed all 24 observations.

## Evidence and remaining work

The final command passed **504/504 observations**, including 42 scroll/refresh
attempts, with identical measured source before and after:

```sh
uv run python scripts/desktop_performance.py run \
  --output /tmp/loo291-scroll-refresh-baseline --samples 21 \
  --baseline .lf/evidence/desktop-performance/20260924-capture-input
```

[Report](desktop-scroll-refresh-evidence/report.md),
[attempt journal](desktop-scroll-refresh-evidence/attempts.jsonl) and
[source/artifact receipt](desktop-scroll-refresh-evidence/receipt.json) retain the
result. The earlier offset counterexample and short pass are included under their
own source fingerprints. `git diff --check` passes. No broader suite was rerun.

For the twenty warm scroll/refresh attempts per population, total capture/OCR
journey p50/p95 was 318.80/359.18 ms (small) and 357.22/647.27 ms (large).
These include the intermediate capture and both text verifications; they are not
rendering budgets. The comparison correctly reports unavailable because the earlier
baseline used different measurement source. This work introduces no product
optimization or performance budget.

This proves a discrete native scroll during a held fixture refresh at the existing
capture/OCR endpoint. It does not measure wheel-event delivery, continuous gesture
smoothness, compositor presentation, frame hitches, correlated production phases,
configured registry/provider costs or automatic active-Run refresh. Those remain
core obligations, alongside bounded native discovery, complete chapter ownership
integration, fallback compilation and human composition acceptance. Original
external-work trials, the authorized directive edit and long-lived-registry
budgets remain open. No publication, installation, PM mutation or Task completion.
