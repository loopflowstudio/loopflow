# Asana Client

## Validate

```bash
cargo test -p loopflow pm::asana  # unit tests pass with mock server
cargo clippy -- -D warnings       # no warnings
```

Manual review against a real Asana workspace should cover:

- `create_project` fails clearly when `asana.workspace` is missing
- `list_items` paginates across multiple pages
- `create_item`, `update_item`, `complete_item`, and `comment` behave correctly with plain-text notes
