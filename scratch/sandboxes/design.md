# Rented sandboxes as Machines, and Wave defaults that rent

LOO-527 (rewritten: Daytona and Modal replace RunPod) and LOO-542 (a Wave's
Tasks go to rented capacity), in one PR. Jack, 2026-10-26: "start running all
infra tasks in loopflow, and all etude and manabot tasks, on on-demand cpus",
closed when the Task completes, after a 15-minute grace.

## Demo

```bash
doppler run -- lf ondemand add code --provider daytona --vcpus 4 --memory-gb 8 --max 2 --max-usd-per-hour 0.40
doppler run -- lf machine use code infrastructure    # the Wave's new Tasks rent; its Session stays here
doppler run -- lf task run LOO-xxx                   # rents code-1, hands the Task off, runs there
lf ondemand list                                     # code-1 · running · holds LOO-xxx · $0.04 so far
# land LOO-xxx; 15 minutes later reconcile copies history up and deletes code-1
```

## Provider boundary

A provider creates a named sandbox with a hard lifetime and an SSH route,
finds it again by id or name, and deletes it. Everything after "answers on
SSH" is the ordinary Machine path shared with machines you own.

- `rental/sandbox.py`: one embedded bridge, run as
  `uv run --no-project --with <sdk>==<pin> python -c <script> <provider> <op>`,
  JSON in on stdin and out on stdout. It builds the image (an `lf` user, key-only
  sshd for Modal, git, gh, node + Claude Code, Rust, uv). `lf` itself is
  installed over SSH after boot, published or `LF_RENT_LINUX_BIN`, so images
  never cache a stale lf.
- Modal: sshd behind an unencrypted TCP tunnel, `timeout` = lifetime.
  Daytona: its SSH gateway with a token valid for the lifetime, `os_user=lf`,
  `ttl_minutes` = lifetime, no auto-stop. The token rides in the Machine route.
- Price is computed from published per-second rates; cost is rate × elapsed.

Deleted with RunPod: `runpod.rs`, `guardian.py`, the CPU catalog and stock
checks, image digest pinning, the ready marker, `lf ondemand availability`.
Kept: identity written before the create, adopt-or-lose by name, give-back.

## Defaults that rent (LOO-542)

`lf machine use <pool-or-ondemand> [wave] | --repo` sets the scope's Tasks
capacity (`scope_defaults.tasks_capacity`), clearing a Tasks Machine at that
scope; the Session stays put. The nearest scope that sets either wins.
`lf task run` on an unowned Task whose scope names capacity places it there,
exactly as `--pool`/`--rent` would.

## Not in this PR

- Pausing a sandbox through review (Daytona stop/start, Modal snapshot):
  until then a rental lives until its Task completes, the grace passes, or its
  hours run out.
- Daytona's organization limit (4 vCPU / 8 GiB / 10 GiB) against loopflow's
  full test suite.
