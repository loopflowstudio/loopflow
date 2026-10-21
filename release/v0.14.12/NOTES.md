# v0.14.12

<!-- loopflow:release-notes=narrative;gate=safe -->

v0.14.12 gives every Task one clear answer to "who is working on this, and where": a person on a Machine, or nobody. That answer can be claimed, handed to another of your Machines or released, and each change shows up in Linear or Git planning. The release also keeps Sessions that were already running able to finish their turns after an `lf` upgrade. It carries a store break: lf < 0.14.12 can no longer open the store (raised by: task_ownership). Upgrade every Machine that shares the store.

## A Task is owned by a person on a Machine

Before this release, three overlapping records tried to say who was working on a Task and where. They were a write-once `started_at`, Machine placement inherited through Wave, Project and Task, and a first-start admission that tied a Task permanently to the Machine that created it. None of them reached Linear, none could be handed off or released, and starting a laptop-created Task on mini needed an SSH connection back to the laptop. All three are replaced by one append-only ownership log. (#1610)

- **Claim, release, hand off.** `lf task claim` and `lf task unclaim` change ownership. `lf machine use <machine> --task <task>` hands a Task to another of your Machines and needs only that Machine's identity, not a live connection to it.
- **Claiming happens on its own.** `lf task run` and `lf task checkout` claim an unowned Task when they allocate its checkout.
- **Each command writes on the right Machine.** Within your own Machines, ownership commands run on the Machine that should record the entry, over `lf --machine`. Nothing is routed to another person's Machine.
- **Competing claims are visible, not destructive.** If two Machines claim, the later claim wins and nothing is cancelled. On its own Machine the earlier claimant stays the owner, `lf task status` reports "Also claimed later by …", and its live Sessions receive a steer.
- **Planning sees it.** Each change is posted as a marked Task comment, such as "Jack Heart claimed this on mini". In Linear, a claim assigns the Task to that Machine's Linear viewer and moves a backlog Task to started. An unclaim clears the assignee and leaves the status alone.
- **Ownership outlives completion.** Completing, abandoning or reopening a Task does not change its owner, so a Done Task shows who last held it.
- **Desktop.** Task rows show an owner line ("Jack Heart on mini" or "Unowned"), and Tasks are grouped as Owned or Unowned.

## Choose your Machine per Wave or repository

Placement used to be shared, inherited configuration. It is now a personal default that stays on your Machines. (#1610)

- `lf machine use <machine> <wave>` (or `--repo`) sets which of your Machines gets a scope's new Tasks and its Session. `--tasks` or `--session` sets only one of them. With neither flag, both are set.
- These defaults live in this Machine's store and are copied to your other added Machines. They are never committed.
- An unowned Task resolves through its Wave, then parent Waves, then the repository, then this Machine. A single-Machine setup needs no configuration.
- Moving the Session Machine completes the old Wave primary Session and starts a fresh one on the new Machine. Native provider history stays on the old Machine.
- `lf connect` and `lf open` route to your Session Machine when they resolve from context.

## Running Sessions finish their turns after an upgrade

Provider hooks call the installed `lf` entry point, and a Session keeps the hook callbacks that `lf` wrote when it launched. An upgrade that removes one of those callbacks, such as `__planning-check` or `__session-title`, used to make the call fail as an unknown command. At Stop, Claude reads that failure as "keep working", so the agent could not end its turn. (#1629, #1630)

- An unknown command whose name starts with `__` now exits 0 and prints a note on stderr telling you to restart the Session to refresh its hooks.
- Callbacks that still exist run as before. Unknown commands without the `__` prefix still fail; `lf no-such-skill` reports an error.

## Operational notes

- **Store break.** lf < 0.14.12 can no longer open the store (raised by: task_ownership). Upgrade `lf` on every Machine before running it against an upgraded store, including cron hosts.
- **Migration.** The `task_ownership` migration converts existing Started and placement facts into `recorded` ownership entries. Recorded entries are never posted to Linear or Git planning, so upgrading doesn't flood Tasks with comments.
- **Removed commands and records.** `lf task admission`, `lf wave place`, first-start admission (`tasks.admission_authority`, `task_admissions`), `work_placements`, the delegation WorkValues and the `started_at` triggers are gone. Scripts that call `lf wave place` should use `lf machine use <machine> <wave>`.
- **Known limits.** Desktop's Task detail doesn't show the full ownership log yet. Handoff to another person is not included; the owner unclaims and the other person claims. Editing the assignee in Linear creates no ownership entry.
- **Restart Sessions after hook changes.** A skipped hook does nothing until its Session restarts, and if a hook was renamed, the new name doesn't run in that Session either. The stderr note is the signal. `a_hook_left_by_an_older_lf_does_not_fail_the_provider_turn` and `hooks_from_an_earlier_lf_never_block_their_provider` cover the behavior. It hasn't been observed against a live Session across a real upgrade.
- **Earlier draft.** These notes replace the v0.14.12 notes from #1631, which were written before the ownership change joined the release and said there was no compatibility break.

## Small changes

- `docs/architecture-reference.md` now opens with the principle "intent flows down, execution facts flow up". The README, `docs/lf.md`, `docs/waves.md`, the architecture pages and the builtin skills now describe ownership instead of placement and admission.
- `lf wave rename` moved into its own module, with no change in behavior.