# Open Questions

*Questions resolved in `scratch/rust-data-backend.md`:*
- ~~Tenancy model~~ → `tenant_id` + `project_id` (flat hierarchy)
- ~~Event retention defaults~~ → Operator-defined via `LFD_EVENT_RETENTION_DAYS`
- ~~Migration tooling location~~ → `lfd migrate` subcommand in Rust daemon
