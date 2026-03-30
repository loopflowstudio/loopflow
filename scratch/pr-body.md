## Try it!

```bash
cargo test -p loopflow asana_html
cargo test -p loopflow asana
cargo fmt --check
cargo clippy -- -D warnings
cargo test --all
```

What to look for:
- `cargo test -p loopflow asana_html` exercises the markdown ↔ Asana HTML converter for headings, lists, bold, italics, code, strikethrough, links, escaping, and `<hr>`.
- `cargo test -p loopflow asana` proves task reads prefer `html_notes`, older tasks still fall back to plaintext `notes`, and create/update requests now send `html_notes` instead of `notes`.
- `cargo test --all` confirms the change stays within the existing PM and release workflows without regressions.
