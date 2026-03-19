# Review — PM priority buckets

## What was implemented
- Reworked roadmap guidance from exact numeric staging (`01-*`, `02-*`) to four semantic buckets (`p0-*` through `p3-*`) in wave authoring docs and built-in prompts.
- Updated ingest and PM sync to carry shared `PriorityBucket` meaning through local roadmap files, the shared PM model, and the Asana/Linear adapters.
- Added regression coverage for prompt/docs guidance, ingest bucket selection, and provider priority mapping.
- Polished adjacent built-in prompts so they no longer describe roadmap items as numbered-only artifacts.

## Key choices
- Kept **semantic meaning** in loopflow (`P0`/`P1`/`P2`/`P3`) while letting providers keep their native UI labels.
- Preserved **legacy numbered files as a fallback** during transition instead of forcing a flag day migration.
- Left **within-bucket ordering intentionally loose**; ingest uses filename order as a local fast path instead of inventing a new shared total-order model.
- Extended prompt parity checks to catch leftover numbered-roadmap language in built-in steps, not just the primary authoring docs.

## How it fits together
`PriorityBucket` is now the shared planning primitive. Local wave files encode that primitive in `p0-*` through `p3-*` filenames, `ingest` picks the highest-priority non-empty bucket first, and PM sync translates the same meaning into each provider's native priority representation (Asana custom-field labels, Linear native priorities).

The prompt/docs layer teaches the same model agents execute: write bucketed roadmap items, pick urgent buckets first, and avoid pretending there is one exact cross-provider queue.

## Risks and bottlenecks
- The transition still depends on mixed local states behaving well: bucketed files are preferred, but legacy numbered items can still exist.
- Asana priority mapping relies on custom-field option names being semantically recognizable (`P0`/`P1`/`P2`/`P3` or `Urgent`/`High`/`Medium`/`Low`). Unexpected labels will need follow-up.
- Within-bucket ordering is deliberately underspecified, so provider round-trips preserve bucket meaning, not a strict shared sequence.

## What's not included
- Notion integration.
- README sync or supporting-doc import.
- Arbitrary user-configurable priority taxonomies.
- A new exact ordering scheme inside each bucket.

## Validation
- `cargo fmt --check`
- `cargo clippy -p loopflow -- -D warnings`
- `cargo test -p loopflow`

These checks passed after the final prompt/doc consistency polish.
