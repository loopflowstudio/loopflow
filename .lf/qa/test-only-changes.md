Read when the change moves or cuts tests.

# Test-only changes

When the change moves or cuts tests, the people it is for are developers
running them. Run each filter exactly as `rust/loopflow/tests/README.md`
prints it, with `LF_*`/`REX_*` unset and stdin closed. Then break the
production decision on purpose: one `sed` on the line the rows claim to
check, run that family's filter, `git checkout -- <file>`. Do one for each
family, plus one Clap → ops wiring break (e.g. drop `worktree` in
`lf/commands/task_create.rs`) run against the kept full-Machine filter.
A break no test notices is a missing row; add it, then repeat the break.
Edit only from one shell, one cargo command at a time, so no other build
compiles the mutated source.

For a Swift-only test change, skip the wrapper's Rust rebuild: run
`swift build --build-tests --package-path swift --jobs 4`, then
`scripts/test_desktop.sh --skip-build --filter <Suite>/<test>`. Rebuild after
`git checkout` restores a mutation, or the next `--skip-build` run tests the
mutated binary. A break that leaves an unbounded Observation wait pending
hangs until Swift Testing's 300-second limit stops the run; run that break
in the foreground so its exit status reaches you.

A Swift "passes under parallel load" check can't use two `swift test`
processes: the second waits on the `.build` lock ("Another instance of
SwiftPM … waiting"), even with `--skip-build`, so the runs never overlap.
Build once, then run the bundle directly for one side:
`swiftpm-testing-helper --test-bundle-path
swift/.build/debug/LoopflowSwiftPackageTests.xctest/Contents/MacOS/LoopflowSwiftPackageTests
--testing-library swift-testing --parallel [--filter …]` from `swift/`, with
`DYLD_FRAMEWORK_PATH` set to the MacOSX platform's `Library/Frameworks` and
`Library/PrivateFrameworks`, and `DYLD_LIBRARY_PATH` to its `usr/lib`. The
helper is in the Xcode toolchain's `usr/libexec/swift/pm/`. Timestamp each
run to show the overlap.

A fixture that runs `lf` sees whichever build is at `target/debug/lf`. To
check a test on its first run after a link, `touch rust/loopflow/src/bin/lf.rs`,
build with `cargo test … --no-run`, and hash `target/debug/lf` before and
after the run with `LOOPFLOW_TEST_LF` unset: a different hash means a test
process rebuilt `lf` under a running server.
