# Keep managed browser login alive without taking over the Mac

## Problem

`lf auth connect` is the recovery path for managed Claude and Codex accounts,
but the recovery ceremony currently makes the machine difficult to use and can
destroy its own OAuth callback listener before the human finishes.

The Claude handoff runs one AppleScript every second for up to 120 seconds. That
script activates Chrome, raises a window, sends global Cmd+A/Cmd+C keystrokes,
and saves and restores the clipboard even while the expected authorization
window does not exist. A user typing or copying in another app loses focus and
can receive those synthetic keystrokes.

The managed Codex path has the opposite problem: after opening the authorization
page it owns the `codex login` process, private `.login-*` staging home, and
localhost callback listener for only 180 seconds. Timing out drops the auth
handle; `AuthProcessGroup` then kills the complete provider process group and
the staging directory disappears. A later redirect to localhost:1455 therefore
cannot complete. This blocks account recovery and, in turn, fleet task runners.

This advances the Loopflow API Project's task-loop trust KR: account recovery
must remain a usable, bounded foreground operation instead of turning ordinary
human login latency into a fleet-wide manual rescue.

## The demo

Run `scripts/dev-lf auth connect claude <account>`, return to another app after
the deliberate browser open, and keep typing while delaying approval. Polling
and code harvest leave that app frontmost and never send it synthetic input.
Then run `scripts/dev-lf auth connect codex manabot-eng@loopflow.studio`, wait
more than three minutes before approving, observe periodic waiting messages,
and see the command complete and `scripts/dev-lf usage` list the connected
account.

## Approach

### Make Claude polling passive and harvesting one-shot

Split `CLAUDE_VISIBLE_PAGE_SCRIPT` into two scripts with visibly different
authority:

1. A passive detection script asks System Events whether Google Chrome has a
   window whose title contains both `Authentication code | Claude Platform` and
   the selected Chrome profile label. It does not activate an app, raise a
   window, read or write the clipboard, or synthesize input.
2. `capture_claude_authorization_code_from_chrome` runs only that script during
   its existing one-second/120-second poll. A missing window continues polling;
   unavailable AppleScript control returns `Ok(None)` and leaves manual entry in
   the invoking terminal.
3. Once detection reports the exact window, run a separate harvest script once.
   That script rechecks the title/profile pair and targets the matching Chrome
   window's active tab with native `select all` and `copy selection` Apple
   Events. It does not activate Chrome or raise any window. It restores the
   clipboard and returns. Parse the handoff exactly as today. If the window
   disappeared, Chrome rejects the targeted commands, or the page did not
   contain a complete code, return `Ok(None)` and wait for a manual paste in the
   invoking terminal; never open or activate another app as a fallback, repeat
   the harvest, or use global keystrokes.

