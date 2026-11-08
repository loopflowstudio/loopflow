# LOO-809: remote attach tickets

Reuse the local attach flow and wire shapes. The planned contract lives in
`rust/loopflow/src/server/README.md#remote-attach-contract`, linked from
`docs/server.md`.

This checkout contains the planned contract only. Implementation, authenticated
connection checks and publication wait for LOO-718's remote admission. The local
UID-only store cannot establish remote acceptance. The listener, mobile settings
and bulk protocol are outside this Task; no product decision is missing.

## Transport dependency — November 8, 2026

LOO-718's PR #1812 supplies `Admission::Remote { node: String, user: u64 }` through
`server/mobile.rs::permits_peer`: Tailscale WhoIs `Node.StableID` and
`UserProfile.ID`, checked against `Node.User`. Both WebSocket paths authenticate
through `host.permits`.

Two inspected revisions differ:

- Published head `b222504a8ededd0f26dbe01e404b1bcb9f446d14`: 30-second UUID-text
  tickets and the older `ticket` / `admitted` wire flow.
- Clean mobile checkout `682e2492bb791dbc7b3c786a68cdfcce00943138`: shared
  60-second lifetime and 22-character base64url tokens.

Both share `server/ticket.rs` between terminal and bulk transfer, bind tickets to
node and user, leave foreign attempts unconsumed and expire with `Instant`.
The mobile checkout tests redemption after foreign attempts, contrary to this
Task's consume-on-attempt contract.

The November 8, 20:29 UTC implement recheck reconfirmed the dependency:
`lf task status LOO-718 --json` reported the interactive
`feature` demo step; `gh pr view 1812 --json state,mergedAt,headRefOid,autoMergeRequest`
reported OPEN, no merge or auto-merge, at the published head above.
`git fetch origin main` found origin/main at
`237a107261572d08e4ffa3c8baf72b1c164c110e`; `git ls-tree` found neither
`server/mobile.rs` nor `server/ticket.rs`. The separate mobile checkout findings
above are from the earlier inspection, not a landed implementation.

## Delete — do not maintain

When the transport lands, replace any remaining UID-only `service.rs::Ticket` /
`Tickets` with its shared authority; do not refactor the predecessor while waiting.
Move `a_ticket_opens_one_bridge_for_its_user_within_its_lifetime` onto that store,
retaining identity, single-use, expiry and sweep coverage. Keep the service-level
bridge rejection test and wire fixtures: they exercise the surviving protocol.
Reconcile wire and lifetime differences against the landed revision. Retain
consume-on-attempt; replace ticket timing with `SystemTime` so the 60-second
deadline includes host sleep. Leave unrelated throttles on `Instant`.
The terminal ticket contract must not silently change bulk transfer redemption
policy when both use the shared store.

## Remaining acceptance

Exercise the remote attach contract through authenticated client and bridge
connections: matching node/user succeeds once; a different node, different user,
or local/remote mismatch fails and consumes the terminal ticket. Check expiry at
60 seconds with a clock advance representing host sleep, expired-ticket sweeping,
and unchanged bulk-transfer redemption behavior. These checks remain for
implementation, gate and QA; local UID tests alone cannot satisfy them.

Visual review remains for demo or QA; Desktop opening results are in
`questions.md`.

After integration, update the docs' planned/local-only wording to match the
implemented endpoint.

`git diff --check` — clean; `lf context` — within both scratch limits. Plan-only
edits need no build or tests.
