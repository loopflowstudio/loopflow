path: done

The embedded terminal build driver is ship-ready. All CI-enforced checks pass:
`cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test -p loopflow`,
`uv run pytest python/tests/test_dto_fixtures.py`, `swift test --package-path swift`
(335 tests), and `scripts/verify_embedded_build_driver.py --skip-build` (palette
session launched, completed via exit file, stayed attachable after flow exit).

The implementation matches the reshaped design: zero new `TerminalSession`
fields, one new wire type (`CreateTerminalSessionRequestDto`), `source ==
"palette"` lifecycle discriminator, Swift panes rewired to lfd session ids and
the existing attach RPC. DTO discipline holds — new wire types have no defaults.
Dead code (`attachCommand`, `launchCommand`, launchpad prompt path) is deleted,
not commented out.

One hygiene issue found and fixed in this pass: a prior run committed two
runtime `.exit` files under `.lf/tmp/terminal-sessions/`. Removed them and added
`.lf/tmp/` to `.gitignore` so palette session exit files never get tracked.
Review doc and PR body updated to record it.

No outstanding work. Ship.
