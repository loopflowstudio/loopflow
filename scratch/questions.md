# Open Questions

## Remote Concerto Phase 05

- Concerto still enters remote mode from an existing repo window. Is a repo-less startup flow required now, or can that wait for a follow-up UX pass?
- `ConnectionStore` owns persistence and trust/token storage, while handshake orchestration remains mostly in `RepoState`. Should handshake/reconnect ownership move fully into `ConnectionStore` in a follow-up refactor?
- TLS pinning trusts the first certificate without additional CA validation (TOFU model). Confirm this is acceptable for both `tls internal` localhost and public-host deployments.
