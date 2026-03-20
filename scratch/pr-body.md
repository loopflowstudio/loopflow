## Try it!

Review this branch in subsystem clusters first:

```bash
git log --oneline main..HEAD | sed -n '1,20p'
```

Run the core validations that are green on this branch:

```bash
cargo fmt --check
cargo clippy -- -D warnings
cargo test --all
uv run pytest python/tests/ -q
tests/e2e/test_smoke.sh
uv run pytest tests/e2e/test_api_smoke.py tests/e2e/test_concurrent_clients.py -v
swift test --package-path swift
cargo test -p loopflow --test wave_worktree_tests wave_rename_renames_branch -- --exact
```

What you should see:
- Rust/Python/e2e validation stays green.
- The targeted worktree regression test now passes because branch renames happen from the linked worktree.
- Swift package tests pass.

If you want to reproduce the only red local check, run:

```bash
cd swift
xcodegen generate
xcodebuild test -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS' CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO
```

Locally, that still fails with `ConcertoUITests-Runner ... Early unexpected exit ... signal kill before establishing connection`.
