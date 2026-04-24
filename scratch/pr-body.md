## Try it!

```bash
cargo run -q -p loopflow --bin lf -- --list | sed -n '/^Gstack$/,/^Ops$/p'
sed -n '78,92p' README.md
sed -n '31,46p' docs/lf.md
```

You should see slash-style namespaced entries such as `gstack/office-hours`, `gstack/pr-review`, and `gstack/sprint` in the CLI catalog, plus matching slash-based examples in the repo docs.

Validation run on this branch:

```bash
cargo fmt --check
cargo clippy -- -D warnings
cargo test --all
```

## Intent

Make gstack a real built-in catalog instead of a checked-in `.lf/steps/gstack/` snapshot, simplify external skill discovery around built-in namespaces plus live `npx` fetches, and make the user-facing docs describe the same slash-style names the CLI now exposes.

## Assumptions

- Slash-style namespaced commands (`gstack/office-hours`, `npx/vercel-labs/deep-research`) are the intended CLI contract.
- Repo-local and user-global overrides should still win over builtins when they exist.
- `npx` is the preferred escape hatch for third-party skill libraries; the older configurable skill-source surface is intentionally being retired, while the existing `rams/rams` alias remains as a compatibility shim.

## Key decisions

- Embedded the gstack prompt corpus and flows under `rust/loopflow/src/engine/builtins/gstack/` so build/list/catalog/discovery all share one source of truth.
- Generated builtin step/flow descriptions from embedded content instead of maintaining a second description table.
- Kept bare-name fallback only for unambiguous namespaced builtins (`office-hours`), documented slash syntax explicitly in `README.md`, `docs/lf.md`, and `docs/config.md`, and called out `rams/rams` as the lone legacy compatibility alias.
- Added a maintainer-facing `refresh-gstack` step to re-sync the bundled prompt corpus from upstream.

## Not included

- A full rewrite of bundled gstack prompt bodies that still mention legacy `gstack:` commands
- Compatibility shims for removed `skill_sources`, `superpowers`, or `SkillRegistry` configuration
- Any behavior change outside step/flow discovery, listing, catalog metadata, and the touched docs
