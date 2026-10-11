# LOO-412 CI repair — October 11

Jack Heart reaffirmed landing. Failed head `72b1e98dd6`, CI run 38154843137:
three Rust failures and one installation assertion. Synced main at `e7c9236e75`.
Repair fixtures for disposable Home, canonical Task/receipt fields, actual outages
and preservation after acquisition. No product behavior changes.

Checks: four repaired cases and two migration proofs pass in disposable Linux;
fmt, Clippy, Ruff and diff checks pass. Logs: `/tmp/loo412-ci-container.log`,
`/tmp/loo412-ci-adoption-recheck.log`, `/tmp/loo412-ci-clippy-final.log`.
Earlier adoption recheck exposed stale Team wording; the final recheck passes.
Hosted verification of the repaired head remains; auto-merge must be re-armed.
