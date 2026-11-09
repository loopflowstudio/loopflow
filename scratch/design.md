# LOO-809: remote attach tickets

Implement the [remote attach contract](../rust/loopflow/src/server/README.md#remote-attach-contract),
linked from `docs/server.md`. This checkout contains the planned contract only.
The shipping docs are `docs/server.md` and `rust/loopflow/src/server/README.md`;
read both before implementation. The write-docs pass retained their existing
contract and planned/local-only wording.

The listener, mobile settings and bulk protocol are outside this Task.
Keep the shared attach rules in one place; the remote section adds only its
exchange example, authentication and expiry requirements.

## Transport dependency — November 9, 2026 UTC

Previously inspected LOO-718 revisions supply `Admission::Remote { node: String, user: u64 }` through
`server/mobile.rs::permits_peer`: Tailscale WhoIs `Node.StableID` and
`UserProfile.ID`, checked against `Node.User`. Both WebSocket paths authenticate
through `host.permits`.

Those revisions share `server/ticket.rs` between terminal and bulk transfer,
leave foreign attempts unconsumed and expire with `Instant`, contrary to this
Task's consume-on-attempt and host-sleep requirements. Their wire and lifetime
differences are recorded in
`9de33d514bb85801b3ff24bbc7f229aaaae0bc46:scratch/design.md`.

The November 9 implementation check at 19:32 UTC found PR #1812 still OPEN at
`2f89c2558347e63e40a679a0d59adefa1cf2d84b`, without auto-merge (`gh pr view`).
That revision's ticket implementation has not been inspected; the earlier
findings do not establish its behavior. `git fetch origin main`
returned `4848816f73ccbacd5f1eb26bff7f54814d7c0c8b`; `git ls-tree` found neither
transport file. The separate mobile checkout findings are not a landed
implementation. `lf task status LOO-718 --json` still reports `feature` at `demo`,
with PR #1812 open.

Implementation and remote acceptance depend on LOO-718's merged transport.
Landing that PR is outside this Task's scope. No product decision is missing.

## Delete — do not maintain

When the transport lands, replace any remaining UID-only `service.rs::Ticket` /
`Tickets` with its shared authority; do not refactor the predecessor while waiting.
Move `a_ticket_opens_one_bridge_for_its_user_within_its_lifetime` onto that store,
retaining identity, single-use, expiry and sweep coverage. Keep the service-level
bridge rejection test and wire fixtures: they exercise the surviving protocol.

## Remaining implementation

Reconcile wire and lifetime differences against the landed revision. Retain
consume-on-attempt; replace ticket timing with `SystemTime` so the 60-second
deadline includes host sleep. Leave unrelated throttles on `Instant`.
The terminal ticket contract must not silently change bulk transfer redemption
policy when both use the shared store.

After integration, update the docs' planned/local-only wording to match the
implemented endpoint.

## Remaining acceptance

Exercise the remote attach contract through authenticated client and bridge
connections: matching node/user succeeds once; a different node, different user,
or local/remote mismatch fails and consumes the terminal ticket. Check expiry at
60 seconds with a clock advance representing host sleep, expired-ticket sweeping,
and unchanged bulk-transfer redemption behavior. Local UID tests alone cannot
satisfy these checks.

Retain local attachment and its consume-on-attempt behavior through the shared store.

Desktop review and opening results are in `questions.md`.

`git diff --check` — clean; `lf context` — within both scratch limits. Prose only;
no build or tests needed.
