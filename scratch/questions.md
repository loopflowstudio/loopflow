# Open questions / blockers — W2-218

## Blocker: `lf task acknowledge` is disabled by the incident under repair

`lf task acknowledge W2-218 --directive 1` exits 1:

```
database migration 0.11.009_profiles is unknown to lf 0.11.1
(latest known 0.11.012_provider_account_lifecycle) ... run lf doctor
Error: no Loopflow registry on this machine; start the owning Wave first
```

The installed `lf 0.11.1` cannot open `~/.lf/loopflow.db` because its ledger is
the exact divergent lineage this Task repairs. The acknowledgment is a store
write, so the incident blocks acknowledging the directive to fix the incident —
a live instance of the Why-7 blast radius (one incompatible ledger disables an
unrelated command).

**Decision:** proceed without the CLI ack. The directive is incorporated in the
design below; the ack summary is preserved verbatim in
`scratch/directive-ack.md` for the runner/supervisor to replay once a
store-capable binary exists. I will **not** mutate the live DB from this dev
worktree to unblock the ack (the directive forbids touching the live database
from development commands). The repair ships as tested code first; the live DB
is repaired by running that code, not by hand.

## Assumptions taken (reversible, simpler path)

- Convergence is **generalized** to any reordering of a known migration
  name-set rather than hardcoding the second lineage. If review prefers the
  conservative single-lineage hardcode, that is a smaller diff but leaves the
  next permutation unsupported — flagged, not blocking.
- Prevention uses **checksum + provenance columns + origin/main ordinal
  allocation + a CI convergence gate**, not a full rewrite of the ledger's
  identity from version-string to content-hash. The version string stays the
  human-facing identity; the checksum is the collision-resistant durable
  fingerprint recorded beside it. This is the "equally strong serialized
  reservation + writer provenance + convergence coverage" arm of the directive's
  menu, chosen over the "rewrite identity" arm for a smaller, safer blast radius.
