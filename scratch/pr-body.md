## Try it!

```bash
cargo test -p loopflow asana_html
cargo test -p loopflow asana
cargo clippy -p loopflow -- -D warnings
```

What to look for:
- `asana_html` round-trip tests prove headings, lists, bold, italics, code, strikethrough, links, and `<hr>` survive markdown ↔ Asana HTML conversion.
- `asana` client tests prove task reads prefer `html_notes`, older tasks still fall back to plaintext `notes`, and create/update requests now send `html_notes` instead of `notes`.

Validation from this branch:
- `cargo fmt --check` ✅
- `cargo test -p loopflow asana_html` ✅ (6 passed)
- `cargo test -p loopflow asana` ✅ (20 passed)
- `cargo clippy -p loopflow -- -D warnings` ✅
