# v0.12.15

<!-- loopflow:release-notes=narrative;gate=safe -->

v0.12.15 makes local execution and release operations recoverable from recorded evidence instead of ambient state. You can promote an unpublished build without disturbing the reliable published Home, compose bounded Runs around tracked Work without handing ownership to a controller, and inspect or replay a Run from its Home-local record. Release tags now attest to an exact candidate that already passed hosted validation and publisher preparation, so a failed candidate can be fixed and retried without consuming the version.

## Try local builds without risking the published Home

Local promotion now switches the CLI, daemon, and app as one artifact set while preserving the published installation as a fallback. Development data lives in a disposable Home forked from the published Home, and every persisted switch phase can recover after interruption.

- `uv run python scripts/install.py local --use` builds and promotes the local artifact set.
- Add `--fresh` to fork a new development Home; otherwise Loopflow reuses a compatible one.
- `uv run python scripts/install.py refresh` returns every surface to the published installation without importing development data.
- Development builds embed dependency-ordered draft migrations and apply only an exact append-only prefix to the disposable store.
- Promotion preflight can recover a published candidate's identity even when the release host's live Home is incompatible with that artifact.

## Share tracked Work without controller ownership

Wave, Project, and Task records now describe durable Work independently from the automation that may pursue it. Humans, bounded agents, cron jobs, and alternate orchestration can use the same execution and delivery primitives without impersonating or advancing an end-to-end controller.

- `lf task prepare` and `lf project prepare` establish tracked Work without starting controller automation.
- `--task`, `--project`, and `--wave` bind independent Runs to existing Work while preserving each Run's own provenance.
- Multiple independent Runs can concern the same Task; Work attribution is no longer a provider writer lease.
- `lf task restart` checkpoints the worktree and resets controller progress while preserving Task, worktree, and PR identity.
- Managed Tasks can follow the human-submit path without requiring a live controller, and landing collapses intermediate checkpoints before publishing the verified head.

## Inspect and replay the Run that actually happened

Provider execution now produces a Home-local Run record: an immutable manifest published before spawn, an append-only event stream, and one exclusive terminal receipt. This replaces the SQL-backed Invocation/Turn execution control plane and makes identity evidence and causality rather than a mutation lease.

- `lf runs <id>` reads the Run summary, while `lf runs <id> --events` exposes its recorded events.
- `lf replay <id>` launches a new child Run through the ordinary harness from the recorded execution contract and preserves the source relationship.
- Run evidence records the effective prompts, agent and model, stable non-secret account identity, turn cap, browser capability, permission mode, filesystem boundary, provider output, conversation, retries, and provider-authored cumulative usage.
- Replay resolves the exact recorded account on the selected Home and fails closed when the schema, account, or managed-Run evidence cannot support the launch; it does not record credential bytes, environment values, resume tokens, callbacks, repository contents, or process ownership as replay inputs.
- `lf trace --events` and `lf doctor` now understand the measured historical usage shapes without accepting unknown or partial records as valid current data.
- Ended Runs from retired CI controllers remain readable with their unknown trigger JSON preserved byte-for-byte, while unknown nonterminal triggers remain invalid and cannot reserve new Runs.

## Tag only candidates that have already passed

The release tag is now the attestation, not the starting gun. Loopflow builds from a provisional exact-commit candidate ref, verifies the workflow by branch and head SHA, prepares publication artifacts, and only then pushes the immutable version tag.

- Nightly and release candidates share one credential-free package matrix, with candidate binaries reporting their intended final version before the tag exists.
- Publisher preparation stores artifacts and receipts under `.lf/releases/`, binding the candidate tag, source commit, workflow run, completed proof stages, and artifact hashes.
- Publication revalidates the receipt and every artifact before upload, then removes successful candidate state.
- A failed build leaves the repository untagged and preserves its candidate evidence, so the same version can be retried after a fix merges.
- Tag pushes no longer trigger release builds; hosted builds use the explicit candidate-dispatch contract, while credentialed signing and publication remain on the maintained publisher host.

## Operational notes

- Work-bound Runs share a worktree but are not file-edit transactions. Give concurrent contributions distinct paths and reconcile them before checkpointing.
- Replay uses current credentials and current repository contents as live launch inputs even though the execution contract itself is immutable. Exact-account replay refuses rather than silently choosing another identity.
- Local promotion retains the published fallback and never imports development data during `refresh`; use `--fresh` when a clean fork is required.

## Small changes

- Cron continuity now checks the latest due local-time interval for an exactly matching scheduled receipt. Historical ledger gaps remain visible without permanently blocking `lf doctor` or the telemetry scorecard.
- Scheduled receipts prove the scheduler fired even when the target failed; manual receipts do not satisfy continuity.
- Wave memory now retains the release-recovery evidence restored during main reconciliation, and the list Wave starts from its current Linear-backed context.