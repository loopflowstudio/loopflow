## Try it!

```bash
cargo test -p loopflow ingest -- --nocapture
cargo test -p loopflow --test golden_prompt -- --nocapture
cargo clippy -p loopflow -- -D warnings
```

You should see two new ingest behaviors covered end-to-end:
- PM-backed waves pull fresh provider state before item selection.
- A failed PM pull emits a warning and falls back to the existing local `wave/<name>/` mirror.

To poke at the user-facing path manually, configure a PM-backed wave and run:

```bash
lf ops ingest --wave <wave-name>
```

On a healthy provider connection, ingest now refreshes before it moves the selected item into `scratch/`. If the pull fails, ingest warns and still picks from the local mirror.

## Intent

Make `lf ops ingest` match the PM-native product model. For PM-backed waves, the tracker is the source of truth, so ingest should refresh that state itself before it decides what work to pick instead of depending on flows to remember a separate `pm pull` step.

## Assumptions

- `wave_pm_is_enabled()` correctly identifies waves that should refresh from a PM provider.
- `pm_pull()` is safe to call redundantly for PM-backed flows.
- Falling back to the local mirror is preferable to blocking ingest when PM credentials or network access are unavailable.

## Key decisions

- Put the refresh in `ingest()` rather than only in flows so manual ingest and future flows stay correct by default.
- Use the main repo root for the PM pull so worktree runs still refresh the canonical `wave/<name>/` directory.
- Log PM refresh failures as warnings and continue with local files.
- Update README, wave authoring docs, and the built-in ingest prompt so the new behavior is discoverable.

## Not included

- No opt-out flag for PM refresh.
- No deduplication of the extra `pm pull` some flows already do.
- No live provider smoke test in this pass.
