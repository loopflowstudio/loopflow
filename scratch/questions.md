# Open Questions

## 2026-04-24 — Bundled gstack prompt bodies still mention legacy `gstack:` commands

Current working tree behavior and the top-level docs now treat namespaced steps as
slash-only (`gstack/office-hours`, `npx/vercel-labs/deep-research`). Many
imported prompt bodies under `rust/loopflow/src/engine/builtins/gstack/step/`
still mention the older colon form because they were converted from upstream
content verbatim.

**Assumption:** This documentation pass updates the repo's user-facing docs
(`README.md`, `docs/`) and review artifacts only. It does not rewrite the full
bundled gstack prompt corpus.

**Follow-up:** If slash-only is the shipped CLI contract, bulk-update or
re-convert the bundled gstack prompt bodies before landing so in-product
instructions match the CLI.
