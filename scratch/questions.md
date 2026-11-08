# LOO-718 decisions and assumptions

November 3, 2026: Jack Heart authorized native fixture implementation before
live connectivity, then sustained improvement after the two initial slices.
Reversible choices: SwiftTerm for native input and accessibility; a separate
`ios.yml` including the shared project; observe first, then grant control
explicitly. Permission/Stop controls stay absent until the backend supplies
action semantics; ordinary typing answers prompts. Physical keyboard, IME,
dictation and VoiceOver quality need device review. Earlier decisions and the
LOO-593 notes this checkout inherited are at `4070315ab3:scratch/questions.md`.

## Revision 2 and tickets — November 5

Jack Heart's steer selected tickets for terminals and a bounded control
connection. Assumptions made while implementing, each reversible:

- Spill whole messages by ticket instead of paging any operation. Nothing the
  phone or Desktop reads today exceeds 1 MiB, so paging would add a second
  reading shape for a path that is not exercised. Revisit if a reading routinely
  spills.
- The ticket lifetime is 30 s and a ticketed transfer has 120 s server-side
  (60 s on the phone). No measurement selected these; they bound a stalled peer.
- Phone requests time out after 15 s and cancel their own id, as revision 1 did
  per connection. Task creation can take longer; the creation sheet already
  recovers the same request by id after a lost reply.
- The phone identifies itself as `kind: mobile, platform: ios`; `welcome` carries
  the Machine id, checked per connection. The negotiated revision is the only
  compatibility check: `welcome.capabilities` and the phone's per-operation
  capability lists were a second, hand-mirrored allowlist that had already
  drifted from the operation names, so compress deleted them (November 5). A
  server missing an operation answers `invalid` by name; a new operation set
  bumps the revision.
- `MobileClient.suspend()` is a protocol requirement with an empty default so the
  fixture client is unchanged; only the remote client closes a connection.
- The opt-in host-Tailscale acceptance was not rerun here: it needs the host's
  private certificate and a dedicated Simulator, and belongs to demo.
- The phone and this checkout speak the revision 2 PR #1813 (LOO-593) shape
  as approved on November 5: dotted operation kinds, `response {result}` typed
  per kind, `subscribe` as its own message, `u64` checkpoints, no `generation`
  on readings. Re-merging that branch was an executive decision; its decisions
  live in PR #1813's `scratch/questions.md`, not here.
- Names chosen for this Task's additions inside that scheme: `view.select`
  (every view plan by expression), `task.start` (create and run a Task, as its
  own `lf` process), `session.start`, `mobile.status`, `mobile.configure`,
  `request.upload`, `terminal.attach` answering `{ticket}`.
- The portfolio is Machine state served as `portfolio.get`/`portfolio.set`;
  the phone's Task list, its creation destinations and Desktop's sidebar all read
  it. A portfolio member with no recorded Work identity contributes no plan.

## Session attachment — November 7

Jack Heart's latest steer settles `session.attach` and removes the earlier
owner-query/terminal-attach split. The November 7 section in `mobile-ios.md`
supersedes the November 5 operation names above. Shared tickets now expire in
60 s, matching LOO-593; the phone retains only its Session identity between
attachments. Jack's key-bar choice remains unresolved and is outside this
protocol reconciliation.
