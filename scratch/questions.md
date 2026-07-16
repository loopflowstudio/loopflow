# Open questions — ENG-20

Headless kickoff; each resolved with a judgment call and noted here.

## Resolved by assumption

**Does `CiFix` carry `Vec<CiCheck>` or `Vec<String>`?**
Chose `Vec<CiCheck>` (name + log URL). The done-when says "failed checks"; the
seed needs URLs, and re-reading them from the observation at boot is the drift
this change removes. `CiIncident.failure_set` stays `Vec<String>` — its identity
hash is over names only, and that stays the dedup key.

**Enqueue while a body is live, or keep the `!is_process_active()` guard?**
Kept the guard. Enqueueing against a live body would permanently `Supersede`
that failure set even if the body then died without fixing it — a stranding
hole against the KR. The `Superseded` arm in `absorb_commands` remains as a
race net, not the normal path. Reversible: dropping the guard later is a
one-line change if the complete-observation-record argument wins.

**Is a `CiFixArmed` event worth its schema surface?** — RESOLVED: no, cut.
I argued it was evidence rather than a second state model, and that
`CommandChanged` carries only `command_id`. Both true, and both beside the point:
every field it carried (`pr_number`, `head_sha`, `failing_checks`) already exists
on `CiIncident` as `pr_number`/`failed_head_sha`/`failure_set`, joined by the
`trigger_command_id` this PR fills in. `lf ci` renders the failure set with no
join at all, so there is no consumer that can read the event but not the
incident. It was a third copy of agreeing facts plus a Rust/Swift wire surface to
keep in lockstep, bought for convenience. Evidence is `CommandChanged{claimed}` +
`trigger_command_id` + `responded_at`.

**Where does `Uncertain` leave a ci-fix wake?** — RESOLVED: it cannot reach it.
Superseded by the `Claimed`-through-the-turn design. `Uncertain` is only reachable
from `Delivering`, and a `CiFix` never enters `Delivering` — there is no provider
call at arm to be ambiguous about. So the wave-memory strand ("Task body recovery
is gated on settled") and the `plan_body_recovery` → `NeedsInput` trap are both
structurally out of reach for this variant, rather than merely unlikely. This is
the KR's "zero durable commands orphaned 'uncertain'" satisfied by construction.

## Genuinely open — for review

**Should `ci_incidents.trigger_command_id` get an FK to `child_commands(id)`?**
It has none today (`0.11.024:18`), and adding one needs a migration — which wave
memory flags as an ordinal race worth avoiding for a nice-to-have. Left as-is.
The write order (create command → stamp identity) makes it sound in practice.

**Does `lf ci` want a `--unattributed` filter?**
The Measure section's jq one-liners do the job for now. If the weekly check
becomes routine, promote it to a flag. Not built.
