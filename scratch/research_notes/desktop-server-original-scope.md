# Historical LOO-593 scope

Captured before the November 3 mobile/backend handoff. Historical evidence, not current implementation direction. App-owned UI automation remains app-owned; the new Machine service has a separate lifetime. Unselected automation routing/discovery extensions remain later questions.

## Problem and outcome

Jack Heart, October 25, 2026 (LOO-519 concept review): Loopflow Desktop needs a server running inside it. LOO-519 ships a minimal one: an app-owned Unix socket serving the `lf desktop` API (inspection, the action verbs, `wait`, `events`) as newline-delimited JSON, replacing Apple events. Jack asked for a follow-up to work out what other roles that server should have and how to architect it well for the long term.

Beneficiary: everything that talks to a running Desktop (the `lf` CLI, agents in its panes, tests, and later other hosts).

## Questions to answer

* **Roles.** Which other paths to a running Desktop should move onto it: `lf open` routing, `loopflow://` URL handling once the app is running, Session attachment and input, store-change notification, the Desktop side of Graph and Process observation, test fixtures.
* **Boundaries.** The server lives and dies with the app and hosts no Sessions (lf's detached transport owns executions). Keep it that way; Jack had `lfd`, wave listeners and residents deleted, and this must not become one.
* **Protocol.** Versioning and skew between `lf` and Desktop, schema ownership (Rust DTOs mirrored in Swift, per the DTO rules), method discovery (Rex's `api list` / `api describe` as reference), errors and exit statuses.
* **Security.** Same-user socket permissions and peer checks replaced the Automation consent prompt; decide whether anything (input into terminals, remote hosts) needs more.
* **Remote.** Whether a Desktop on another Machine is ever reachable (Rex `-S`), and through what (`lf ssh` forwarding vs nothing).
* **Lifecycle.** Installed vs Dev side by side, crash and stale-socket recovery, multiple windows and repositories.

## Relationships

Follows LOO-519 (PR #1677). Reference: Rex's server API, [https://www.superlogical.com/rex/docs/automate/server-api](<https://www.superlogical.com/rex/docs/automate/server-api>).