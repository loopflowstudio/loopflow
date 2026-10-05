# Reconciliation assumptions — October 4, 2026

No unresolved product judgment blocks implementation. Jack Heart approved the
workflow design; its [implementation defaults](focus-on-your-own-work.md) remain
reversible: 120-second Waiting fallback, most-recent interactive Task primary,
optional occurrence names, backward-target loops and source-only workflow YAML.
These are implementation choices, not Jack's exact spelling/timing preferences.

The saved-capture bridge preserves the existing invocation; live migration and
configured acceptance remain separate from implementation.

Superseded questions and proposals: `5090f672e:scratch/questions.md`.

Context query: memory and scratch fit. The assembled Task seed was 16,439/16,000
tokens (439 over before trimming; 12,187 submitted). Stored steers and the
branch patch contribute to that seed; this pass changed no external history or limits.

Implementation interpretation: the existing per-Task automation on/off field also
holds CI repair. Preserve that setting and its delivery UI while deleting Flow
scheduling and its counters. This preserves unrelated CI policy; it provides no
Flow restart authority. Initial Swift dependency resolution failed under the
headless runner's file-only transport; ordinary package resolution repaired setup.

Sync check: `cargo test -p loopflow --lib completed_session_with_exited_provider_does_not_block_task_work` passed (1 test); retained main's confirmed-dead completed-provider exemption without restoring managed recovery.
