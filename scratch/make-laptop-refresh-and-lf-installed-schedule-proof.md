# Published installation and preserving checkout updates

## Accepted contract

The September 24 Task brief supersedes the original combined refresh design.
`lf install` updates published machine artifacts independently of checkout; it
must not update main, run Homebrew/uv, or introduce `lf refresh`. `lf rebase`
fetches current upstream on every invocation, refreshes canonical main, and
integrates an ordinary caller worktree even when main was already current.
Local commits and staged, working and untracked bytes survive; unpublished main
commits remain valid sibling bases under LOO-257's separate placement contract.

Scheduling is opt-in `lf install schedule [weekly|daily|hourly|5min]`, default
login plus Monday 09:00 local. Calendar intervals preserve sleep catch-up;
retain the launchd label and LF_INSTALL_DIR without source cwd or Python.
Positional cadence replaces the proposed --every flag. A separate daemon is
retired in the published installation and is no longer an artifact requirement.

Jack Heart's October 2 steer requests headless proof and reconciliation in the
existing Task checkout, retaining the authored demo review. No production
release, branch-binary installation, replacement worktree, or Task completion
is authorized by initial schedule success. Interactive app acceptance remains
Jack's judgment.

## Evidence and remaining work

- The September 23 [demo](demo-laptop-refresh.md) records actual candidate main
  catch-up, repeat and sibling creation. Its 43 focused passes cover advancing
  upstream, dirty/unpublished preservation, feature integration and failure/retry
  using local remotes. These remain dated candidate/fixture evidence, not a
  current installed replay. Package participation is historical, no longer required.
- The September 24 [incident](install-incident-20260924.json) reproduces the
  retained v0.12.19 candidate's empty preflight and child-preview EOF, with
  unchanged active receipt. The [causal analysis](5whys-install-preflight.md)
  remains historical. The original design and review handoff are retained at
  `4295ef22ce204f3edf3fc8d8916721a91318b7b4:scratch/make-laptop-refresh-and-lf-installed-schedule-proof.md`.
- The October 2 [operator proof](operator-install-proof-20261002.md) establishes
  installed repeat/no-op, weekly activation, identical plist on repeat and
  successful initial launchd execution. Activation is complete. Later login,
  scheduled firing and wake catch-up remain unproven.
- The updated [installed demo](demo-published-install-20261002.md) establishes
  published 0.12.31 selection, exact schema frontier, compatible executable
  references, matching CLI/app/helper hashes and valid app signature. Jack's
  latest steer reports successful published installation and main-Home migration;
  this gate has no before/after migration inventory and does not invent one.
- Released-path fresh-account bootstrap and repair still need public-channel
  proof. Existing Ubuntu candidate results used local HTTPS and materialized
  migrations; the failed published 0.12.20 bootstrap is contrary historical
  evidence. A disposable OS account/container is required where machine authority
  uses account identity; changing HOME is insufficient. Never damage production
  artifacts to manufacture a repair demonstration.
- Retained-candidate prevention proof still needs a traceable released test
  receipt: actual direct and child preflight, incompatible-store rejection JSON,
  ordinary inactive-byte refusal, and unchanged stores/receipt. Successful active
  preflight alone does not establish those retained-development conditions.

## Remaining operator proof versus review

The operator can collect a naturally occurring login, Monday firing or wake
receipt, correlated with installer output and actual artifact outcomes. A manual
kickstart proves only manual execution. No forced sleep/logout is needed.
Explicit published install after a missed/failed opportunity must retain artifact
and populated-history baselines; the reported main-Home upgrade is useful evidence
but not a complete record-by-record preservation audit.

The installed checkout demonstration still needs evidence covering repeated main
updates after upstream advances and a stale feature caller with both stale and
current main, followed by sibling creation through lf. Preserve original fixture
proof for dirty/index/unpublished conditions; no origin-equality requirement.
No checkout implementation changed in this evidence-only serial PR.

Jack's authored demo review remains outstanding. App interaction and acceptance
cannot be inferred from codesign, JSON, operator installation, or initial job load.
The Task remains open. The current Task read identifies gate running; the earlier
legacy restart failure is dated history, not the current blocking diagnosis.

## Gate assessment

Review found stale active instructions to implement the old repair, reinstall
packages, activate an absent job, and wait for v0.12.19 publication. Reconciled
those instructions against dated evidence and the accepted command split; no
new implementation failure was exposed. Full release/installer suites are not
claimed for this documentation-only branch. Scratch is intentionally retained
for the authored review; CI scratch-clear remains a later delivery requirement.

Check: `git diff --check` and local scratch links PASS; `uv run python scripts/test.py --base c56340a142bcb6a854b98fed9b757cd9a6423f9a --reuse-passing` stopped at the 60s resource-measurement timeout before architecture ran; architecture deferred to capable CI. No Rust/Python implementation changed.
