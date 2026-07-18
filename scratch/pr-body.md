## Usage

```bash
lf work --help
lf launch --help

cargo nextest run -p loopflow only_the_active_parent_run_can_steer_child_work
cargo nextest run -p loopflow seed_only_wave_services_child_once_without_advancing_background
cargo nextest run -p loopflow live_wave_preempts_background_for_child_and_preserves_playhead
```

## Summary

Replace overlapping interaction and execution truths with a durable control
spine: stable Work/Epoch identity, Basis-fenced Steers, exact Run authority,
provider/process Launches, optional observed Turns, typed Waits, and derived
Review attention. Codex may live-send while Claude, OpenCode, and opaque TUIs
seed another boundary; durable outcome and fencing stay the same.

Task and Project still execute through the existing Session controller in this
landing. Every such body now registers its real process as a Launch under the
mirrored Run, so Run interrupt and steering never fall back to Session lookup.
A follow-up Task owns the one-way runner rewrite and Session deletion.

## Changes

- Delete stored `InteractionReview`, `InteractiveHandoff`, and `ChildCommand`
  aggregates and their commands, DTOs, stores, and Swift surfaces.
- Make Steer/Send/Basis authoritative with one opaque Run capability that
  fails closed when missing, stale, or stopped.
- Keep Review route open while pending attention parks; re-arm it once on the
  child's next terminal Turn and advance parent evidence once.
- Service direct input and oldest child Review before Wave/Project background
  work, preserving the background playhead for live and seed-only providers.
- Store root assistant output and normalized usage on Turn; derive monitoring
  and spend from Run → Launch → Turn.
- Register new, recovered, and migrated legacy bodies as product Launches and
  close those Launches when their process boundary ends.
- Remove retired controls from user docs, built-in skills, smoke tests, and
  fixtures.

## Rebase decisions

Current main's `task reconcile` repairs orphaned ChildCommands and an
unincorporated directive completion gate. This branch removes both underlying
concepts: no ChildCommand can orphan, and Task completion reads Review/Basis
rather than directive acknowledgement. Keeping the command would recreate a
deleted compatibility model, so the architecture supersedes it while retaining
main's Linear attachment, migration release boundary, promotion fence, and PM
fixes.

## Follow-up

Rewrite Task and Project execution through shared Run `reserve | advance |
stop`, then delete `ProjectSessionStatus`, `TaskSessionStatus`,
`ChildWriteLease`, body generations, legacy authority env vars, duplicate
runners, and Session lifecycle/process schema. That Task also owns keeper
recovery, the final schema draft, zero Session-controller references, and the
121,819-line ceiling.
