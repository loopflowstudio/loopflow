# Complete local run records — design review

## What was implemented

Every provider launch now establishes a durable local capture before token spend. The capture stores exact provider-facing prompts, attributed context assets and inclusion decisions, normalized conversation/tool events, observed provider-native frames, provider usage, lifecycle state, and optional vendor-session identity below `~/.lf/traces`, with searchable relationships in SQLite.

`lf trace` now returns one metadata envelope and can stream or render recorded events. `lf context` returns turn/asset/decision datasets and human summaries without opening prompt bodies. `lf doctor` verifies launch coverage, artifact safety and availability, event sequencing, terminal turns, token reconciliation, usage reconciliation, and orphan directories.

The unused output-log path and `run_events.context` field were removed. README and TESTING examples describe the new readers and focused verification commands.

## Key choices

- `run_id` remains the trace and `process_id` remains the process span; launches and turns have their own identities. One process may launch several agents and one launch may contain several turns.
- SQLite owns searchable metadata. Exact prompts and JSONL bodies remain private local files with relative, traversal-safe paths.
- Capture is fail-closed before provider launch and fail-open-but-visible after launch: mid-run append failure marks the launch partial and makes doctor red without killing the provider.
- Context kind and origin scope are separate axes. Exact prompt byte ranges, SHA-256, isolated tokens, and prefix-attributed tokens let readers explain both inclusion and assembly overhead.
- Exact token accounting shares one cl100k encoding across totals, prefix boundaries, and isolated ranges. The copied cl100k regex is checked against actual token boundaries at runtime; any mismatch falls back to the slower direct calculation.
- Migration 063 repairs the partially applied 062 contract because migration ids are immutable once observed by the long-lived ledger. The repair preserves captured rows and process evidence.
- Provider totals stay `NULL` unless Loopflow observed a usage event; absence is no longer reported as zero.

## How it fits together

Prompt assembly produces an exact `PreparedTurnContext`. The launch gate atomically publishes private artifacts, commits launch/turn/assets/decisions in one SQLite transaction, and only then invokes the provider. One capture handle fans observed raw and normalized events into JSONL while terminal updates persist usage and completeness. `lf trace`, `lf context`, and `lf doctor` read the same metadata contract.

## Validation and measures

- Full CI matrix: Python 53 passed; website 59 passed with 3 intentional skips; Swift 302 passed; E2E smoke passed; Loopflow macOS test build passed.
- Final Rust checks after gate fixes: `cargo fmt --all -- --check`, focused trace/prefix/migration tests, clippy with warnings denied, and 1,351/1,351 nextest cases passed with 3 intentional skips.
- Provider conformance fixtures cover Claude, Codex, and OpenCode.
- Long-lived ledger after release-profile probes: 100% of captured headless launches are complete; all assembled turns reconcile asset tokens exactly; normalized event files are parseable.
- A release-profile 18,868-token probe recorded 13 ms gather, 39 ms exact render, and 64 ms durable persistence. Exact render is below the 100 ms target. The pre-polish debug path measured 439 ms and also rebuilt the context twice before launch.
- `lf context --json` over 30 days and full local history takes about 10 ms warm and does not open prompt or transcript bodies.
- `lf trace --events` begins streaming the largest current capture in about 10 ms.
- Current trace storage is under 1 MiB. Captured normalized conversations are about 80 KiB each for the exercised runs.

## Risks and bottlenecks

- Exact capture can contain pasted secrets and tool output. Files are mode 0600 and directories 0700, but there is intentionally no retention or redaction policy in this branch.
- SQLite and the filesystem cannot share a transaction. The two-phase publish can leave a pre-launch orphan after process death; doctor names it.
- The cl100k regex is duplicated from pinned `tiktoken-rs` to avoid repeated full-prefix encoding. Runtime boundary validation preserves correctness if that implementation drifts, at the cost of falling back to slower accounting.
- The long-lived ledger still has three historical lineage failures predating this work, so `lf doctor` correctly remains non-zero for lineage. Migration 067 starts capture coverage after the final audit contract and mixed-version development interval; release-profile probes from this branch capture completely.
- Interactive TUI/IDE handoff is deliberately prompt-only. Loopflow does not claim a complete transcript after control passes to the vendor.

## What's not included

- Mac UI or Swift DTOs for trace browsing.
- Historical import/backfill or copying vendor session directories.
- Compression, deduplication, retention, redaction, upload, or remote telemetry.
- Controlled eval tasks or semantic judgment of context quality.
- Repair of historical ledger lineage or PM/Linear project retirement.
