---
extends: builtin
---
## In this repository

### Focused tests

Run from the checkout root with inherited `LF_*` unset and `< /dev/null` —
inline in zsh: `env $(env | grep '^LF_' | cut -d= -f1 | sed 's/^/-u /') <cmd> < /dev/null`.

```bash
cargo test -p loopflow-execution --lib skills::catalog::          # one library module
cargo test -p loopflow-harness claude_decode                      # one decoder or transcript reader
cargo test -p loopflow --test integration context_tests::         # one integration module
cargo test -p loopflow --test golden_prompt                       # one standalone binary
uv run pytest python/tests/test_gate_bounded.py                   # one Python file
```

Never `cargo test`, `cargo test --all` or `cargo nextest run --all` here.

### Crates and surfaces

- `rust/loopflow-model` — shared domain/ID/status types (wire-facing; changes reach Swift)
- `rust/loopflow-storage` — SQLite store, migrations, `SCHEMA.md`
- `rust/loopflow-harness` — provider decoders, transcripts (builds no storage or execution)
- `rust/loopflow-execution` — launch, skills, shipping, wire/transport types
- `rust/loopflow` — CLI, server (`src/server/`), integration tests
- `swift/Loopflow` — shared Swift library (DTOs, server client, themes, shared views used by Mac and iOS)
- `swift/LoopflowMac` — macOS Desktop shell and GhosttyKit integration
- `swift/LoopflowiOS` — iPhone app target (`scripts/test_ios.py`)
- `website/` — site and generated CLI reference

**DTO parity.** A wire field change (in `rust/loopflow-model/`,
`rust/loopflow/src/server/`, `wire.rs`, `read_stream.rs`, or CLI `--json`
reads/writes) updates `tests/fixtures/dto/` and Rust + Swift required-nullable
decoders (`swift/Loopflow/`), then runs
`scripts/test_desktop.sh --filter DTOFixtureTests` and
`uv run python scripts/test_ios.py`.

**iPhone parity.** A change to shared Swift views, themes, models or Task
status (`swift/Loopflow/`) is checked in the iOS target too:
`uv run python scripts/check_swift_multiplatform_boundaries.py` and
`uv run python scripts/test_ios.py`.

### UI rules

**A number on screen holds still.** Show progress until a count or total
finishes loading, then draw the final value once; never let it climb live.

**One spinner per pane.** Delay briefly on an empty pane so fast opens never
flash, then center a regular-size spinner. Over existing content, keep the body
clear and show progress in the toolbar strip.

**Plain icon buttons show hover and state.** Give an icon-only `.plain` button
a hover highlight and tooltip explaining what it does; when it toggles a view or
mode, show its on/off state clearly.

**Desktop confirmations are sheets, not `.confirmationDialog`.** On macOS,
`.confirmationDialog` with a `.destructive` button promotes Cancel to the
accented default button, painting both red. Use a `.sheet` with
`LoopflowButtonStyle`: at most one filled button (`.accent` for primary,
`.success` for completing work), neutral Cancel, and `.destructive` outline
when an action destroys work.

**Isolate modifier sheets and borderless menus.** Put a `ViewModifier`'s
`.sheet` inside `.background { Color.clear.sheet(...) }` so sibling `.sheet`
modifiers do not swallow it. Style icon-only `Menu` buttons with
`.menuStyle(.borderlessButton)` and a `HitTarget.control` frame, not
`.menuStyle(.button).buttonStyle(.plain)`.
