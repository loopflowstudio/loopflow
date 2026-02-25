# Open questions

- Store boundary cleanup in this pass adds `SessionStore` and capability accessors (`wave_state()`, `execution()`, `sessions()`, `admin()`), but `Store` still uses backend `match` dispatch internally and call sites are not fully migrated to capability traits yet. Should we do a follow-up that introduces concrete backend adapters (`SqliteStoreBackend` / `PostgresStoreBackend`) and removes the remaining forwarding surface from `Store`?
- Docker tests now skip two startup-recovery cases when Docker is unavailable (to keep non-Docker environments green). If we want strict enforcement in local runs, should these tests be split into a Docker-required suite instead of soft-skipping?
