# Accounts-first Gate repair and foreground SSH pinning

## User-visible outcome

The Gate repair keeps the real `connect_account` proof that a Chrome venue
whose current login differs from its durable `expected_login` is rejected
before OAuth, names both logins, and does not stop the next configured venue
from succeeding.

An explicit account now composes with a remote execution location:

```bash
lf --account engineering ssh mini -- lf -m codex : "work here"
```

Loopflow resolves `engineering` against the local account store, verifies its
local provider login, extracts a foreground access token, and sends a concrete
provider/account pin with that token. Remote `lf` runs Codex as exactly that
account without a native account home, account configuration, or route
membership on `mini`.

Without `--account`, `lf ssh` continues forwarding the local repository route,
falling back to the machine default route per provider, and remote selection
continues normal ordered health-aware failover.

## Source of truth

The local account store and the explicit selector are authoritative.

- Local matching uses the existing case-insensitive exact-or-unique-prefix
  account matcher separately for Claude and Codex. One selector may therefore
  produce one concrete pin for each provider it matches. Providers with no
  match are omitted; no matches at all is an error. An ambiguous match for any
  provider is an error rather than a partial bundle.
- Every matched account is live-verified from its local native home before SSH,
  using the same verification path as local `lf --account`. A valid live login
  may be explicit-only, disabled for automatic routing, cooling, outside the
  repository route, or recorded missing; those automatic-selection facts do
  not overrule the operator's explicit choice.
- `ForwardedAccountBundle` carries one required `ForwardedAccountSelection`:
  either `Routed(Vec<ForwardedProviderRoute>)` or
  `Pinned(Vec<ForwardedProviderPin>)`. The wire cannot contain both. An
  explicit bundle contains only matching pinned accounts and access-token
  credentials; it does not invent a one-account route.
- The encoded bundle is the wire fact. The remote SQLite store contains only
  derived account metadata and routes for the foreground lease. Credentials
  remain in the encoded environment bundle and provider child environment;
  they are never written to the remote store or a provider credential file.

Remote resolver precedence is:

1. a forwarded concrete pin for the requested provider;
2. a process-local `LF_ACCOUNT` selector when no forwarded explicit pins exist;
3. the forwarded or native ordered route.

When any forwarded pins exist, requesting a provider without exactly one pin
is a hard forwarded-bundle error. A valid forwarded pin returns an access-token
route directly, without `select_provider_account`, so route membership,
cooldown, paid-through, routing state, and automatic eligibility cannot
reinterpret the local decision. Missing account metadata or credential for the
pin is also a hard error; it never falls back to ambient or native remote auth.

## Affected surfaces and consumers

- Global CLI `--account`: spelling and parsing stay unchanged. The existing
  invocation-scoped `LF_ACCOUNT` carries the selector only inside the local
  process tree; it is input to SSH bundle construction, not the remote fact.
- `lf::commands::ssh::resolve_credentials`: reads the optional local selector
  and asks the local account builder for either explicit pins or ordinary
  routes before spawning `ssh`. `run`, `run_routed`, and `capture_routed` share
  this path.
- `ForwardedAccountBundle`: add `ForwardedProviderPin` and the required routed
  or pinned selection enum to serialization, constructors, round-trip tests,
  and preamble fixtures. No serde default or compatibility shim; both ends
  ship together.
- Local bundle construction: accept an optional selector. Ordinary mode keeps
  repo/default route lookup. Explicit mode scans all local provider accounts,
  live-verifies matches, and forwards only those matches.
- Provider resolver and forwarded hydration: pinned resolution uses the remote
  ephemeral store for derived metadata/rate-limit recording but chooses the
  pinned access token directly. Ordinary forwarded routes continue through
  `select_provider_account` and session/health failover.
- SSH preamble: continue exporting the encoded bundle, create one private
  `LF_ACCOUNT_LEASE_DIR`, point the derived SQLite store into it, run the
  foreground command under the EXIT/signal cleanup traps, and avoid exporting
  the selector as remote authority.
- README: add the combined command and distinguish explicit pinning from
  ordinary ordered-route forwarding.

No Swift or external JSON DTO consumes this internal process-lifetime bundle.

