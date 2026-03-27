---
asana_id: '1213751585305659'
linear_id: 8d48e039-250f-4156-9c8f-b8a87aa8bff0
notion_id: 32af8f99-3d81-8121-b592-d35ee90a848d
---
# Ingest becomes PM-native

## Validation

```bash
cargo test -p loopflow ingest -- --nocapture
cargo test -p loopflow --test golden_prompt -- --nocapture
cargo clippy -p loopflow -- -D warnings
```

### Manual verification

On a wave with a `pm` block:

```bash
lf ops ingest --wave <wave-name>
```

- A new item added remotely appears as the next pick
- A reprioritized item changes the pick order
- A deleted remote item is no longer eligible
- If the provider is unreachable, ingest warns and picks from local files
