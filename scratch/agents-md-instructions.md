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
- Remove obsolete guide references and filename-specific rendering tests;
  preserve generic document rendering and native-exclusion coverage.
- Retain native instruction detection and canonical-path deduplication: Loopflow
  operates in other repositories where either native filename can be symlinked.

## Remaining acceptance

- CI: complete the Rust suite, including the process-cleanup test described
  below. Local gate did not establish a full Rust pass.

## Native loading acceptance (2026-09-30)

Fresh headless sessions launched through `lf -b -m claude` and `lf -b -m codex`
in this checkout identified AGENTS.md and correctly reported wire DTO defaults,
Python import placement, Rust keyword naming, and conversion-method conventions.
Both used `--no-loopflow --no-diff --no-diff-files --max-turns 1 :` with an
inline probe that prohibited tools, file reads, and edits. The probe supplied
questions, not the answers or guide contents. Neither session called tools.
Installed provider versions were Claude Code 2.1.286 and Codex CLI 0.159.0.

## Review findings

The architecture scanner explicitly named the former guide, so its scan root
moves with the file. Context documentation incorrectly described native
instructions as injected files; it now identifies the provider as their owner.
Generic native-file detection remains unchanged, with regressions covering the
regular file and symlinks in either direction.

Compression removes the remaining filename-specific rendering test: the generic
document test covers rendering, while the native-exclusion test proves the actual
launch behavior. Symlink cases name their target and link directly.

Gate review found no further changes needed in the branch. AGENTS.md is a
regular file, byte-identical to the base guide; remaining obsolete guide-name
strings are captured history. The two remaining alternate native-filename
references implement generic provider support.

The affected-suite runner stopped Rust after macOS `syspolicyd` sustained 232%
CPU for three samples. Before that stop, the unchanged
`harness::opencode::tests::kill_process_group_reaps_live_parent_and_provider_child`
failed with “provider-child descendant outlived the group kill.” The same
compiled test passed in isolation; its cause remains unresolved. Preserve the
original failure rather than treating the isolated pass as a green suite.
Rust recorded 1,764 passes, one assertion failure, four signal-aborted tests,
255 unrun tests, and 17 skips. Native-exclusion tests and the builtin prompt
golden passed. CI owns the interrupted remainder and the cleanup regression.
Detailed local output is under `.lf/tmp/gate/run-6011/`.

Checks: `uv run python scripts/test.py --base de074a2ebd2cb54e6f7dde799400dae36c87bbbe --reuse-passing` (inherited `LF_*`/`LOOPFLOW_*` cleared, `LF_BIN` pinned to this checkout's compiled CLI) — architecture, Python (296 passed/19 skipped), formatting, Clippy, website (78 passed/3 skipped) passed; Rust incomplete as above, deferred to CI; `target/debug/deps/loopflow-75d1a0b5e21206ba --exact harness::opencode::tests::kill_process_group_reaps_live_parent_and_provider_child --nocapture` passed (1); fresh `lf` Claude/Codex loading probes and `git diff --check` passed.
