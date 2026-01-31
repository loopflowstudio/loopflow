# Open Questions

*Questions resolved in `scratch/rust-data-backend.md`:*
- ~~Tenancy model~~ → `tenant_id` + `project_id` (flat hierarchy)
- ~~Event retention defaults~~ → Operator-defined via `LFD_EVENT_RETENTION_DAYS`
- ~~Migration tooling location~~ → `lfd migrate` subcommand in Rust daemon

*Open questions from UI pass (2026-01-31):*
- Should the new "Abandon" action also remove the worktree on disk, or only delete the wave record?
- Should commit history be shown relative to `main` or the wave's base branch when available?
