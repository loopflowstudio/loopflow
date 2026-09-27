# LOO-312 assumptions and outstanding evidence

- The selected step is implement. This pass repairs the completion ordering
  identified in `installer-slice-review.md` and `concept-review.md`: commit
  selection before reopening, retain the active receipt until Home recovery
  and reopening succeed, and observe an already reopened app on retry. The
  existing receipt and startup-routing rules remain authoritative. No new
  product decision was needed. See the design's current slice and ledger for
  proof and limits; the earlier reviews remain historical findings.
- Recovery must preserve retained release history even if interruption falls
  after active.json/archival and before pending-receipt removal. The same
  recorded ActiveInstall is now reused at that boundary.
- First-install cancellation has no prior installation to authorize. Ordinary
  helper startup stays unavailable until cancellation completes; install
  commands bypass that authorization. A live copied-app activation/retry proof
  must cover this boundary, in addition to the isolated unadvanced-update
  rollback fixture. Do not authorize an unadvanced candidate's normal startup
  to make that proof pass.
- The no-rendering constraint prevents the live baseline required before UI
  edits. Continue with installer-owned work and isolated process fixtures;
  do not substitute those fixtures for the baseline or clean-account trial.
- Source inspection found that `quiesce_switch_app` ignored the app target when
  no prior receipt existed and the old AppleScript quit addressed an app ID,
  not the selected copy. Both are repaired in this pass. A published helper
  running inside the bundle must re-exec retained bytes before quiescing.
- Signature validation currently lives in `release/install.sh` (`codesign`),
  while Rust validates bundled helper identity and tree digests. The forthcoming
  bundle-activation entry must bring signature verification into the installation
  owner; calling promotion directly does not establish signed-bundle acceptance.
- Jack wants Linear to be optional sharing, not a requirement anywhere in the
  ordinary local experience. Existing Task/planning coupling needs actual
  product work; determine the full boundary with the related LOO-316 work.
- Jack confirmed GitHub sign-in during initial setup: "make sure theyre signed
  in asap". Require verified sign-in before **Open project…**, reusing an
  existing connection. The `gh` installation mechanism remains implementation
  work, with no package-manager prerequisite. See `account-requirements-review.md`.
- Jack prefers scoped sessions and is content to avoid init here. Use app
  actions for initial setup, with no onboarding conversation or required init
  after opening a project. Repo-wide init was a possible separate direction,
  not a request to redesign it in this Task. See `init-setup-review.md`.
- The Task explicitly ends at an open Loopflow with one obvious action. This
  design uses Open project; creating a project/first Task remains LOO-316.
- Jack confirmed "drag to app -> open". Keep the direct DMG download and add
  first-launch activation through the existing Rust installation owner. No
  initial external Terminal command. See `dmg-first-launch-review.md` for
  remaining activation evidence and the revised first slice.
- This run has no rendering environment or fresh logged-in macOS account.
  Current primary-screen behavior is a code finding, not live confirmation.
  Capture the baseline in a running app before UI implementation or a present-
  tense walkthrough. Clean-account acceptance remains outstanding.
- HOME-isolated shell probes are useful and were performed. They do not isolate
  machine install state, Keychain, preferences, Git installation, or an OS user.
  Do not run promotion/login on this account to manufacture acceptance evidence.
- Guided setup supports Codex and Claude Code; an existing OpenCode setup keeps
  working. No provider cost, entitlement, quota, or purchase is assumed.
- Default Mac zsh gets automatic PATH persistence. Other-shell instructions
  remain explicit; no universal shell configurator is proposed.
- The Wave's paused discrepancy does not affect local design work and was not
  changed. No live planning queries or cross-Work launches were needed.
