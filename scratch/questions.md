# LOO-809

At November 10 18:20 UTC, `lf open --host desktop --file docs/server.md --file
rust/loopflow/src/server/README.md` reported both files opening in Loopflow
Desktop. At 18:56, 19:33, 20:10, 20:47, 21:23, 22:00, 22:37, 23:14 and 23:51 UTC,
and November 11 00:26, 01:02 and 02:12 UTC, the same command failed:
`Error: Desktop did not answer within 10 seconds. Inspect again before retrying.`
Visual inspection remains for QA. No product question is open.

At November 11 02:49, 04:16, 04:53 and 05:28 UTC, the same command returned:
`No Desktop Files view attached; nothing opened.` Both docs remain at their
paths; the existing text needs no further change in this docs step.

Earlier opening results and exact errors are in
`811c6b703480f05513897ea264b44b2ecd72aeaf:scratch/questions.md`.
