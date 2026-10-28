# LOO-669 open questions and assumptions

LOO-656's design, migration study/history/research and its own questions live at
#1755's head `b884d9c83` (`git show b884d9c83:scratch/<file>`). Items from them
that streams depend on:

- `lf self install --main` installs the CLI only and needs `gh` auth (Actions
  artifacts are private); the `dev` stream inherits both until slice 7.
- Sources without Git history (crates.io package, container builds) treat every
  slug as unmerged unless `LOOPFLOW_MIGRATION_ORDER` names the order.

## Slice 1 (minors from main), headless implement 2026-10-28

- **A minor with no commits since the last patch still releases** when the cycle
  since the preceding `.0` has changes (the change check uses the notes base).
  This keeps "graduate the cycle" possible right after a patch, as v0.15.0 did.
- **Leftover `.lf/releases/minor-<target>.json` receipts** are ignored, not
  deleted; they are local controller files and harmless.
