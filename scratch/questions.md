# Open questions / assumptions

- The branch design doc (`scratch/mobile-lfd-discovery.md`) references `address` + `GET /api/v1/daemons`, but implementation follows the newer area doc contract with `url` + `GET /api/v1/daemons/discover`.
- Rust address detection currently uses `tailscale status --json` (with fallback to routable interface IP and bind address). If we want strict parity with the area doc, we should switch to Tailscale LocalAPI probing in a follow-up.
