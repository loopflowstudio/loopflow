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

**Is `CiFixArmed` a new event a "second state model"?**
Judged no — it is an event, not a state; no transitions, no queue. It exists
because the done-when demands "Task status and events expose which failure set
woke which body", and `CommandChanged` carries only `command_id`. If review
disagrees, the fallback is to drop the event and require a two-table join
(`ci_incidents.trigger_command_id` → `child_commands.kind_json`), which is
strictly worse to read at 2 a.m.

**Where does `Uncertain` leave a ci-fix wake?**
Left on the existing semantics deliberately. The `Delivering` window for `CiFix`
holds no provider call and is a few statements wide — narrower than any live-input
command's. Wave memory ("Task body recovery is gated on settled") records a real
strand under an open healthy PR, but that is a recovery-path bug, not this seam.
Not in scope.

## Genuinely open — for review

**Should `ci_incidents.trigger_command_id` get an FK to `child_commands(id)`?**
It has none today (`0.11.024:18`), and adding one needs a migration — which wave
memory flags as an ordinal race worth avoiding for a nice-to-have. Left as-is.
The write order (create command → stamp identity) makes it sound in practice.

**Does `lf ci` want a `--unattributed` filter?**
The Measure section's jq one-liners do the job for now. If the weekly check
becomes routine, promote it to a flag. Not built.