Keep the harvest as one AppleScript command so its clipboard save/copy/restore
window remains as short and internally paired as it is today. Use Chrome's
tab-targeted `select all` and `copy selection` Apple Events instead of System
Events' global keystrokes. The commands work against the addressed tab without
making Chrome frontmost. They are present in both the installed Chrome
scripting dictionary and [Chromium's current scripting
definition](https://chromium.googlesource.com/chromium/src.git/+/refs/heads/main/chrome/browser/ui/cocoa/applescript/scripting.sdef),
and they stay addressed to the Chrome tab even if another app remains
frontmost. If macOS denies Chrome automation, the current `lf` process asks for
the code instead of launching a second Terminal window or taking focus.

Do not make `execute ... javascript` the primary path. Chrome exposes it, but
[Chromium's handler](https://chromium.googlesource.com/chromium/src/+/refs/heads/main/chrome/browser/ui/cocoa/applescript/tab_applescript.mm)
rejects it unless the user has enabled JavaScript from Apple Events. Account
recovery cannot acquire a new browser preference prerequisite.

### Give the foreground login ceremony its real human window

Replace the managed path's fixed
`timeout(AUTH_STATUS_POLL_TIMEOUT, handle.wait())` with a private
`wait_for_browser_confirmation` helper. The helper owns the `handle.wait()`
future, a deadline, and a 30-second heartbeat interval:

- use `flow.expires_in` when the provider actually emits a lifetime;
- otherwise use the existing `AUTH_CODE_FLOW_TIMEOUT_SECS` (600 seconds);
- print `Still waiting for browser approval (<N>s elapsed); the login listener is still running. Press Ctrl-C to cancel.` every 30 seconds;
- return immediately when the provider command completes;
- return the existing account-specific timeout error when the deadline expires.

Use Tokio time for the deadline, elapsed duration, and heartbeat so the behavior
is deterministic under a paused test clock. The helper should accept the auth
wait future rather than own provider construction; this is the semantic
boundary it supervises and lets the timeout behavior be proved without a live
OAuth provider.

Keep `AuthProcessGroup`, the `.login-*` tempdir, and the account login lock
unchanged. Successful completion still verifies identity and atomically installs
the staged credential. Timeout, command failure, or another ordinary early
return still drops the handle and kills the provider group; the existing
interrupt hook kills it on Ctrl-C before `lf` exits. The fix extends valid
foreground ownership; it does not detach a callback server or leave credential
state behind.

### Prove the old boundary and the real recovery path

Add paused-clock behavioral tests around `wait_for_browser_confirmation`:

- a confirmation future completing at 181 seconds succeeds, proving the old
  180-second failure boundary is gone;
- a pending confirmation future times out at 600 seconds, proving the ceremony
  remains bounded;
- a provider-supplied expiry is honored instead of the default.

Keep the existing process-group cancellation test as the proof that dropping or
interrupting a wait still tears the provider child down. The focus guarantee
needs a real macOS demo because its outcome is foreground application and
keyboard behavior, not Rust state. During that demo, watch the frontmost app and
clipboard while the auth window is absent, then approve once and confirm only
one harvest occurs.

## De-risking

| Question | Finding | Impact on design |
|----------|---------|------------------|
| What kills the Codex callback listener at 180 seconds? | `connect_managed_account` wraps `handle.wait()` in the fixed 180-second timeout. On timeout the handle drops, and `AuthProcessGroup::drop` sends SIGKILL to the provider's process group. The private staging home then drops too. | Extend the owned foreground wait; do not detach or transfer the listener. |
| Does the provider already give managed Codex a usable expiry? | `CodexAuthBroker` parses generic CLI output, but current `codex login` emits the loopback listener and authorization URL without an `expires_in`; `AuthFlowResponse.expires_in` is therefore normally `None`. | Default managed browser confirmation to the already-established 600-second authorization-code window. |
| Will a longer wait make Ctrl-C leak the login server? | No. `lf` installs a SIGINT/SIGTERM/SIGHUP handler, and `AuthProcessGroup` registers an interrupt cleanup that kills the exact provider process group before process exit. | Preserve the existing owner and cleanup registration; no new signal path is needed. |
| Can Chrome window presence be checked without focus? | The existing System Events query is read-only; `activate`, `AXRaise`, global keystrokes, and clipboard access are separate statements in the same script. Splitting them makes every unsuccessful poll passive. | Poll with a dedicated side-effect-free script. After an exact title/profile match, address the Chrome tab directly without crossing the focus or keyboard boundary. |
| Can the page be read without the clipboard? | The installed Chrome scripting dictionary exposes JavaScript execution, but Chromium gates it behind the user's “Allow JavaScript from Apple Events” setting. Chrome also exposes tab-targeted selection/copy commands, which avoid global keystrokes but still use the clipboard. | Do not require JavaScript. Use targeted selection/copy once; if Chrome rejects it, ask in the invoking terminal rather than synthesizing input or activating another app. |
| Can the 181-second regression be tested without waiting three minutes or logging into a provider? | Tokio's test dependency includes `test-util`; a private future-based wait helper can run under `#[tokio::test(start_paused = true)]`. Existing provider tests already prove callback URL parsing, authorization-code submission, and process-group teardown. | Add a focused paused-clock result test instead of provider mocks or a slow integration test. |
| Does the fix need a config/env timeout override? | The provider flow already has an optional lifetime and the CLI already defines the 600-second human authorization window. A second knob would create policy drift without helping the reproduced case. | Use provider expiry or the single existing default; add no configuration surface. |

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Read `document.body.innerText` through Chrome JavaScript | Eliminates focus, synthesized input, and clipboard mutation. | JavaScript from Apple Events is disabled unless the user changes a Chrome setting; recovery must work on an unprepared profile. |
| Detach `codex login` after 180 seconds and let localhost:1455 outlive `lf` | The browser redirect could arrive indefinitely. | It abandons ownership of the staging home, lock, identity verification, credential installation, output, and cancellation. It replaces one timeout bug with an orphaned credential server. |
| Wait forever for every managed provider | Removes arbitrary deadlines and leaves cancellation to Ctrl-C. | Provider authorization grants expire, headless callers can stall forever, and the product already has a 600-second interactive-flow policy. A bounded truthful wait is easier to operate. |
| Keep one combined AppleScript but activate only conditionally inside it | Smallest textual diff. | The same script remains both observer and mutator, making repeated harvest on a partially loaded or changed window easy to reintroduce. Two scripts make the side-effect boundary structural. |
| Retain global Cmd+A/Cmd+C for the one-shot harvest | Reuses the currently proven page-copy mechanism. | A foreground change between `AXRaise` and the keystrokes can still send input to another app. Chrome's tab-addressed commands fail closed into a prompt in the invoking terminal instead. |
| Open a visible Terminal when native Chrome automation is unavailable | Preserves the current out-of-band manual handoff. | Launching another app violates the non-interference requirement. Keep the fallback in the terminal the user already chose to run. |

## Key decisions

- Human-confirmed intent: the deliberate browser open may take focus once. After
  the user switches away, `lf auth connect` must still complete without any
  poll, harvest, or fallback activating an app or sending synthetic input.
- The passive poll has zero foreground, keyboard, or clipboard authority.
- After the deliberate browser open, Loopflow never activates or raises Chrome
  or another app. Detection and one-shot harvest both run in the background.
- Synthetic keyboard input is removed from the browser handoff entirely;
  one-shot selection/copy is addressed to the exact Chrome tab.
- If native Chrome automation is unavailable, manual entry stays in the
  invoking terminal and occurs only when the user returns to it deliberately.
- The browser wait is 600 seconds by default, or the provider's stated expiry,
  with a 30-second heartbeat and existing Ctrl-C cleanup.
- The provider process remains a child of the foreground `lf auth connect`
  command for its entire lifetime. No daemon, background callback owner, or
  persistent staging state is introduced.
- The live victim account is part of Done When, not an optional smoke test:
  restoring the fleet's real recovery path is the user-visible point of this
  hotfix.

Wild success is boring in the best way: a password-manager/2FA/SSO login can
take several minutes, `lf` periodically says why it is still present, the
callback lands, and the user keeps using the machine without Loopflow changing
the frontmost app or typing into it. Wild failure would be hiding the old
behavior behind a longer timer: Chrome would still steal input, or a detached
callback would write credentials after `lf` had stopped verifying the requested
identity. The structural split and retained foreground ownership rule those
outcomes out.

## Scope

- In scope: the managed Claude Chrome detection/harvest path in
  `provider_auth/mod.rs`; the managed browser-confirmation wait in
  `lf/commands/auth.rs`; focused Rust tests; real macOS Claude and Codex demos.
- In scope: preserving current title/profile matching, providing a manual
  fallback in the invoking terminal, staged credential verification/installation,
  timeout cleanup, and Ctrl-C process-group cleanup.
- Out of scope: changing provider CLI OAuth protocols, adding a Chrome
  extension, enabling JavaScript from Apple Events, backgrounding login
  servers, adding timeout configuration, or changing account/routing models.
- Out of scope: broad auth-service refactoring or documentation that duplicates
  the timeout constant; the command's heartbeat is the user-facing explanation.

## Done when

- `cargo test -p loopflow browser_confirmation` proves completion at 181
  seconds, bounded failure at 600 seconds, and provider-expiry precedence under
  paused time.
- Existing focused provider tests still pass:
  `cargo test -p loopflow generic_parser_waits_past_the_loopback_callback_listener`
  and `cargo test -p loopflow cancelling_auth_wait_kills_the_provider_process_group`.
- `cargo fmt --check` and `cargo clippy -p loopflow --all-targets -- -D warnings`
  pass.
- On macOS, `scripts/dev-lf auth connect claude <account>` can poll for the full
  window while the user types and copies in another app: the deliberate browser
  open may foreground Chrome once, and after the user leaves it neither an
  unsuccessful poll nor the successful harvest changes focus or sends synthetic
  input. Unsuccessful polls never touch the clipboard.
- On this machine,
  `scripts/dev-lf auth connect codex manabot-eng@loopflow.studio` remains alive
  past 180 seconds, prints the heartbeat, accepts the localhost callback before
  the 600-second deadline, reports the account connected, and
  `scripts/dev-lf usage` shows it.
- A Ctrl-C during either ceremony leaves no provider login process or localhost
  listener behind. Staging-directory cleanup on signal-driven `process::exit`
  is not added to this hotfix.

## Measure

Baseline: the Claude poll can activate Chrome and transact the clipboard once
per second for 120 seconds; managed browser confirmation kills its provider
process group at 180 seconds with no liveness output.

After: the deliberate browser open is the last automatic focus transition;
Claude detection and one-shot harvest cause zero foreground or keyboard events,
and unsuccessful polls do not touch the clipboard. Managed login remains owned
for up to 600 seconds (or the provider's stated lifetime) and emits one
heartbeat every 30 seconds until completion.
