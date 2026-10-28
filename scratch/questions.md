# LOO-664 assumptions — 2026-10-27

- The bounded repair interprets “never commit telemetry” as never automatically
  adding generated local telemetry. Explicitly pre-staged or already committed
  telemetry is caller-owned Git state; the existing whole-index checkpoint can
  include pre-staged telemetry. Changing that requires selective checkpoint
  semantics or index preservation beyond this pathspec repair. Preserve caller
  state rather than silently removing it. This limitation must remain visible in
  review; the staging probe does not establish a blanket guarantee for that case.
- Preserving staged and unstaged work retains the accepted automatic checkpoint
  behavior, which consolidates both into a commit. It does not preserve their
  former staging partition. No product choice blocks the demonstrated hotfix.
