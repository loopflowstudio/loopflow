# Open questions / assumptions

- The branch design doc (`scratch/mobile-lfd-discovery.md`) references `address` + `GET /api/v1/daemons`, but implementation follows the newer area doc contract with `url` + `GET /api/v1/daemons/discover`.
- Rust address detection currently uses `tailscale status --json` (with fallback to routable interface IP and bind address). If we want strict parity with the area doc, we should switch to Tailscale LocalAPI probing in a follow-up.
- `xcodebuild test -scheme Concerto` currently fails in local macOS CI parity runs during `ConcertoUITests` link (`open() failed, errno=1`). Swift package tests pass; unclear if this is an environment issue vs a reproducible project issue.
