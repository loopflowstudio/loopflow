# b28a8a: one Linear 429 stops pausing every Linear write

## Outcome

A Task created while PR links are being refused reaches Linear within a
minute. A 429 on `LinkAttachment` holds `LinkAttachment`, nothing else, and
only until Linear says that limit refills. Failing PR links stop going out
from every `lf` command.

Done when: with `lf server run` up on this build and the link backlog
still in `tracker_actions`, `lf task create 'probe'` shows its Linear
identifier within a minute, and `lf history linear` shows no `paused` rows
for anything except `LinkAttachment` after a `LinkAttachment` 429.

## What the log and the code show (2026-11-22)

Read from `~/.lf/logs/linear-requests.jsonl`, `~/.lf/loopflow.db` and the
code at 621fb0926d.

**Cause 1: a 429 borrows the hourly reset.** `cooldown::reset_at`
(`rust/loopflow-execution/src/linear/cooldown.rs`) takes, when the response
is limited, the reset of *every* scope it reports (`if empty || limited`),
including `x-ratelimit-requests-reset`, the global hourly window. Every
`LinkAttachment` 429 arrived with 9952–9982 of 10000 requests left, so the
cooldown file was set to the hourly reset and `Cooldown::check` paused every
operation until it. Linear's docs
(<https://linear.app/developers/rate-limiting>): "Some queries and mutations
have individual request rate limits that are lower than the global request
limit", reported as `X-RateLimit-Endpoint-Requests-{Limit,Remaining,Reset}`
and `X-RateLimit-Endpoint-Name`; a limited request is HTTP 429 with
`RATELIMITED`, and `Retry-After` gives seconds to wait. The request log does
not record the endpoint headers, so which endpoint and window Linear applied
is unread; the build records them.

**Cause 2: failing PR links resend from every `lf` command.** Each `lf`
process (`c6487bb3`, `98ae0a66`, `1fa0e34e`, `dec08480`, and the server) sends
the same 13 `LinkAttachment` within half a second, gets 200 on each, and
sends no `UpdateAttachment` after any of them: 9 `UpdateAttachment` against
~1000 answered `LinkAttachment` in the current log. `pr_link`
(`linear/actions.rs`) sends `UpdateAttachment` right after a successful
`link_attachment`, so `link_attachment` is returning an error on a 200 body.
The action stays `sent` (`Sent::Unknown`), and `rules::lost_reply` resends an
idempotent `sent` action on every pass with no window
(`loopflow-storage/src/planning/rules.rs`). Every `lf` command's after-save
pass (`planning_sync::sync_saved`, `Scope::First`, `Bound::PASS`) drains all
owners, so each command re-sends all 13. About 26 `LinkAttachment` inside
~11 s is when the 429s arrive.

The stored errors don't show the 200-body failure: when
`Tracker::for_repository` fails while paused, `drain_repository` writes its
error onto every queued action, so all of them read "Linear rate limit
reached; saved changes retry after it resets".

Probe, 12:04:59 on 2026-11-22: the first pass after the reset saved the
real error on all 13 `sent` links before a paused pass could overwrite it.
Each says `This URL has already been linked with LOO-966.` (LOO-979, …),
and every named issue is **that Task's own issue** (seq 226 → LOO-966 is
task_54326b36's `issue_identifier`). So `attachmentLinkURL` answers 200 with
a GraphQL error when the URL is already attached to the same issue. The
comment on `LINK_ATTACHMENT_MUTATION` ("re-linking the same PR returns the
existing attachment") is wrong for these. The 36 confirmed links are first
links. A second link action for the same PR (a state change, or a retry
after a lost reply) can never confirm.

## Design

### 1. A 429 pauses what Linear limited

`Cooldown` keeps one deadline per scope it can name:

- The global deadline (today's file) is set only when `requests` or
  `complexity` reports `remaining <= 0`, and runs to that scope's reset.
- An operation deadline (`locks/linear-cooldown/<key>.op/<Operation>`) is set
  when the 429 names an endpoint (`x-ratelimit-endpoint-*` exhausted or
  `x-ratelimit-endpoint-name` present), or when the response is limited and
  no global scope is exhausted. It runs to the endpoint reset, else
  `Retry-After`, else 60 s.
- `send` checks the global deadline and its own operation's deadline.
  `graphql` already knows the operation name (`request_log::operation`);
  that function moves to where both use it.

The 1-hour fallback stays only for a global exhaustion with no usable reset.

Choice made for review: the operation deadline is keyed by our GraphQL
operation name, not `x-ratelimit-endpoint-name`. We can check our own name
before sending; Linear's endpoint name is only known after a 429. Passed
over: a per-kind pause in the drain (links vs creations), which would leave
the client pausing everything for any other endpoint-limited mutation.

### 2. A failing idempotent action waits its window

`rules::lost_reply` resends an idempotent `sent` action only once `sent_at`
is older than `window` (600 s on a pass, 0 for `lf task sync`), the same
window looks already use. A genuinely lost reply resends at the next pass
after ten minutes; `lf task sync <id>` resends now. This turns "13 links per
`lf` command" into "13 links per ten minutes".

### 3. An already-linked PR reuses its attachment

When `link_attachment` fails with `This URL has already been linked with`,
`pr_link` reads the issue's attachments and takes the one whose `url` is
this PR, then goes on to `UpdateAttachment` and the comment as usual.
`item_attachment_urls` (`IssueAttachments`, `linear/mod.rs`) already pages
an issue's attachments; it starts returning `{id, url}` and its one caller
reads `url`. If no attachment on the issue has the URL (linked to another
issue), the action retires with Linear's message (`Sent::Unsupported`):
moving a PR between issues is not this Task's call.

Choice made for review: reuse `IssueAttachments`, which is known to work
against the live API, over `attachmentsForURL(url:)`, which is one request
but unprobed here. The match is on Linear's message prefix because the
error has no distinguishing code in what we saw; the stand-in test pins
that text.

Fix the comment on `LINK_ATTACHMENT_MUTATION` to say what Linear does.

### Request log

`LinearRequestRecord` gains `endpoint: Option<Allowance>` plus
`endpoint_name: Option<String>`, and `retry_after_ms`, so the next 429 says
which limit it was. Old lines without the fields still parse: serde reads a
missing `Option` field as `None`.

## Exclusions

- Reordering the drain so creations go before links. With (1), a link 429
  no longer holds creations; ordering stays oldest-first.
- The pacing/reserve budget (`Budget`), untouched.
- Asana.

## What goes

- `reset_at`'s "limited takes every scope's reset" branch.
- The unconditional idempotent `Resend` in `rules::lost_reply`.
- The wrong "dedupes" comment on `LINK_ATTACHMENT_MUTATION`.

Survives: the 13 stuck link actions in `tracker_actions` confirm on the
first pass after install through (3); nothing needs hand repair.

## Slices

1. **Scoped cooldown** (teaches the most: proves a link 429 no longer stops
   a creation). `cooldown.rs`: global vs operation deadline; `send` checks
   both; request log records endpoint headers. Test with `test_server`: a
   429 on `LinkAttachment` carrying healthy global headers and an exhausted
   endpoint scope pauses the next `LinkAttachment` and sends the next
   `DeliverTaskCreation`; a 429 with `requests-remaining: 0` still pauses
   both. Rewrite `cooldown_uses_reset_milliseconds_retry_after_and_safe_missing_reset`
   for the new reset rules.
2. **Resend window.** `rules::lost_reply` + a drain test: a `sent` idempotent
   link whose send failed is not resent by a second pass inside the window,
   and is by `Bound::NOW`.
3. **Already-linked PR.** `pr_link` + a stand-in test in `linear/actions.rs`:
   `attachmentLinkURL` answers `This URL has already been linked with
   LOO-1.`, `IssueAttachments` lists the URL, and the action confirms with
   that attachment's id after one `UpdateAttachment`. The second case
   (URL not on the issue) retires.

Order: slice 3 is the smallest and stops the burst that trips the 429
today, so it may go first; slice 1 is what the outcome needs (a creation
never waits on a link 429), and slice 2 keeps the next persistent error
from becoming a storm.

## Acceptance at gate

`cargo test -p loopflow-execution --lib linear::cooldown::` and
`ops::tracker::drain::` pass, including the two new scenarios. On the live
Machine after install: `lf task create` during a link backlog reaches Linear
in under a minute; `lf history linear` shows `LinkAttachment` sends at most
once per ten minutes per link.
