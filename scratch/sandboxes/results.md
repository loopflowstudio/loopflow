# Sandbox trials

`uv run --with modal scratch/sandboxes/trial.py modal`, 2026-10-26, loopflow at a7481dffdc.

## Modal — 8 vCPU (4 cores) / 16 GiB, sshd behind an unencrypted TCP tunnel

| Step | Seconds |
|---|---|
| create (includes the one-time image build) | 69.4 |
| tunnel address | 1.6 |
| ssh ready | 2.8 |
| upload source (`git archive`) | 3.0 |
| cold `cargo build -p loopflow --bin lf` | 91.1 |
| filesystem snapshot | 20.0 |
| create from snapshot | 0.2 |
| ssh ready from snapshot | 3.5 |
| warm build after touching one file | 61.8 |
| terminate | 0.1–0.3 |

- SSH over Modal's TCP tunnel works with ordinary `ssh -p`.
- `nproc` reports 20; the request is a floor, and Modal bills max(request, usage).
  The $0.07 estimate assumes the request; the dashboard holds the real figure.
- `target/` after the cold build: 2.0 GB.
- The warm build is mostly the `loopflow` crate itself; prebuilt dependencies
  saved about 30 s.

## Daytona — 4 vCPU / 8 GiB (org default maximum), SSH through its gateway

`doppler run -- uv run --with daytona scratch/sandboxes/trial.py daytona`. Container
sandboxes cannot be snapshotted, so the second half stops and restarts the same sandbox.

| Step | Seconds |
|---|---|
| create (includes the one-time image build) | 64.5 |
| SSH token | 0.2 |
| ssh ready | 0.8 |
| upload source (`git archive` through the gateway) | 8.3 |
| cold `cargo build -p loopflow --bin lf` | 100.9 |
| stop | 2.1 |
| start the stopped sandbox | 1.1 |
| ssh ready after start | 0.8 |
| warm build after touching one file | 53.8 |
| delete | 0.1 |

- `ssh <token>@ssh.app.daytona.io <cmd>` runs non-interactive commands and carries stdin; it logs in as root.
- `nproc` reports 48; the 4 vCPU limit is presumably a cgroup quota.
- Estimate $0.02 for the run at the requested size. How a stopped sandbox bills is unverified.
- The org needed a default region set in the dashboard before any create.
- Afterwards: 0 sandboxes on either provider.
