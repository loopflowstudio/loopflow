# Branch review — jack-heart.gstack-debug.20260424_1001

## What was implemented

Bundled the imported gstack catalog into loopflow's built-in step/flow system, simplified external-skill discovery down to `npx/` (plus `rams`), and documented slash-style namespaced invocation across the repo's user-facing docs.

Concretely, the branch:
- moved the generated gstack steps from repo-local `.lf/steps/gstack/` into `rust/loopflow/src/engine/builtins/gstack/`
- added built-in gstack flows (`gstack/plan-manual`, `gstack/review`, `gstack/sprint`)
- taught builtin discovery/listing/catalog code to expose namespaced builtins, descriptions, and bare-name fallback for unique names like `office-hours`
- removed the old configurable `skill_sources` / `superpowers` / `SkillRegistry` paths in favor of built-in namespaces plus live `npx/<owner>/<repo>` fetches
- aligned `README.md`, `docs/lf.md`, and `docs/config.md` with the slash-based namespaced syntax now shown by the CLI

## Key choices

- **Bundle gstack in Rust builtins, not `.lf/steps/`.** That makes `gstack/*` part of the shipped catalog, so `lf --list`, catalog APIs, tests, and releases all see the same source of truth.
- **Use slash namespaces everywhere.** The branch moves user-facing examples to `gstack/office-hours` and `npx/vercel-labs/deep-research`, matching the current discovery/listing behavior and keeping namespaced steps/flows visually distinct from `step: args` syntax.
- **Keep `npx` as the only general external skill channel.** The old configurable skill-source surface added config drift and duplicate discovery paths. The new model is: local overrides in `.lf/steps/`, builtins in the binary, everything else through `npx`.
- **Auto-generate builtin descriptions from prompt content.** `lf --list` and the HTTP catalog now stay in sync with the bundled prompt files instead of depending on a separate hand-maintained description table.

## How it fits together

`build.rs` now embeds the gstack step and flow files as builtins. Runtime loading goes through the same engine path as core builtins: repo overrides first, then bundled builtins, with bare-name fallback when a namespaced builtin is unique. Listing and catalog endpoints read descriptions directly from the embedded prompt/flow content, so the CLI and UI expose the same namespaced catalog the engine resolves.

## Risks and bottlenecks

- **Prompt-body drift:** many bundled gstack prompt bodies still mention legacy `gstack:` commands even though the top-level docs and current working tree treat namespaced invocations as slash-based. That is the biggest reviewer-facing mismatch left.
- **Upstream sync drift:** `refresh-gstack` updates the bundled prompt corpus, but gstack flow composition still needs human review when upstream renames or reshapes steps.
- **Name-compatibility risk:** removing or tightening legacy colon handling is user-visible. Reviewers should confirm the intended CLI contract before landing.

## What's not included

- Bulk rewriting the imported gstack prompt corpus to replace every legacy `gstack:` command reference
- A backwards-compatibility shim or migration layer for removed `skill_sources` / `superpowers` / `SkillRegistry` config
- Any changes to Python/Swift docs beyond the top-level repo docs touched here

## Validation

- `cargo fmt --check`
- `cargo clippy -- -D warnings`
- `cargo test --all`
- `cargo run -q -p loopflow --bin lf -- --list` (manual spot-check of the exposed gstack catalog)
