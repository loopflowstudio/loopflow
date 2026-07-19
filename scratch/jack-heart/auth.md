# 5 Whys: Revoked Codex auth blocked PR publication and its own recovery

## The Problem

`lf pr open` reached PR-copy generation with a revoked Codex refresh token, while the installed `lf auth connect codex engineering` command misparsed the account as a run-time selector and fenced the re-authentication it was supposed to perform.

## Chain

PR copy failed on a revoked token → cached account health still looked active → no live auth preflight or actionable account transition ran → the recovery command created its own fixed lease → the parser fix existed on main but not in the installed binary → provider liveliness, account authority, recovery, and release state had no end-to-end invariant

**Problem**: PR copy generation failed with Codex `token_invalidated`, then `lf auth connect codex engineering` failed because account authority was supposedly fixed by an outer invocation.

**Why 1**: Codex had invalidated both the access path and refresh token for the routed engineering account. The real model request was the first operation that proved the credential could no longer refresh.
↳ *Could we have caught this earlier?* Yes. A live account verification before an agent-backed operation, or a provider-auth failure transition recorded by the launcher, could have named the affected login before PR-copy generation failed.

**Why 2**: `lf auth status codex` still reported the credential as active with eight days remaining because `CodexAuthBroker::check_status` trusts the locally decoded access-token expiry until refresh is due. Revocation is server state and cannot be inferred from that expiry.
↳ *What process allowed this?* Account inventory exposed stored/decoded state as authentication health without a distinct live-verification state or command.

**Why 3**: The documented recovery command could not run in the installed build. Clap assigned the same implicit argument id, `account`, to the global `--account` selector and the `auth connect` positional. Parsing `engineering` populated both fields, so `lf` built a local account lease and `auth connect` rejected itself as a nested account mutation.
↳ *What assumption was wrong?* The parsing test checked the `AuthCommand::Connect` payload but did not assert that the unrelated global account selector remained empty. Commit `d3544bf94` added explicit global argument ids and the missing assertion, but the installed build predates it.

**Why 4**: The recovery path depended on four independently valid mechanisms composing correctly: provider token health, account selection, mutation fencing, and managed-login browser handoff. Their unit tests did not exercise the operator journey from a revoked routed token through re-authentication and retry.
↳ *Why was that assumption encoded?* Account authority was designed fail-closed, while auth mutation and CLI parsing evolved around it. The fence correctly rejected a lease; nothing proved the command could not accidentally mint that lease itself.

**Why 5 (Root)**: Loopflow lacks an installed-binary recovery invariant: given a routed credential revoked by the provider, the running release must identify the login, permit an identity-checked reconnect outside inherited authority, and let the original operation retry or fail over. Main contained the parser repair, but the production binary was fourteen merged commits behind and surfaced that only as a `doctor` warning unrelated to the auth error. The official promotion preflight treated an exact-frontier CLI repoint and a migration-bearing store write as the same risk, so an always-on fleet could keep a merged recovery fix operationally unavailable. Production then exposed a second gap: the release that introduces `runs` queried `runs` before migration, making its pre-migration drain evidence unreadable.

## Unanswered Whys

| Branch Point | Unexplored Question | Priority |
|--------------|---------------------|----------|
| Why 1 | What upstream event invalidated the engineering refresh token, and can Codex expose a durable reason beyond `token_invalidated`? | Medium |
| Release | What operator command should quiesce a resident Home without changing Project/Task lifecycle state? | High |

## Fixes

| Level | Fix | Prevents |
|-------|-----|----------|
| Immediate | Use a validation-only, schema-compatible build at the installed migration frontier; reconnect every managed login; bind Chrome Profile 9 to `codex/manabot-eng`; retry the workflow | This incident without bypassing the production migration boundary or account identity checks |
| Structural | Ship the explicit Clap argument ids and regression assertion already on main | Managed auth positionals becoming run-time account selectors |
| Structural | Add network-backed account verification that marks revoked credentials actionable and prints the exact `lf auth connect <provider> <login>` command | Cached expiry masquerading as live authentication |
| Structural | Treat provider `token_invalidated` as a routable account-health signal and fail over when another granted account is healthy | One revoked account stopping an agent-backed operation |
| Structural | Refuse PR publication before copy generation when the branch has no changes from its base; report that it may already be landed | Expensive model work followed by GitHub's `No commits between` error |
| Structural | Replace the macOS Chrome `open -n` handoff or make the visible-terminal code fallback first-class when Accessibility automation is unavailable | Blank auth windows and hidden one-time-code prompts |
| Structural | Provide a bounded drain operation or prove that schema-compatible binary-only replacement can safely coexist with already-loaded bodies | Correct fixes starving behind permanent global activity |
| Systemic | Add an installed-release acceptance test: revoked routed credential → actionable diagnosis → identity-checked reconnect → successful retry/failover | Drift between individually tested auth, lease, launcher, and release behaviors |

## Evidence after implementation

- `lf auth accounts` says `cached … · live not checked`; `--verify` polls Claude/Codex, records revoked credentials missing, and prints the exact reconnect command.
- Agent launch recognizes `token_invalidated` and `refresh_token_invalidated`, records the selected account through local or brokered authority, removes its leased credential, and retries the route without resuming the revoked provider session.
- Non-Task PR publication refuses an empty base range before copy generation, push, or GitHub mutation.
- Claude opens the selected Chrome executable directly. When browser capture is unavailable, a private Unix-socket handoff opens in Terminal and accepts the code with echo disabled; no Apple Events or Accessibility permission is required.
- Exact-frontier promotion is allowed with 30 live Runs because it writes no store. Each body generation starts and continues through one canonical executable target. Migration-bearing promotion still requires zero Runs.
- Pre-Run stores are censused through legacy Project/Task leases, matching the migration's own quiescence condition.
- Nightly/weekly package acceptance runs the revoked-selected-account recovery journey.
- The production drain found 26 recorded bodies but only 10 live containments. Resident supervision immediately replaced killed bodies until their Wave servers were stopped; after positive containment absence, three final stale leases were reaped under a SQLite backup at `~/.lf/backups/pre-auth-promotion-20260719.db`.

## Changes to Implement

- [ ] Promote an authorized build containing this migration-boundary repair; verify `lf doctor` reports a revision containing `d3544bf94` or later.
- [x] Add `lf auth accounts --verify` with explicit cached-versus-live output.
- [x] Record `token_invalidated` against the selected provider account and exercise route failover in an integration test.
- [x] Add a generic non-Task empty-range guard before PR-copy generation and GitHub mutation.
- [x] Make managed Claude login visibly recoverable without Apple Events/Accessibility permission.
- [x] Make exact-frontier production promotion achievable under an always-on fleet; test the 30-live-Run case and preserve the migration drain.
- [x] Add the revoked-token recovery journey to release acceptance coverage.
