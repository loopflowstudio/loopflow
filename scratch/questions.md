# Open questions

- Assumed built-in direction group mappings for shorthand names introduced on this branch:
  - `infra` → `infra-engineer`
  - `ux` → `designer`
  - `values` → `clarity`, `simplicity`, `craft`, `flow`, `scale`
  If a different mapping is desired (for example `ux` including `product-engineer`), update `engine/builtins.rs`.

- Local macOS `xcodebuild test` currently fails linking `ConcertoUITests` with:
  `open() failed, errno=1` for the generated test binary under DerivedData.
  Needs confirmation whether this is a local environment issue vs. a project-level UI test target problem.
