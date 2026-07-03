# Open questions — jack-heart.concerto-wavechat

## WaveChat first surface: data source (resolved by best judgment)

The brief assumed `lf wave` writes per-pass logs under `wave/<name>/streams/`
that WaveChat could parse into turns. That capture was **removed** ("inherit
terminal instead of teeing passes"): nothing read the streams, and each inner
`lf -b goal` pass already writes durable logs under the agent's log dir. So
there is no `streams/` source to parse.

Decision taken (the brief's documented fallback): back `WaveChatView` with a
small local `WaveChatSource` that reconstructs two turns from the two-file wave
surface `lf wave` maintains — `GOAL.md` (the operator's directive → opening turn)
and `MEMORY.md` (the wave's working memory → its reply). A "latest state"
snapshot, not a live transcript.

## Base branch note

The brief said to open this PR against `jack-heart.lf-loop` (PR #778). By the
time the slice was ready, #778 had been squash-merged into `main` and its branch
deleted, so this PR targets `main` instead — which now contains #778.

## Follow-ons (separate PRs, not built here)

- Rust **codex harness** for the wave's inner agent.
- **Per-wave in-process chat server** exposing streamed `ChatTurn`s.
- **Live turn updates** in `WaveChatView` (replace the two-file reconstruction).
- Chat **input/composer** to message a running wave.

`WaveChatSource` is the single seam to swap when the server lands.
