# Stream parser: suppress unknown protocol events

## What was implemented

Two changes to the stream parser in `engine/stream.rs`:

1. **Default `StreamFormat` changed from `Raw` to `Human(false)`.** New callers get human-readable output by default instead of raw JSON passthrough. Existing callers that explicitly set `StreamFormat` (like `run.rs:234`) are unaffected.

2. **Unknown protocol JSON types are now suppressed.** Previously, any `{"type":"..."}` line with an unrecognized type returned `Passthrough`, causing callers to print raw JSON to the terminal. Now these return `Skipped`. Non-JSON lines (plain text, malformed input) still pass through — the distinction is: if it parsed as protocol JSON (has a `type` field), we own it and suppress it; if it's not protocol JSON, the caller decides.

Four additional Claude event types were explicitly added to the skip list: `content_block_start`, `content_block_delta`, `content_block_stop`, `rate_limit_event`. These were previously caught by the `_ => Passthrough` fallback and leaked as raw JSON.

## Key choices

**Suppress-by-default vs. passthrough-by-default for protocol JSON.** The old design assumed unknown types might contain useful output. In practice, they're always internal protocol events (rate limits, content block deltas) that pollute human-readable output. Suppressing unknown types is safer — if a new event type carries meaningful content, we add a parser arm for it.

**Kept `Passthrough` variant.** It's still needed for non-JSON lines (agent stdout that isn't protocol), so the variant stays. The semantic shift is narrower: `Passthrough` now means "not protocol at all" rather than "protocol we don't understand."

## How it fits together

`StreamParser::feed_line` is the single entry point. Callers in `engine/agent.rs` (CLI) and `lfd/executor/mod.rs` (daemon) both match on `Passthrough` to print raw lines. The change means they only print genuinely non-protocol output now.

## Risks and bottlenecks

- **Future agent event types with displayable content** will be silently suppressed until a parser arm is added. This is the intended tradeoff — explicit parsing over implicit passthrough.
- No performance impact — same code path, just a different enum variant returned.

## What's not included

- No changes to how `Raw` mode works (it bypasses the parser entirely).
- No changes to the `Passthrough` handling in callers — they still print raw when they get it, they just get it less often.
