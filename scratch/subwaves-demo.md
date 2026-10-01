# Release subwave demo · 2026-09-30

Interactive review for LOO-354 with Jack Heart. Review remains open; Jack has
not yet supplied feedback or accepted behavior in this Session.

## Experience under review

Selecting `infrastructure/release` supplies Infrastructure's top-level Wave
documents followed by Release's, from the executing checkout. Release owns its
objective, memory and authored schedule. The [accepted design](define-durable-subwave-identity-and.md)
owns scope; [read-slice evidence](subwave-read-slice.md) and the design's gate
record retain earlier checks and their limits.

## Observed preview

At checkout commit `9385c811b`, the existing compiled `lf-prompt` rendered the
current checkout for `--wave infrastructure/release --surface cli`. The command
used a disposable Home/database with inherited LF_/LOOPFLOW_ authority removed.
The rendered Wave documents were exactly, in this order:

1. `wave/infrastructure/GOAL.md`
2. `wave/infrastructure/MEMORY.md`
3. `wave/infrastructure/release/GOAL.md`
4. `wave/infrastructure/release/MEMORY.md`

Each occurred once. No other Wave file tags appeared. The complete preview is
[release-demo-prompt.md](../.lf/tmp/subwaves/release-demo-prompt.md), a local
artifact. This is the prompt renderer's output, not a provider response or
Desktop acceptance. The earlier authored-content proof used copied files;
this preview reads the supplied worktree itself.

Preview check: compiled `lf-prompt --repo <this checkout> --wave infrastructure/release --surface cli` → four expected Wave files in order, once each; disposable data, 32.3 GiB free, no rebuild.

## Feedback and remaining review

No new human feedback, agreed design changes or acceptance are recorded yet.
The demo has not exercised a configured provider or Desktop. Jack's judgment
of the selected and inherited context remains pending. The preview alone does
not finish the requested demonstration through the configured path.

Next: review the Release context with Jack, then exercise the selected experience
and record observations and feedback here. Preserve any requested changes in
the accepted design before marking this review ready. Do not infer acceptance
from silence or a successful local check.

After landing, installed `lf` owns creation of Release's Linear Initiative and
plan, release-focused Task/KR moves, and live schedule cutover. No such operation
ran during this preview. The following Flow decision owns navigation; this note
records no navigation verdict.
