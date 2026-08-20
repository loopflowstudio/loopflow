# Review: managed browser login hotfix

Reviewed and independently re-reviewed 2026-08-19 against the Task directive,
design, and complete branch diff. No source defects were found in the
implemented slice. The current worktree again passes the paused-clock browser
confirmation tests and the macOS AppleScript compile/authority test. The two
remaining proofs require a human-present macOS browser ceremony and are not
simulated here.

## Evidence matrix

| Claim | Planned behavior | Implemented behavior | Proof | Result |
|---|---|---|---|---|
| Claude polling does not take focus or input | Window discovery has no activation, raising, clipboard, or synthesized keyboard authority | Polling runs only `CLAUDE_AUTH_WINDOW_SCRIPT`; harvesting runs once after an exact title/profile match | `cargo test -p loopflow claude_browser_automation_compiles_without_focus_or_keyboard_control`; source inspection | pass (source/compile), live focus gap |
| Claude harvest is one-shot and background-addressed | Copy the exact Chrome tab without global keystrokes; fall back in the invoking terminal | Harvest uses Chrome `select all`/`copy selection`, contains no `activate`, `AXRaise`, or `keystroke`, then returns immediately; the former Terminal/socket handoff is deleted | Same AppleScript compile test; complete diff | pass (source/compile), live clipboard gap |
| Managed confirmation survives the old boundary | Completion at 181 seconds succeeds and the callback owner remains alive | `wait_for_browser_confirmation` defaults to 600 seconds and owns `handle.wait()` until completion or deadline | `cargo test -p loopflow browser_confirmation -- --nocapture` completed at 181 seconds and printed the 30-second heartbeats | pass |
| Browser confirmation remains bounded | Default timeout is 600 seconds; provider expiry wins | Tokio deadline uses `expires_in` or 600 seconds | Same paused-clock test completed both timeout cases at their exact boundaries | pass |
| Full managed connection still installs verified credentials | Preserve staging-home, profile retry, identity verification, and installation | Existing `connect_account` path runs through the new wait and completes normally | `cargo test -p loopflow connect_tries_access_profiles_in_configured_order` | pass |
| Cancellation tears down the provider listener | Dropping or interrupting the wait kills the exact provider process group | `AuthMonitor::drop` aborts its task; `AuthProcessGroup::drop` kills the group | `cargo test -p loopflow cancelling_auth_wait_kills_the_provider_process_group` | pass |
| Loopback-listener output is not mistaken for completion | Continue waiting after the callback listener is announced | Generic parser/monitor keeps ownership until provider exit | `cargo test -p loopflow generic_parser_waits_past_the_loopback_callback_listener` | pass |
| Slow real Codex approval completes | Delay approval past three minutes, accept localhost callback, connect the account, and show it in `lf usage` | Source and deterministic boundary proofs support the path | Human-present `scripts/dev-lf auth connect codex manabot-eng@loopflow.studio` | gap |
| Real Claude ceremony leaves another app undisturbed | Type and copy elsewhere during polling and harvest without focus or clipboard interference | Source removes every focus/global-input operation and limits clipboard access to harvest | Human-present macOS observation | gap |

## Source review

- `connect_managed_account` remains the sole owner of the staging home, login
  lock, provider handle, identity verification, and credential installation.
- The two AppleScripts intentionally duplicate title/profile matching to keep
  passive observation structurally separate from one-shot clipboard authority.
- The 180-second `wait_for_active_status` policy remains only on the separate
  service/store polling path; managed provider-process ownership now uses the
  600-second browser-confirmation policy.
- No background listener, timeout configuration, alternate auth model, or
  compatibility path was introduced.

## Disposition

Do not publish yet. Complete the two human-present demos above, then refresh
this matrix with the observed account and focus results. If both pass, the Task
PR is ready for `lf pr publish`; review does not land it.

A headless victim-path attempt on 2026-08-19 ran
`scripts/dev-lf auth connect codex manabot-eng@loopflow.studio`, but the sandbox
failed before provider startup while resolving the provider account store
(`Operation not permitted`). It proves neither success nor failure at the
browser boundary; the host-authorized ceremony remains required.

Repeated review/implement attempts on 2026-08-19 tried to open that intervention
with `lf ask --user`, but Loopflow rejected every Ask because the
AgentInvocations had no active Turn authority. `lf ps --json` could not inspect
the receipt in this sandbox (`Operation not permitted`). The auth implementation
is now checkpointed and pushed; review still does not publish it without the
missing human-present evidence. This note cleanup remains uncommitted because
the sandbox cannot write the shared worktree index or runtime ledger.
