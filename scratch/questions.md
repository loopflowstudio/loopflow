# Questions / Blockers

- Local Loopflow UI validation is blocked by the Xcode UI runner in this headless gate environment. `xcodebuild test -project LoopflowSwift.xcodeproj -scheme LoopflowMac -destination 'platform=macOS' ...` reports 304 passing app tests, then fails because `LoopflowUITests-Runner` hangs before establishing a connection. A fresh DerivedData rerun reproduced the same runner hang.

- `lf op pm doctor` and `lf op pm sync --plan` are byte-for-byte identical (both call `pm_sync` with `plan: true`, same output). `doctor` earns its place only as a memorable read-only verb; if that UX value isn't wanted, drop `doctor` and keep `sync --plan`. Left in place — removing a command is a product-surface call, not a mechanical reduction. Same note applies to the `update` command as a compat alias for the `task create/update/done` subcommands (deliberate, per the design doc).
