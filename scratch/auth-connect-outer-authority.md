# 5 Whys: Auth connect creates its own fixed account authority

## The Problem

`lf auth connect codex engineering` fails in an ordinary local terminal because
the command creates a fixed account lease for its own auth target and then
refuses to edit authentication under that lease.

## Chain

Auth connect rejects itself → the auth target also becomes a global route
preference → Clap identifies both arguments as `account` → parser tests inspect
only the nested auth command → internal account IDs are exposed as a second
user-facing identity

**Problem**: A local `lf auth connect <provider> <account>` reports that account
authority was fixed by an outer invocation even when no outer invocation or
`LF_ACCOUNT_LEASE` exists.

**Why 1**: The positional auth target populates both
`AuthCommand::Connect.account` and the top-level repeatable `Cli.account`. A real
target such as `engineering` resolves successfully, so startup creates a local
lease before dispatching the auth command.

↳ *Could we have caught this earlier?* A parser assertion that the top-level
selection remains empty for every auth lifecycle command would have caught it.

**Why 2**: Both arguments use Clap's derived ID `account`. Making the top-level
`--account` option global caused its value to propagate across subcommand
boundaries that already contained account arguments.

↳ *What process allowed this?* The lease change tested global selection and auth
parsing independently, but never parsed an auth target and inspected the entire
`Cli` value or dispatched it with a resolvable account catalog.

**Why 3**: Existing auth parser tests assert only the nested enum fields they
care about. They prove that `AuthCommand::Connect` receives the target, but not
that unrelated top-level routing state remains untouched.

↳ *What assumption was wrong?* Field ownership in the derived Rust structures
was treated as sufficient argument isolation. Clap's global propagation is
keyed by argument ID, not by Rust nesting.

**Why 4**: `account` describes two different inputs: a process-tree route
preference and the identity whose credentials are being edited. The latter can
be an internal account ID, a login email, or sometimes an ID prefix, so the CLI
has no single canonical identity.

↳ *Why was that assumption encoded?* Stable path-safe account IDs were promoted
from storage keys into labels and selectors, even after verified login email
became available and unique per provider.

**Why 5 (Root)**: Loopflow lacks a boundary between stable internal account keys
and user-facing account identity. That lets storage names leak into parsing,
display, documentation, and repair commands, and made an argument-ID collision
both possible and hard to notice.

## Unanswered Whys

| Branch Point | Unexplored Question | Priority |
|--------------|---------------------|----------|
| Why 3 | Should a generic parser invariant assert that every subcommand leaves unrelated global state untouched? | Medium |
| Why 5 | Should a later migration replace internal IDs with opaque generated keys in persisted records? | Low |

## Fixes

| Level | Fix | Prevents |
|-------|-----|----------|
| Immediate | Give global provider selectors explicit Clap IDs distinct from auth identity arguments. | `auth connect` creating and rejecting its own lease |
| Structural | Name auth inputs as emails and resolve exact or unambiguous email prefixes for every existing-account command. | Internal aliases remaining necessary for auth, access, route, lifecycle, and reset commands |
| Systemic | Render login emails in account and usage surfaces while retaining account IDs only as stable storage keys. | A second user-facing identity drifting back into commands and repair guidance |

## Changes to Implement

- [x] Isolate global account-selector argument IDs and add parser regressions for auth commands.
- [x] Resolve existing accounts by exact or unique email prefix, case-insensitively.
- [x] Use login email in usage, auth account listings, repair commands, and account-facing docs.
- [x] Keep persisted account IDs and credential-home paths unchanged.
- [x] Run focused auth, selector, usage, parser, formatting, and clippy checks.
