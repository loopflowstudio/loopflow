# Restore candidate preflight, then prove installed laptop refresh

## Problem

LOO-292 remains open after PR #1273. Maintainers need `lf rebase` from main or
an ordinary sibling to fetch current upstream, preserve edits and unpublished
commits, and make current main available to a new sibling. `lf install` owns
full checkout, package, Python environment, and published binary refresh. Its
login/hourly job should normally keep that work cheap.

The 2026-09-24 install incident now takes priority: the verified published
candidate cannot emit promotion preflight JSON while a development install is
active. Restoration is **not yet achieved**. This kickoff establishes the
failure mechanism and the next repair; it does not establish installation,
schedule activation, or Task completion.

## The demo

From the existing laptop installation, run `lf install`, then repeat it: both
finish through the published-release path, the second reports the complete
release already installed, and preflight returns the candidate's valid JSON.
Then run `lf install schedule`, observe a real automatic catch-up, and recover
a missed/failed opportunity with installed `lf install` and `lf rebase` before
creating a sibling with `lf wt create <unused-demo-name>`.

## Observations and hypotheses

### Reported observations

- Original main failure: target `bca3f35ac7d4ad24ebed2a30481d0b7ba3a86bde`
  was not an ancestor of HEAD `aef8c0c98a975a9bd3ceafc6bcedc3e2da37e408`.
- Human's 2026-09-24 install completed Homebrew and uv steps and verified the
  published DMG, then failed with `inactive retained install artifact .../lf
  cannot start as Cli`, followed by a JSON EOF. The report did not establish
  the candidate's version or why startup refused it.

### Fresh evidence, 2026-09-24

See `install-incident-20260924.json` for exact commands, outputs, hashes and time.

- Installed `/Users/jack/.local/bin/lf` routes through the machine entry gate.
  It reports `lf 0.12.19+561eadcce`, exposes public refresh and schedule, and
  the active receipt selects **development**, revision
  `561eadccefa6bbf1d67cd832cbe7b5df728d6c8e`, with its separate development store.
- The published fallback is revision `bca3f35ac7d4ad24ebed2a30481d0b7ba3a86bde`.
  The live latest-release redirect still resolves to **v0.12.19**, which
  predates #1273. A working installed development interface is not evidence
  that the published destination contains the refresh feature.
- Downloaded the real v0.12.19 aarch64 macOS archive, checked its SHA256SUMS
  entry, and extracted its CLI into a temporary directory. Its CLI hash
  exactly matches the retained published fallback.
- That candidate's `install preflight --json` exits 1 with empty stdout and
  the inactive-artifact error. `install promote --preview` follows the
  installer's candidate-to-child-preflight path and reproduces the full error
  including EOF. Both results remain identical after removing every `LF_*`
  environment variable from the child.
- The machine's active receipt is byte-identical before/after all four probes;
  no switch receipt was created. These are real published-candidate and live
  machine-authority observations, **not** a complete shell/DMG install or a
  successful promotion. The temporary extraction was removed after capture.
- `com.loopflow.refresh` is not loaded. No refresh plist is present. Existing
  release/telemetry jobs are separate and were not invoked or changed.
- Canonical main is clean at `7160f3637b590988cd2304a186d5c80941c44b65`.
  No main update was attempted in this kickoff; this is not an upstream
  freshness assertion.

### Hypotheses resolved or retained

- **Confirmed:** standard preflight is classified as ordinary runtime startup.
  Both the published source and this Task's source omit `Preflight` from the
  installer commands that bypass ordinary machine authorization. `Promote`
  and `LocalPreflight` already bypass it.
- **Confirmed:** classification is by content hash, not temporary path. Copying
  retained bytes is intentionally insufficient to grant ordinary runtime
  authority. Keep that contract.
- **Refuted as a necessary cause:** inherited `LF_*` context. The clean-context
  reproduction is identical; further environment scrubbing does not fix it.
- **Not yet established:** whether promotion will pass compatibility and
  preservation checks after startup is repaired. Valid JSON can still contain
  a legitimate reject verdict. Do not turn rejection into success.

## Approach

Repair candidate inspection at the existing CLI boundary. Standard
`InstallCommand::Preflight` must reach its existing read-only implementation
without claiming authority to run the active Home. Include it with the existing
installer-only startup exceptions in `rust/loopflow/src/bin/lf.rs`; document why
candidate inspection is independent of active selection. Do not exempt the
whole `Install` command: bare install and schedule are ordinary installed
operations. Do not change `machine_install::authorize_for_switch` or relax its
retained-byte rejection.

