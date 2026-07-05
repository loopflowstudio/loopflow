---
requires: wave/goals/wave-agent-follow-ups.md, the blast-radius map (agent report, 2026-07-05)
produces: the lf / lfd / lfdb collapse — decisions, cut list, sequence
---
# The collapse: lf does, lfdb remembers, lfd relays

Executes the architecture item "Collapse lfd/lfq into lf; shrink lfd to a
guarded subscription server," under waves-outward. All decisions ratified
(Jack, 2026-07-05).

## Target shape (revised 2026-07-05: the filesystem is the database)

**The trinity (Jack):** the filesystem is the database; lf is the hands;
lfd is the GATEKEEPER — the machine's face to the outside world: discovery
("which waves exist here"), aggregation (union of the machine's channels),
relay (push outward), gate (auth inward), token custody. sqlite survives
only as the gatekeeper's PRIVATE REBUILDABLE INDEX over the filesystem —
derived state, crash-harmless, exactly what the doorman tests demanded.
"Yesterday's db-is-the-registry was a transitional stop, not the
destination."

Substrate mapping (what replaces each registry job):
- session/run lifecycle → publications on the run's own channel journal;
  the parent folds child journals (the observer reads files, not tables)
- one-brain / liveness → endpoint + pid files + probes (the floor exists)
- capacity → the parent listener is the single dispatcher; serialization
  by sovereignty, no lock
- runs/trace ledger → per-run journals + gatekeeper index (coordinate with
  the lf-meta run_events branch — same data, table costume)
- attention → messages on channels
- PR/queue state → git and gh are truth; derived
- wave rows → derived from wave/<name>/ markdown (always were)
- provider tokens → keychain/file custody under the gatekeeper
- discovery → the gatekeeper's name index (config or ~/.lf/waves.d/)

**Dispatch dissolves placement (Jack):** `--dispatch z` from inside channel
x.y mints child channel x.y.z (worktree = channel name) forked from HERE —
stack is just depth; pool is just not dispatching; fresh is dispatching
from the wave root. The placement trichotomy dies.

**lf d dies entirely (Jack):** the row is not the wave — reads are the
gatekeeper's job to serve; creation is writing markdown; update is editing
markdown (boot re-syncs derived fields); delete/gc → lf op wt + files.

Migration staging (honest): (1) run lifecycle publishes to channel journals
[depends on realign Wave B channels]; (2) readers move to files (observer,
context); (3) lf stops writing sqlite; gatekeeper indexes files; (4) tables
drop. Old target shape below stands where not superseded.

## Target shape (original)

```
lfdb        crate::lfdb (module now, crate when earned): backends, migrations,
            persisted domain types, the registry API. Written/read by lf
            directly. THE substrate. (Type split is already clean — persisted
            types vs wire DTOs, no circular deps; Event stays wire-only with
            http; TokenUsageReport placed deliberately.)
lf          the one binary. lf d <reads/writes as verbs>, lf q (dispatch),
            lf wave (crate::wave — out of the daemon's namespace), lf
            chat/memory/sub, lf op.
lfd serve   the doorman: READ routes + push relay + exec-lf for every
            mutation. Constitutional tests: route-around locally; writes only
            through the doors; crash-harmless; non-exclusive. "It may
            aggregate what anyone could read, relay what anyone could
            subscribe to, forward what anyone could say — never anything only
            it can do."
```

**Vocabulary (final):** a **channel** is a named stream — journal + thread +
subscribability. Every wave has one (its name); every work line gets one
(the ownership name IS the channel name: `goals.148e0e02`); channels are
cheap, transient, pen-held by the nearest running listener, folded upward.
**Wave** stays an identity (GOAL, MEMORY, home, tree position, mind-able);
a work line that grows a GOAL and memory has simply become a wave. Names
are topics, dots are the tree, subscription by name/prefix. No scope field.

## The cut (ratified as one block)

loop_ticker; activation queue + its tables (pending_activations,
activation_log, + stimuli/agents per map); watch/cron/ci-failure triggers;
the repair chain (verified: nothing else uses it); executor dispatch paths
(run_wave_handler → build_wave_agent_command → LOOPFLOW_OPERATING_PROMPT +
InFlightDispatch; the workers route; auto_create_pr; advance_branch;
recovery; summary_refresh); python lfq entire; the ghost surface
(unregistered secrets routes — Concerto's secrets UI 404s TODAY; Swift dead
runWave/ingestAndBuild/createAndRunWave; calls to unregistered routes;
POST /attention, /resolve, /check-ci — zero callers; LFD_DISABLE_TMUX; the
`loop` alias).

**Survivors extracted first:** the worktree/dispatch helpers lf q already
uses (create_run_for_placement, ensure_wave_worktree, tmux wrapper, etc. —
new home beside lf q or lfdb-adjacent); token_refresh (doorman keeps the
loop; lf keeps lazy refresh); boot registry-hygiene (session/ghost
reconcile + worktree janitor → doorman boot or `lf d gc`);
launch_palette_session only if Concerto terminals return (no live caller —
cut with the rest, revive from git if needed); wave_config.rs moves out of
http/routes (misfiled; registry + ops/pm depend on it).

## The seven calls

1. **Webhooks become speech.** check_run → doorman execs
   `lf chat --wave X "CI failed on PR #N: …"` — the mind dispatches ci-fix.
   PR-merged → execs `lf op queue reconcile`. push → speech too (Jack):
   "main moved: <sha>" to each affected wave — the mind decides to
   rebase/integrate with judgment, replacing the watch trigger's blanket
   activation. The trigger MACHINERY dies; watch/cron/ci-failure as
   CAPABILITIES all re-home (mind clock, webhook speech).
2. **Cron moves into the mind**: a third deadline in the select loop, read
   from the wave's cron rows in lfdb; fires a system turn ("cron due:
   <flow>"). The poller dies. External cron calling `lf chat` stays legal.
3. **Queue reconciler → `lf op queue reconcile`** (a verb; logic in lf);
   the doorman (60s + PR-merged webhook) execs it. Stacks keep working.
4. **The push bridge** (the map's biggest unstated item): once mutations
   leave the daemon's process its EventHub starves — the doorman grows a
   store-poll→event bridge (StoreObserver generalized machine-wide) so
   Concerto's live fleet updates survive.
5. **Concerto live mutations**: createWave + addRepo → exec-lf; auth OAuth
   flows port to lf (doorman keeps read status); iOS agent-session chat
   migrates to the wave server's door (remote rides the relay later).
6. **Auto-PR gets a structural analog (Jack).** A worker's PR must not
   depend on its prompt remembering `lf op pr`: the dispatch wrapper runs
   `lf op pr` itself on clean flow exit (opt-out flag for deliberately
   PR-less runs) — the executor's guarantee, daemon-free.
7. **advance_branch survives as a verb (Jack)**: `lf op advance` (or folded
   into `lf op next` semantics) — recurring-wave branch rotation invoked by
   a human or the mind, not an executor side effect.

## Sequence (hazard-ordered, from the map)

a. **Grow the missing lf verbs**: `lf d` reads (waves/runs/sessions/
   attention list/get), wave create/update/delete, repo add/remove,
   `lf op queue reconcile`, auth flows. (Doorman/Concerto keep working
   against old routes meanwhile.)
b. **Repoint callers**: scripts/lib scenarios, shell scripts, concerto-dev,
   Concerto's four live mutations, docs.
c. **Delete python lfq + the dead/mutation routes**; doorman keeps reads +
   /ws; wire the push bridge + webhook-as-speech.
d. **Kill the organs, drop the tables last** (old bundled daemons must
   never meet dropped tables — destructive migrations only after the fleet
   runs the new lfd).
e. **Renames ride separately** (in flight now): lfd::store → lfdb,
   lfd::wave → wave.

Postgres asymmetry noted: lf-direct migrate is sqlite-only; postgres
deployments need an explicit migrate story before lf verbs write there.
Journal run-event ledger is sqlite-concrete — surface on lfdb's API or
document as sqlite-only.

## Phase 2 (designed, not in this PR): the mind extracts

The mind becomes its own lf process — the resident publisher in
`<repo>.<wave>`, tmux-attachable, vendor-free server. Turn deltas become
wire through a door (full DTO discipline — the journal's "not wire"
exemption ends for TurnStarted/Item/Finished); the mind's input is its
subscription (lf sub's second customer). Server = pure listener:
hear / check / fold / tell.
