# Join: Infrastructure Roadmap Items

Synthesized from two forked worktrees proposing complementary CI improvements.

## Items Added

1. **ci-lint-checks** - Add ruff linting to CI workflow
2. **ci-cache-determinism** - Add caching for uv, SwiftPM, and Xcode toolchain

Both items are complementary and can be implemented independently. The lint checks provide faster feedback on code quality; the caching reduces CI runtime and flakiness.
