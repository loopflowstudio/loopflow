## Try it!

```bash
cargo test -p loopflow notion
cargo test -p loopflow pm
cargo clippy -p loopflow -- -D warnings
uv run pytest python/tests/
```

To exercise the live auth path:

```bash
export NOTION_CLIENT_ID=...
export NOTION_CLIENT_SECRET=...
lfq auth notion
lf ops auth notion
```

To point PM ops at Notion:

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

Then run `lf ops pm init`, `lf ops pm pull <wave>`, or `lf ops pm status` against a Notion-backed wave. You should see Notion databases/pages participate in the same PM lifecycle as Asana and Linear, with page bodies round-tripping as markdown.
