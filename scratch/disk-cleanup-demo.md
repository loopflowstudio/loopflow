# Laptop cleanup demo — 2026-11-04

Task: LOO-722. Review PR #1820 at `9575a5b6eb11466ccef5d7c672f0590cd402c9bc`, base `dfd454cfc783b5ff3014d5a398ccf1627b427d11`.
The Task is at demo. The implementation Graph completed; review launches no new
Graph and does not approve merge or Task completion.

## Experience and evidence

The experience is aggressive coding-only cleanup through the installed hourly
launchd job, with explicit preservation and visible partial failures. The current
design is [loo-722.md](loo-722.md); accepted policies are in
[cleanup-policy-decisions.md](cleanup-policy-decisions.md).

Fresh read-only audit at 13:47 PST: all five current source, installed and final
acceptance digests match; installed entry-point help exits zero; the enabled
calendar job retains the exact hourly :17 plist and last exit zero. Both private
project reservations and their root identities remain intact. Raw audit evidence
is private under `~/.lf-machine/maintenance/disk-cleanup/evidence/20261104-LOO-722/demo-review/`.

The preserved final calendar acceptance at 13:12–13:15 removed its successful and
held specimens (180224 allocated bytes), recorded TERM then KILL, preserved a
neighboring process, failed/saved/recent evidence, unmarked source/history and
reserved roots, and restored the schedule. These are recorded agent observations,
not a newly watched deletion or a statement of Jack Heart's demo acceptance.
`calendar-owner-poll/` holds exact receipts, assertions, file digests and teardown.

The ordinary 13:17–13:20 run exited zero, retained 263 candidates and removed zero
allocated bytes. Its observed free delta was +634703872 bytes. The final controlled
run's free delta was -1544781824 bytes. Concurrent work prevents attributing either
change to cleanup. The 11:17, discovery and 13:05 failures remain historical
counterevidence; final repaired bytes passed without deleting under surviving owners.

At 13:48:57 PST, live free space was 208.0 GiB (223.3 decimal GB), compared with
217.5 GiB at the last run's finish. Current usage history has 12 samples: latest
27 measured roots, 101 unknown, 8 growing, 1 shrinking. These are varying-interval,
overlapping observations, not proof of sustained containment.

Local gate log `.lf/tmp/gate/run-11361/python/python.log` records 627 passed and
one skipped. Publication records architecture/checkout acceptance and lint passes.
No suites were repeated for this documentation-only review. Most remote checks
are skipped; do not report them as independent passing CI.

## Human feedback and agreed change

Jack Heart requested both demo and PR-review walkthroughs. During review Jack
asked that the Scheduled disk cleanup guide have its own Markdown file rather
than live in scripts/README.md. Agreed and applied: move the guide unchanged to
`scripts/disk-cleanup.md`, with a short link from `scripts/README.md`. It is a local,
unpublished documentation correction; implementation and installation are unchanged.

Jack asked where free space ended up; the fresh 208 GiB measurement and 9.5 GiB
drop since 13:20 were reported. Previously accepted cleanup policies remain
accepted. No feedback on these new rendered demo/review pages has arrived yet.
No approval is inferred from opening or closing them.

## Review surface and remaining work

- Experience runbook: `scratch/demo.html`.
- Pinned implementation walkthrough: `scratch/pr-review.html`.
- This durable record: `scratch/disk-cleanup-demo.md`.
- Operating guide: `scripts/disk-cleanup.md` (local move not in published head).

The page asks whether observed keep/delete behavior matches the accepted rules
and whether the chosen 8/10/8 GiB native cache budgets need adjustment. Those
budgets are implementation choices within accepted bounded-cache policy, not
new mandatory approvals. Draft notes in the HTML remain local until shared here.

Recommended next action: review the installed behavior in this conversation,
record feedback here, and keep successive comparable scheduled observations for
the original laptop-wide outcome. Sustained containment remains unproven. The
caller chooses further execution or delivery; the demo does not restart a Graph.
The documentation move should accompany the next authorized publication.

Both HTML pages were rendered at 1100px/light and 390px/dark. No network requests,
script errors or body overflow occurred. Code, navigation and feedback sections
were visually inspected; local note persistence was exercised. The new guide
preserves the former README section's commands and policy verbatim, and its link
resolves. Whitespace checks passed. No implementation tests were rerun because
the only source edit in this review is the documentation move.
