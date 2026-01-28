Open questions / assumptions

- Assumed Rust core is a new crate at rust/lf-core with a root Cargo workspace. If this should live elsewhere, adjust paths.
- Flow loading currently supports only YAML/JSON under `.lf/flows/` and only linear `Step` items in `tick_flow`; fork/choose/loop are parsed but not executed yet.
- `run_step` shells to `lf --step <name>` as in the design doc; if the CLI expects `lf <step>` or different flags, the runner should be updated.
