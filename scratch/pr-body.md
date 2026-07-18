## Try it!

Inspect the new stable control surfaces:

```bash
cargo run -p loopflow --bin lf -- work --help
cargo run -p loopflow --bin lf -- launch --help
```

Exercise the Review handshake and both Wave delivery shapes:

```bash
cargo nextest run -p loopflow only_the_active_parent_run_can_steer_child_work
cargo nextest run -p loopflow seed_only_wave_services_child_once_without_advancing_background
cargo nextest run -p loopflow live_wave_preempts_background_for_child_and_preserves_playhead
```

Run the complete matrix:

```bash
uv run python scripts/test.py --all
cargo nextest run --all --no-fail-fast
```

Current result: Python, website, Swift, E2E, fmt, clippy, Swift boundaries, and
the Mac build pass. Rust is 1,814 passed / 1 failed / 2 skipped. The one failure
is the intentionally unmasked Task/Project controller boundary described under
**Not included**.

## Intent

Replace overlapping execution and interaction truths with one durable control
spine: stable Work and Epoch identity, Basis-fenced Steers, exact Run authority,
provider/process Launches, optional observed Turns, typed Waits, and derived
Review attention. Provider transports may differ; stale execution, recovery,
completion, usage, and parent scheduling must not.

## Assumptions

- User authority is constructed only by authenticated external entrypoints;
  agent entrypoints require one opaque active Run lease and fail closed without
  it.
- Provider acceptance proves only that a Send was observed. A later successful
  boundary Basis proves application.
- Provider transcripts and resume tokens are optional Launch hints. Durable
  Work truth, Steers, flow position, workspace, and external evidence are the
  reconstruction floor.
- Opaque providers must expose a containable process/tmux boundary and explicit
  handback; process exit alone is not success.

## Key decisions

- Deleted stored Review, Handoff, and ChildCommand aggregates rather than
  adapting them to the new model.
- Kept Review route separate from the current unanswered `attention_at`, so a
  parent reply parks attention without ending the interactive interval.
- Allocated one parent evidence revision when the child's next terminal Turn
  re-arms attention, making racing parent completion stale.
- Made Turn the sole additive usage grain and persisted root output for parent
  reconstruction.
- Asked the exact active Turn whether it can accept a live Steer; no static
  provider capability flag or caller-selected delivery policy remains.
- Refused to restore a Session interrupt fallback just to turn the remaining
  integration proof green.

## Not included

This PR is **not ready to submit or land**. Task and Project still run through
the legacy Session/body generation and `ChildWriteLease` controller. The
remaining failing test constructs that real split: the mirrored active Run has
no product Launch, so direct Run interrupt cannot find a boundary. The fix is
the one-piece shared `reserve | advance | stop` controller cut, followed by
deleting Session process authority.

The same final cut must move unpublished `0.11.029`–`0.11.035` migrations into
the dependency-ordered draft contract. Rust is currently 122,944 Tokei code
lines, 1,125 above the acceptance ceiling; deleting the duplicate controller is
expected to pay that down. OpenCode Task/Project usage normalization and
credentialed provider recovery drills also remain outside this checkpoint.
