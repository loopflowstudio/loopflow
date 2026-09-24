# Native capture/input baseline — 24 September 2026

Compare a future compatible measurement from the repository root:

```sh
uv run python scripts/desktop_performance.py run --output /tmp/desktop-after --baseline .lf/evidence/desktop-performance/20260924-capture-input
```

462/462 observations passed: eleven scenarios, two populations, one first
interaction plus twenty warm attempts each. `report.json` contains the original
per-attempt values and host/build/source provenance; `attempts.jsonl` and `run.json`
allow report reconstruction. `receipt.json` pins the original artifact hashes and
original ignored evidence location. These files were copied unchanged during
Product memory curation; this is the implementation writer's result, not a rerun.

The endpoint is forced native bitmap capture/text verification and retained PTY
input/replies. Observer overhead is included; OS event delivery, compositor
presentation, frame hitches, configured registry reads and vendor-provider readiness
are excluded. Synthetic Run rows do not prove shared liveness discovery. The runner
refuses comparison when host, endpoint, measurement source or population differs.
No optimized after-build, rendering budget or improvement is established here.
