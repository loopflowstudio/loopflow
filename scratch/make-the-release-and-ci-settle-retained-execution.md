# Retained Exec completion boundary

## Direction and evidence — October 5, 2026

Jack Heart requested recovery of LOO-326 without fabricated success, raw-store
edits, lost history or weaker live/unknown process protection. The supplied
steer records PRs #1413/#1435 installed in v0.13.3 and both stopped Flows ended.
This pass confirms the installed CLI reports 0.13.3.

`lf monitor show 5f239ead-89f9-49c4-92c4-4c2f8b97ca94` retains the read-only
`task status LOO-326 --json` command, started at 1791175322, with no terminal
fields. No current runtime process receipt names this Exec. Its completed
caller Session does not establish child exit.

The local journal at
`.lf/journal/traces/da30b415-2918-45bb-b3a0-55877d8b7525/events.jsonl`
has two status-command starts before a completion and later commands in the same
trace. Events carry no Exec ID. None can safely be assigned as this Exec's
terminal receipt. The retained Exec began after the current boot.

## Remaining work

Recovery lacks an exact process identity or terminal observation. Preserve this
unknown outcome. An independently retained PID/start-time receipt could permit
the existing liveness reader to prove exit; a subsequent machine boot also lets
the existing pre-boot rule establish exit without rewriting outcome. Neither
has been established here. Rebooting the machine is outside this implementation
pass. Task completion also remains outside the active contribution.

Source inspection found three ways identity can disappear: terminal journal
emission removes the process receipt even when the ledger write fails; receipts
are keyed by reusable PID; monitor prune removes stale receipts. These are
demonstrated source paths, not a proven explanation for this specific lost
receipt. A prevention change must preserve exact identity through all three
paths and test failed terminal writes, PID reuse and pruning together. It cannot
retroactively recover the missing identity. No speculative partial repair was
implemented in this pass.

## Delete — do not maintain

No authorized deletion target yet. Any prevention design must replace the
loss-prone identity lifetime, preserve existing history, and retain live/unknown
blocking. Do not add a read-only-command exemption or infer child death from
Session closure, trace completion or elapsed time.

Checks: installed `lf monitor show` confirms unknown outcome; receipt/journal
inspection cannot establish exact exit; `lf monitor prune --dry-run --json`
reports zero errors and makes no changes. Prose-only reconciliation; no builds.
