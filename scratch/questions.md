# Open questions

## A2 step 7 — `Wave::repo()` accessor retained

The base (e65526ce) had already removed the flat `Wave.repo`/`status`/`iteration`/
`cycle_start_iteration` fields and `primary_repo()` (ancestor 8228a5dd), plus the
migration and store work. What remained was a repos-backed `Wave::repo()` accessor
(`self.repos.first().map(|r| r.repo.as_str()).unwrap_or("")`) used by ~27 single-repo
readers.

Decision: kept `repo()` as the single sanctioned "primary repo" accessor. It centralizes
the "single-repo need → `wave.repos.first()`" rule the task prescribes rather than
inlining that Option chain across every call site (which would be more verbose and
riskier than the accessor). Repo-*filter* sites were repointed to
`wave.repos.iter().any(|rw| rw.repo == repo)` per the multi-repo membership semantics.

If the reviewer wants zero single-repo bridge at all, inline `repo()` at its call sites
and delete the accessor.
