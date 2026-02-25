# Open questions

- [infra-core-boundary-cleanup] ~~Should a follow-up introduce concrete backend adapters (`SqliteStoreBackend` / `PostgresStoreBackend`) and remove the remaining backend `match`/forwarding surface in `Store`?~~ **Resolved:** Yes, folded into Pass 2 scope item 6 (store backend-port cleanup).
- [infra-core-boundary-cleanup] ~~Should Docker startup-recovery tests be split into a Docker-required suite instead of soft-skipping when Docker is unavailable?~~ **Resolved:** Folded into Pass 2 scope item 3 (invariant-focused test expansion). Decision will be made during implementation.
