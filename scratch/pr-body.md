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
