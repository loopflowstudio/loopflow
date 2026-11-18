# Open choices (LOO-938 kickoff, 2026-11-18, headless)

- Hub setting lives on the Machine (`lf machine hub mini`), not in
  `.lf/config.yaml`. Passed over: `tracker: { hub: <machine> }` per
  repository. Reversible.
- Should the hub also run the Linear/Asana `tracker_actions` drain, so only
  one Machine talks to each tracker (and webhooks land where the drain runs)?
  Left per Machine in this Task.
- First contact queues `create` for every unbound local Project/Task in a hub
  repository. Alternative: the cutover is by hand only and unbound rows stay
  local. Chosen for the 22 personal-ref records.
- Personal Git plans (`users/<key>`) have no hub equivalent: one plan per
  repository on the hub.
- mini's etude checkout and `lf` version were not verified (mini unreachable
  during kickoff).
- Concept review (2026-11-18): hub edits follow "the change sent last wins"
  and never come back `lost`; the kickoff's per-field seq conflict check was
  dropped. Reversible before release: a hub-side check could be added later.
- Concept review: the hub is not a `Tracker` variant and carries every
  `WorkValue` (Waves included), not the Linear action lens.
- Docs (2026-11-18): Git sharing let a coworker who cloned the repository read
  and write the plan with no account. The hub is SSH-as-you, so without Linear
  or Asana the plan is one person's. Docs say "To share a plan with coworkers,
  use Linear or Asana." Intended, or should a hub admit other people?
- Implement (2026-11-18): `lf machine hub --clear` deletes every
  `hub_sends` row (values saved here but never sent stay here only) and
  every `hub_plans` row; setting another hub keeps open sends and starts
  first contact with the new one. Reversible.
- Implement: the `hub:` key is new in `~/.lf/config.yaml`; a released lf on
  the same Machine rejects it as an unknown setting. Only `lf machine hub`
  writes it, after install.
- Implement: owners hard-deleted on the hub (Wave removal) leave their
  `plan_changes` rows; `plan.changes` skips them and other Machines keep
  their copies. Deletion that travels is still `task_deleted_at` only.
- Implement slice 2 (2026-11-18): "each `lf` command reads" is built as the
  Machine server exchanging after each command it answers, coalesced and
  quiet for 5 s after the last. A command run without the server only sends.
  Reversible: a periodic server-side exchange instead.
- Implement slice 2: the `two-people` World and its binary are deleted with Git
  sharing (no coworker sharing without Linear/Asana). Claims across two
  people's Machines are now covered only by the ownership tests over a hub.
- Implement slice 2: `lf session receive --planning-key` is gone, so a released
  lf moving a Task to a branch-built Machine fails on the unknown flag until
  both run the same release.
- Implement slice 2: `lf repo tracker --subscribe` exchanges when it reads,
  which is when the store changes, not on a clock.
