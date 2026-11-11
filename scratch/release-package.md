# Source package repair

v0.15.3 candidate workflow 42964959280 passed for commit
0766e9cbfd5e01f1c244f0fb3cdecfe8aa6fad9c. Signing and notarization passed.
Publication then failed in `cargo publish` archive verification with 68 missing
server example files. GitHub release v0.15.3 remains a draft; the tag already
exists. The installed CLI is 0.15.2+922830b0b and includes the terminal fix.

Move the shared server examples into the crate, retaining one canonical copy.
Verify the source archive in the candidate workflow before tagging, and use
the same locked dependencies when publishing.

The tagged release currently resumes its original commit. This source repair
does not move that tag or alter the retained signed artifacts. Replacing a
tagged draft needs a deliberate recovery path before publication can resume.

Checks: cargo package --locked --allow-dirty -p loopflow passed (887 files; extracted archive compiled).
Checks: cargo test -p loopflow --lib server:: passed (59; 2 ignored).
Checks: swift test --filter ServerProtocolTests passed (17).
Checks: pytest release_publisher and release_automation passed (54).
Checks: cargo fmt, cargo clippy --all-targets -- -D warnings, architecture and git diff --check passed.