Keep the existing preview, identity, migration and executable-compatibility
checks in `lf/commands/install.rs`. The downloaded candidate still describes
itself; the coordinator must not substitute its own build metadata or approval.
Improve malformed child-response diagnostics only if needed to report the
exit status and stderr clearly; this is secondary to reaching preflight.

Existing authority map:

| Responsibility | Owner and path |
|---|---|
| Shared upstream catch-up and preservation | `ops/checkout.rs::refresh_main`, called by rebase, refresh and ordinary worktree creation |
| Caller feature integration | `lf/commands/ops/mod.rs::run_rebase` and `ops/rebase.rs` |
| Full laptop refresh | `lf/commands/refresh.rs`: main, required packages, locked uv sync, `scripts/install.py refresh` |
| Dependency inventory | `SYSTEM_DEPS` and `refresh_required_packages`; no second package list |
| Published asset selection and current-install skip | `scripts/install.py`, pinned installer and manifests |
| Candidate inspection and promotion | `lf/commands/install.rs`; existing switch transaction and immutable artifacts |
| Ordinary installed startup | `machine_install.rs`; OS account authority, content hashes, entry gate |
| Laptop schedule | `scripts/install.py schedule`: installed `lf install`, canonical cwd, stable PATH, RunAtLoad and hourly calendar |

No new installer, receipt store, scheduler, authority, or compatibility path is
introduced. #1273's checkout mechanism stays intact.

## De-risking

| Question | Finding | Impact on design |
|---|---|---|
| Is the candidate corrupt? | Downloaded archive digest and candidate/fallback hashes match. | Fix execution classification, not download verification. |
| Does ambient Run context cause the refusal? | All four live probes fail identically with/without LF context. | Do not implement environment scrubbing as the repair. |
| Would moving the binary fix it? | Temporary downloaded bytes match the retained artifact and are refused. | Preserve hash-based runtime authority. |
| Can a source edit repair v0.12.19? | Published `promote` executes that published binary's own preflight before coordinator handoff. | A containing published repair release is required; changing the local shell script alone cannot fix these bytes. |
| Can a development build be used as the production demo? | Explicit Task direction prohibits promoting unreleased development bytes into the production Home. | Build and test separately; retain the publication dependency. |
| Can HOME isolate a machine-authority test? | `machine_install::account_home` uses getpwuid_r, not HOME. | Use an isolated OS account/container for process-level retained-state fixtures; never overwrite the laptop authority as a fixture. |
| Are green existing tests sufficient? | `local_promotion.rs` calls `build_preview` directly; shell installer tests substitute a shell script for lf. Neither crosses actual CLI startup. | Add real process boundary coverage alongside the existing retained-artifact refusal test. |
| Is schedule loading proof of catch-up? | Current job absent; plist generation/RunAtLoad alone proves no stale checkout recovery. | Capture actual execution and before/after ancestry plus failure/recovery evidence. |

## Alternatives considered

| Approach | Tradeoff | Why not |
|---|---|---|
| Read-only candidate preflight bypasses runtime selection | Small change at the existing boundary; leaves promotion and runtime checks intact. | Chosen. |
| Remove retained-artifact rejection or authorize any verified download | Would permit inactive binaries to claim current runtime state. | Violates the preservation contract. |
| Scrub context, copy the candidate, or invoke preflight on the active coordinator | Scrubbing/copying fails experimentally; coordinator inspection would describe the wrong candidate. | Does not solve the reported path. |

## Key decisions

- Keep the human's command split: rebase refreshes canonical main from any
  ordinary checkout, then integrates the caller. Install owns packages and
  local binary updates. Rebase does not acquire a new build/download step.
- Make the repair on this Task's existing serial branch. No duplicate Task,
  replacement worktree, release start/retry, or artificial PR rotation.
- Restoration and prevention are linked here: the broken check is embedded in
  immutable published bytes. Record the outstanding restoration dependency
  instead of bypassing it with receipt edits or an unreleased production build.
- The human gate is the working demo, not design approval. An external release
  dependency does not mean the human has approved a release or unsafe recovery.
- Wild success: daily refresh is forgotten because current installs are cheap,
  while explicit commands recover stale checkouts and failed opportunities.
  Wild failure: the read-only exception spreads into runtime authority, or a
  green installer simulation conceals another real startup failure. Bound the
  exception and make the process boundary executable in tests.

