## Try it!

```bash
cargo test --all
cargo test -p loopflow docker_
uv run pytest python/tests/
tests/e2e/test_smoke.sh
uv run pytest tests/e2e/test_api_smoke.py tests/e2e/test_concurrent_clients.py -v
swift test --package-path swift

sed -n '1,120p' docs/lfd.md
sed -n '1,120p' deploy/README.md
rg -n 'LFD_AUTH_MODE|LFD_AUTH_PROVIDER|mode: container' docker/docker-compose.yml deploy/docker-compose.prod.yml docs/getting-started.md docs/lfd.md
```

What you'll see:

- `cargo test --all` now passes without depending on your real `~/.lf/config.yaml`
- the daemon/docs/compose story is now `native|container` for deployment and `local|studio` for auth
- the default container recipe points at studio auth + Docker without asking operators to pick from old profile names or unsupported install flags

_Local note:_ local `xcodebuild test` still hit a `ConcertoUITests-Runner` bootstrap crash after the XCTest suites passed, so UI confidence here still leans on CI.

## Intent

Collapse `lfd` onto the two deployment shapes and two auth modes the product actually wants to support, then make code, docs, compose defaults, and Concerto all tell that same story. The branch removes leftover `static`/`ci`/manual-token language, keeps container mode honest about its real entrypoint, and hardens the test suite so gate results are not contaminated by a developer's existing loopflow config.

## Assumptions

- Existing callers can migrate from `auth.provider`/`LFD_AUTH_PROVIDER` to `auth.mode`/`LFD_AUTH_MODE`
- Container deployments should default to studio auth and Docker unless operators intentionally use an escape hatch from the config reference
- Regressions in local macOS UI-test bootstrapping, if any, will be caught by CI because local Swift package tests already pass

## Key decisions

- reject old auth keys/aliases instead of carrying compatibility shims
- use the session-token file as the single local credential story
- keep `mode: container` in config as the real persistent installation path
- remove manual iOS connection setup rather than preserve a path that no longer matches the product auth model
- isolate `HOME` in Rust tests so repo validation is deterministic

## Not included

- team auth or any new shared-auth product surface
- a final product decision on sandbox executor support
- install-time CLI sugar that writes `mode: container` for the user
- a migration layer for deprecated auth names
