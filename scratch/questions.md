# Open questions / follow-ups

- Docker executor currently fails `fork(select: all)` runs intentionally to avoid unsafe shared-workspace concurrency. Follow up with isolated per-branch Docker workspaces for fork branches.
