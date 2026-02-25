# Open questions

- [infra-core-boundary-cleanup] Should a follow-up introduce concrete backend adapters (`SqliteStoreBackend` / `PostgresStoreBackend`) and remove the remaining backend `match`/forwarding surface in `Store`?
- [infra-core-boundary-cleanup] Should Docker startup-recovery tests be split into a Docker-required suite instead of soft-skipping when Docker is unavailable?
