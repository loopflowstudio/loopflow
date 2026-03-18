## Try it!

```bash
lf ops auth configure linear
lf ops auth asana

cat >> .lf/config.yaml <<'YAML'
pm:
  rw_provider: linear
  export_providers:
    - asana
YAML

lf ops pm init --wave pm
lf ops pm status --wave pm
```

What to look for:
- `pm init` links existing roadmap items by provider ID or normalized title, creates missing local/remote items, and writes `linear_id` / `asana_id` frontmatter plus wave project IDs.
- `pm status` reports per-provider local totals, linked totals, remote totals, and remote-only counts.
- PR-oriented wave runs now import from the read/write provider at run start and export back to the configured providers at run completion.

Validation run on this branch:
- `cargo clippy -- -D warnings`
- `cargo test --all` (`845 passed; 0 failed; 2 ignored` in the main Rust test binary, plus the cargo integration/bin/doc suites)
