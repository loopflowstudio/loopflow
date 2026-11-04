# Laptop cleanup demo — 2026-11-04

Task: LOO-722. Review PR #1820 at `9575a5b6eb11466ccef5d7c672f0590cd402c9bc`, base `dfd454cfc783b5ff3014d5a398ccf1627b427d11`.
The implementation Graph completed and the Task reached demo. Jack Heart then
reviewed the pages and explicitly approved shipping: “lg2m ship it”. This
supersedes the pending-review disposition below and authorizes the Task ship path.

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
`scripts/disk-cleanup.md`, with a short link from `scripts/README.md`. The PR
preparation checkpoint `f812515e2b` includes this documentation correction;
implementation and installation are unchanged.

Jack asked where free space ended up; the fresh 208 GiB measurement and 9.5 GiB
drop since 13:20 were reported. Previously accepted cleanup policies remain
accepted. Jack Heart subsequently said “lg2m ship it”, accepting the reviewed
behavior and authorizing shipping. No approval was inferred from opening or
closing the pages.

## Review surface and remaining work

- Experience runbook: `scratch/demo.html`.
- Pinned implementation walkthrough: `scratch/pr-review.html`.
- This durable record: `scratch/disk-cleanup-demo.md`.
- Operating guide: `scripts/disk-cleanup.md` (included in `f812515e2b`).

The page asks whether observed keep/delete behavior matches the accepted rules
and whether the chosen 8/10/8 GiB native cache budgets need adjustment. Those
budgets are implementation choices within accepted bounded-cache policy, not
new mandatory approvals. Draft notes in the HTML remain local until shared here.

Recommended next action after Jack Heart's explicit approval: run `lf ship LOO-722`
through remaining checks and landing, verify delivery and the installed job, and
preserve the outstanding sustained-containment evidence in durable PR copy. The
existing PR already states that long-term containment is unverified. Shipping
approval accepts the demonstrated implementation without inventing that proof.

Both HTML pages were rendered at 1100px/light and 390px/dark. No network requests,
script errors or body overflow occurred. Code, navigation and feedback sections
were visually inspected; local note persistence was exercised. The new guide
preserves the former README section's commands and policy verbatim, and its link
resolves. Whitespace checks passed. No implementation tests were rerun because
the only source edit in this review is the documentation move.

## Headless demo decision — 2026-11-04, 14:10 PST

Decision: advance the existing Graph to delivery. Jack Heart's explicit
“lg2m ship it” approval is recorded in Task comment
`05f946d5-d646-4c9e-9fe4-6167266327bf`; this decision applies that approval,
not an inferred response to the headless run. The documentation move is included
in current HEAD `f812515e2b33d83e5179652b69ec38e433b99fbe`.

The preceding implementation intended to repair discovery and owner polling,
prove calendar-triggered cleanup, and preserve protected content. Its final
installed acceptance passed all fourteen assertions, including TERM/KILL,
deletion after owner exit, and preservation. Earlier failed cycles remain
counterevidence for their superseded versions. The ordinary 13:17 cycle also
passed. These observed results constitute progress; no concrete implementation
gap requires returning to implement at this review boundary.

A fresh read-only configured-path audit confirmed all five current source,
installed and accepted digests match, the enabled hourly :17 plist is byte-for-byte
unchanged, installed `--help` succeeds through its actual clean-environment
launcher, and launchd is waiting with last exit zero. Both project reservations
remain configured. The latest receipt is still the successful 13:17–13:20 run;
this audit did not perform or imply another deletion. Free space at the audit
was 208.32 GiB. Raw evidence: private
`~/.lf-machine/maintenance/disk-cleanup/evidence/20261104-LOO-722/demo-decision/audit.json`,
with adjacent `launchctl.txt` and `installed-help.txt`.

No new design changes or unresolved product questions arose. Sustained containment
is still unverified and must remain explicit in delivery reporting. Next action:
let the already-running shipping Graph deliver PR #1820, then verify merged
delivery and the installed job. Do not launch a second Graph from this step.
The refreshed view remains `scratch/demo.html`; the design and acceptance links
above remain authoritative. This headless pass checked the configured path and
recorded evidence; it did not repeat the earlier visual rendering review.
