# Preserve release ownership through manifest lockfile tools

## Remaining work

Keep the serial PR and Task open. Task revocation/compensation, notes agents and
source/worktree creation, reset and removal children still need complete ownership
propagation and survival proof. Historical telemetry prerequisite associations,
bounded current retry, dated repair ownership and closed-obligation continuation
remain open. The observed missing `agent_turns` scorecard table remains a
verification blocker; no Intelligence handoff was accepted here. Required UI-host,
public exact-tag smoke and two adjacent automatic settlements, including at least
one publication without manual repair, remain mandatory.

## Counterexample and implementation

Starting head: `0b8e756ab6ca7aaf8683f8df56c80c0887903d49`.
The new built-CLI regression reproduced a second release successfully tagging
`v0.9.2` after the original controller died while its Cargo launcher was paused.
The fixture also attempted checkout removal; the first assertion failure was
target exclusion, so the red result alone does not establish checkout behavior.

Release preparation now passes its held target lock and exact checkout lease
into the existing manifest updater. Its command inheritance reaches both
`cargo update --workspace` and `uv lock`. The standalone `release bump` caller
explicitly supplies no release capability. Both tools use the existing output
and error conversion. The conditions for updating a lockfile, tool ordering,
manifest selection and version rewriting are unchanged.

The shared updater remains the only implementation. No new lock, executor,
persisted field, ambient capability lookup or settlement writer was introduced.
This slice changes neither scheduled prerequisite policy nor product evidence.

## Focused proof

`surviving_release_lockfile_tool_retains_target_and_checkout` crosses Cargo and
uv with controller death and failed-launcher exits. It starts the built release
CLI in disposable repositories, pauses the selected tool launcher, then kills
and reaps only its own controller or observes the launcher failure propagate
through normal controller cleanup. A second release must defer and ordinary
checkout removal must fail while the surviving child holds the descriptors.

After the barrier opens, the child invokes real Cargo or uv in offline mode on
a dependency-free manifest. The fixture reads the resulting lockfile's new
version, then observes restored release access and successful checkout removal.
It also retains caller HEAD, branch, byte-identical index, staged/unstaged bytes,
an untracked file and an unpublished local commit. The caller's original
lockfile still contains its previous version.

Git, bare origins, tools, process boundaries and OS locks are real. GitHub is
simulated; a shell wrapper provides the explicit interruption barrier before
the real tool runs. The test proves protection through that wrapper's lifetime
and a successful actual lockfile update. It does not establish arbitrary tool
descendants retaining descriptors after their own parent exits, network package
resolution, provider-agent behavior, installed scheduling or either configured
automatic settlement.

## Review and validation

The simulated review followed both manifest updater callers and both lockfile
commands. Preparation and rebuild share the same updater; the callback borrows
the existing two owners, while standalone bump retains ordinary behavior.
Errors still propagate through `command_stdout`. Failed-launcher cases reach
the existing cleanup path and prove it preserves the child's checkout. The test
asserts actual lockfile content and caller state, not callback invocations.

- New four-case CLI proof: failed before repair with competing tag creation;
  passed after repair in 18.34 seconds.
- `cargo test -p loopflow --test release_tests release_bump_updates_cargo_toml -- --nocapture`:
  passed, 0.20 seconds.
- `cargo test -p loopflow --test release_tests release_run_reintegrates_a_dirty_existing_pr -- --nocapture`:
  passed, 4.49 seconds.

- `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings`: passed.

No executable or test change followed the final behavioral proof and static
checks. No affected-suite/full-CI gate, installation/sync, cron trigger,
production publication, PM assignment, PR publication, landing or Task completion
occurred.
