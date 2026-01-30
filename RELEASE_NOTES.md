# v0.7.1

A polish release for the Rust engine and Concerto app. The daemon gains Rust-powered git operations for more reliable rebases and pushes, while Concerto now sends local notifications when waves need attention, fail, or have PRs ready for review.

## Changes

- Concerto notifies you when a wave needs input, fails, or opens a PR—no more polling the sidebar
- Cancel and continue buttons for interactive sessions let you bail out or resume mid-flow
- The `lf ops land` command now uses Rust git operations for rebasing and pushing, making merges more reliable
- Unified step execution removes the internal work queue—waves run more predictably
- Git ops consolidated under `lf ops` namespace with new Rust-backed rebase/push primitives
