# LOO-809: remote attach tickets

Reuse the local attach flow and wire shapes. The planned identity, lifetime and
rejection contract lives in
`rust/loopflow/src/server/README.md#remote-attach-contract`, linked from
`docs/server.md`; keep client behavior there instead of repeating it here.

Implementation and publication wait for LOO-718's remote admission. Its listener,
mobile settings and bulk protocol are outside this Task. No product decision is
missing.

The checkout contains the planned remote contract only. Its existing local
tickets bind to a Unix UID, consume on every redemption attempt and use
`Instant`; remote identity binding and expiry across host sleep remain unbuilt.

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

`lf task status LOO-718 --json` still reported the interactive
`feature` demo step; `gh pr view 1812 --json state,mergedAt,headRefOid,autoMergeRequest`
again reported OPEN, no merge or auto-merge, at the published head above during
the November 8 implement check. `git fetch origin main` advanced origin/main to
`7165925e5df60cef61cd014821deebb4d4339de0`; `git ls-tree` still found neither
`server/mobile.rs` nor `server/ticket.rs`. The transport dependency remains;
no implementation or publication can proceed within this Task's scope.

## Delete — do not maintain

When the transport lands, replace any remaining UID-only `service.rs::Ticket` /
`Tickets` with its shared authority; do not refactor the predecessor while waiting.
Move `a_ticket_opens_one_bridge_for_its_user_within_its_lifetime` onto that store,
retaining identity, single-use, expiry and sweep coverage. Keep the service-level
bridge rejection test and wire fixtures: they exercise the surviving protocol.
Reconcile wire and lifetime differences against the landed revision. Retain
consume-on-attempt; replace ticket timing with `SystemTime` so the 60-second
deadline includes host sleep. Leave unrelated throttles on `Instant`.

## Remaining acceptance

Exercise every case in the remote attach contract through authenticated client
and bridge connections during implementation, gate and QA. Keep the contract
there so the plan cannot drift into a second specification. Reconcile PR #1812's
tests while keeping bulk transfer behavior intact.

The November 8 write-docs run reused `docs/server.md` and
`rust/loopflow/src/server/README.md` unchanged. `lf open --host desktop --file
docs/server.md --file rust/loopflow/src/server/README.md` returned success,
reporting both paths opening in Loopflow Desktop after the earlier failures in
`questions.md`. Visual review remains for demo or QA; the transport is the
implementation dependency.

`git diff --check` — clean on the November 8 implement check; prose-only edits,
no build or tests required. `lf context` — scratch within both limits.

Compress: `git diff --check` — clean; plan-only edits, no build or tests required.
