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

## Intent

Turn PM integration on with minimal setup decisions. This change reshapes PM config around provider roles, adds a low-decision bootstrap command, and moves the important sync behavior into the normal wave lifecycle so Linear can be the canonical source while Asana receives mirrored updates automatically.

## Assumptions

- Linear is the single read/write PM provider for the repo.
- Export providers should never drive local roadmap state.
- Wave-level import/export is the valuable v1 lifecycle behavior; item-level PR comments/completion can wait until runs keep stable roadmap-item identity after ingest.
- Reviewers testing the live flow have valid Linear/Asana credentials configured locally.

## Key decisions

- Use `pm.rw_provider` plus `pm.export_providers` instead of `pm.provider`.
- Keep `lf ops pm init` conservative: match by existing ID, then normalized title, create what is missing, and avoid destructive deletion/replacement during bootstrap.
- Trigger PM import/export only for PR-oriented runs so repair flows on an existing PR branch do not add duplicate PM churn.
- Preserve local markdown bodies when bootstrap matches an existing read/write item; gate fixed an overwrite bug and a remote-rank filename off-by-one bug here.

## Not included

- Import from export-only providers.
- Destructive multi-source reconciliation.
- Item-level PR-open / PR-merge / failure comments back to PM.