## Absent and error states

- Empty/whitespace selector: local error before SSH.
- No local account store while `--account` is present: local error before SSH;
  never fall back to ambient Claude/Codex tokens.
- No Claude or Codex account matches: local error naming the selector and
  `lf auth accounts` repair.
- A provider has several prefix matches: local error listing that provider's
  candidate account ids; do not forward matches from other providers.
- A matched account has no native home, provider status cannot be read, status
  is unauthenticated, or no access token can be prepared: local error naming
  the provider/account and reconnect command; `ssh` is never spawned.
- Explicit selection outside a discoverable local repository: local error
  because the foreground derived route still needs a repository identity. It
  does not require membership in that repository's configured route.
- Ordinary mode with no route keeps today's behavior: no account bundle and
  ambient local token forwarding may apply. An incomplete eligible route keeps
  today's fail-closed forwarding error.
- Invalid bundle encoding, duplicate pins for one provider, a pin absent from
  `accounts`, or a pin without its access token: hard remote bundle error with
  no ambient/native fallback.
- A forwarded explicit bundle with no pin for the provider the remote command
  requests: hard error naming the missing provider pin.

## Operational boundary

This PR owns foreground process-lifetime leases only. Local account matching,
live status, and token preparation all complete before the SSH transport is
spawned. The token crosses only the stdin preamble/environment channel, never
argv or logs.

The remote shell creates one private temporary directory and SQLite file. The
foreground command does not use `exec`, so the shell retains EXIT, HUP, INT,
and TERM cleanup ownership. On command success or failure, the lease directory
and derived store are removed while preserving the command's exit status.
Existing SSH connection and keepalive budgets remain unchanged.

## End-to-end proof

1. **Gate drift boundary.** The focused `connect_account` test configures
   Profile 3 with `someone.else@example.com` against expected
   `operator@example.com`, then a valid Profile 8. It asserts Profile 3's
   rendered failure names both logins, only Profile 8 opens, the provider runs
   once, and the command succeeds. Removing the expected-login guard must make
   it connect Profile 3 and fail the test.
2. **Explicit local pin to remote access token.** Build a local store with a
   cooling/explicit-only Codex `engineering` account outside the repo route and
   a live credential home. Build, encode, decode, hydrate, and resolve the
   explicit bundle against an empty remote temporary store. Assert the bundle
   has `Pinned(codex/engineering)` and no route, the remote row has `home: None`, remote
   resolution selects `engineering` despite its health/routing state, and
   applying the route supplies `CODEX_ACCESS_TOKEN` rather than `CODEX_HOME`.
   Set a conflicting remote `LF_ACCOUNT` in the fixture so the forwarded pin's
   precedence is executable proof, not commentary.
3. **Ordinary route compatibility.** Build an unpinned bundle from an ordered
   local route whose first account is cooling and second is healthy. Round-trip
   it into an empty remote store and assert normal selection reaches the second
   account by access token while preserving route order.
4. **Local refusal before transport.** Run the SSH command path with a fake
   `ssh` executable that leaves a marker if spawned. Missing, ambiguous, and
   unauthenticated explicit-selector fixtures must each return their local
   error with no marker.
5. **Foreground cleanup.** Run a generated account-bundle preamble through the
   real foreground SSH command path with a fake transport that executes
   `bash -s`; its command records the lease path and materializes the derived
   store. After normal command exit, assert both the lease directory and store
   are gone. Keep the existing trap-string and detached-command rejection tests
   as structural controls.

Verification target:

```bash
cargo test -p loopflow --lib lf::commands::auth::account_first_tests
cargo test -p loopflow --lib provider_account::tests
cargo test -p loopflow --lib lf::commands::ssh::tests
cargo clippy -p loopflow --all-targets -- -D warnings
cargo fmt --all -- --check
git diff --check
```

## Exclusions

- Detached `tmux`, Wave, Project, Task, daemon, or renewable remote sessions.
- Persisting provider credential files or native account homes remotely.
- Forwarding Chrome access profiles; they remain local ceremony venues.
- Changing local non-SSH `--account` semantics, ordinary failover policy,
  provider session pinning, rate-limit recording, or SSH transport topology.
