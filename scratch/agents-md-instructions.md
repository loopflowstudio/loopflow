# One repository instruction file

Accepted scope: Jack Heart's LOO-363 directive, 2026-09-30.

AGENTS.md becomes the regular repository instruction file, preserving the existing
guide's content. Skills, documentation, fixtures, and the architecture scanner
refer to that owner. Loopflow continues excluding provider-native instructions
from injected context.

The [Claude Code memory documentation](https://code.claude.com/docs/en/memory#agentsmd)
confirms native loading from v2.1.277, subject to project instruction settings
and absence of competing ancestor instruction files. This supports the file
change; it does not substitute for the fresh-session acceptance proof.

## Delete — do not maintain

- Delete the separate style-guide file and both root instruction symlinks;
  preserve the guide as a regular AGENTS.md.
- Remove obsolete guide references and the redundant single-document style
  rendering test; preserve generic document rendering coverage.
- Retain native instruction detection and canonical-path deduplication: Loopflow
  operates in other repositories where either native filename can be symlinked.

## Remaining acceptance

- Gate: fresh Claude Code and Codex sessions in this repository demonstrate
  native AGENTS.md loading without manually reading or injecting the guide.
- Gate: affected suites, including the builtin prompt golden. Implementation
  runs focused prompt tests, the architecture scanner, formatting, and Clippy.

## Review findings

The architecture scanner explicitly named the former guide, so its scan root
moves with the file. Context documentation incorrectly described native
instructions as injected files; it now identifies the provider as their owner.
Generic native-file detection remains unchanged, with regressions covering the
regular file and symlinks in either direction.

Checks: `cargo test -p loopflow --lib engine::prompt::tests -- --test-threads=4` passed (82); `uv run python scripts/check_architecture.py`, `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, `git diff --check`, guide-byte comparison and live tracked-reference scan passed; fresh provider sessions and golden/affected suites belong to gate.