## Scope

- In scope: candidate preflight repair and regression, published-path recovery,
  installed repeat/catch-up proof, real login/hourly scheduling, preservation.
- Out of scope: release initiation, production development promotion, Task
  placement redesign from LOO-257, new process liveness or scheduling systems,
  deleting Sessions, resetting main, pushing unpublished local commits.

## Done when

1. An isolated account/container fixture runs the real candidate executable
   against an active-development receipt retaining that candidate as published
   fallback. `install preflight --json` emits the candidate identity and typed
   verdict; a deliberately incompatible store yields valid rejection JSON and
   nonzero exit. An ordinary command from the same retained bytes remains
   rejected. Exercise the child path reached by `install promote --preview`.
   Prove the selected store and install receipt unchanged. Do not use a shell
   stub as the only candidate or add a runtime authority-root override.
2. Existing `copied_retained_artifact_cannot_claim_the_active_store` and
   `local_promotion` tests pass. Run affected CLI/installer suites once, plus
   `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings` for Rust
   changes. Preserve the checked command/content receipts; no full suite pass
   is implied by focused tests.
3. A published release contains both #1273 and this repair. Record tag, commit,
   manifest/candidate identities and live preflight JSON. Run the configured
   `lf install` successfully and repeat it. Verify active selection, CLI/daemon
   and app versions, package and uv outcomes, existing Sessions and history.
   A typed reject remains a blocker to investigate, not permission to bypass.
4. Activate `lf install schedule`; inspect actual launchd configuration and log
   outcomes. Observe a real hourly or wake catch-up after upstream advances.
   Record launch time, exit status, upstream and local commits, dependency
   outcome and preserved bytes. A manual kickstart is manual evidence.
5. Demonstrate explicit installed recovery after a recorded missed/failed
   opportunity. Prefer the already-observed absence/failure as the baseline;
   do not cause an outage just to test recovery. Installed rebase from main and
   a stale feature checkout must fetch again and integrate current main, even
   when main was already current but the feature was behind. Repeat after
   upstream advances, then create the sibling with lf. Record real and sandbox
   evidence separately. Retain the previous dirty/unpublished fixture proof;
   rerun it if the repair changes checkout behavior.

## Forbidden outcomes

- Empty preflight output, suppressed EOF, or substitute coordinator JSON counted
  as valid candidate inspection.
- Removing inactive-artifact rejection, rewriting active receipts, moving the
  production Home, discarding Sessions, or force-resetting checkouts.
- Tests that exercise only library preview or a fake candidate and claim the
  installer-to-executable boundary is covered.
- A successful refresh with stale main, origin equality imposed on unpublished
  commits, or a current main used to skip needed feature integration.
- An installed development interface or simulated plist counted as a published
  install, automatic catch-up, or completed Task.

## Internal slices

1. Repair the candidate inspection boundary and prove retained runtime refusal
   plus real child preflight. Include actionable diagnostics only where needed.
2. After an independently authorized published release exists, restore the real
   laptop install and demonstrate repeat invocation without losing Sessions.
3. Activate and observe scheduled catch-up, then explicit recovery and sibling
   creation. Keep Task closure contingent on this evidence.

## This slice

Implement slice 1 next. This kickoff has reproduced the failure and selected
the narrow repair. Production restoration cannot yet be claimed: latest
published v0.12.19 contains neither the refresh surface nor the repair.
Do not start or retry release to remove that dependency.

## Slice ledger

- #1273 / `c56340a14`: shared preserving checkout refresh and full install/
  scheduling source merged. Prior demonstration records actual candidate main
  catch-up, repeat, sibling creation and package setup; automatic proof absent.
- `7892ba68f`: latest retained versions of all three prior scratch notes.
  `f73671bd0` cleared scratch. This kickoff restored those three files exactly
  from `7892ba68f`, after reading each; historical claims remain dated.
- 2026-09-24: clean Task tree at `f73671bd0`; four real downloaded-candidate
  reproductions captured in `install-incident-20260924.json`. No installation,
  release, schedule activation, production edit, or checkout update performed.
- Review finding: a source-only routing test would repeat the original test
  gap. The required process fixture uses an isolated OS authority and the
  actual candidate; changing HOME is explicitly insufficient.
- Five Whys and remaining causal questions: `5whys-install-preflight.md`.
