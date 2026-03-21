---
asana_id: '1213717741038313'
linear_id: 9147c3a0-8f29-4d0c-b16f-d0ac3a5e72e5
---
# 08: Notion client — validation

```bash
cargo test -p loopflow notion     # all Notion client + blocks tests pass
cargo test -p loopflow pm         # existing PM tests unaffected
cargo clippy -p loopflow -- -D warnings
cargo fmt --check
uv run pytest python/tests/
```

Live auth path:

```bash
export NOTION_CLIENT_ID=...
export NOTION_CLIENT_SECRET=...
lfq auth notion
lf ops auth notion
```

PM ops against Notion:

```yaml
# .lf/config.yaml
pm:
  provider: notion
notion:
  title_property: Name
  status_property: Status
  done_value: Done
  priority_property: Priority
```

Then run `lf ops pm init`, `lf ops pm pull <wave>`, or `lf ops pm status` against a Notion-backed wave.
