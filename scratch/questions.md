# Open questions / follow-ups

- Should `RepoState`/store extraction into `LoopflowCore/State` be completed as a strict Stage 01 requirement, or tracked as an immediate Stage 01.1 follow-up?
- Should simulator validation docs/tests be updated to accept current runtime names (iPhone 17 / iPad Pro 11-inch M5) when older runtime labels are unavailable?
- Should docker-dependent Rust tests skip automatically when `/var/run/docker.sock` is unavailable, or stay as hard failures outside CI docker environments?
