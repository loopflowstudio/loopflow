# Open Questions

- Concerto still enters remote mode from an existing repo window. Is a repo-less startup flow required now, or can that wait for a follow-up UX pass?
- `ConnectionStore` currently owns persistence and trust/token storage, while handshake orchestration remains in `RepoState`. Should handshake/reconnect ownership be fully moved into `ConnectionStore` in a follow-up refactor?
- TLS pinning currently trusts the first certificate without additional CA validation (TOFU model). Confirm this is acceptable for both `tls internal` localhost and public-host deployments.
