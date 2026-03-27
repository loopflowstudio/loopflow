# PM-native ingest review

## Validation

```bash
cargo fmt
cargo test -p loopflow ingest -- --nocapture
cargo test -p loopflow --test golden_prompt -- --nocapture
cargo clippy -p loopflow -- -D warnings
```

Manual live-provider verification was not run in this headless pass.
