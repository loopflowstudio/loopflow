# 337044 questions and assumptions

- **Dependency order, for Jack Heart.** The brief deletes `is_additive` after 843f5c
  puts the rule in `check_migrations.py`. On 2026-11-05 that rule is unwritten: the
  843f5c checkout has no `scripts/` change, and its plan still changes floor semantics
  (outcomes 2-4), which this diff deletes. If this lands first, nothing in CI stops a
  direct `DROP COLUMN`. Assumption: finish here, land after 843f5c's check, rebase.
  843f5c was only read, not steered.
- **`reader_floor` writes stop.** A NULL row reads as "names itself" in shipped builds
  (`applied_floor`), identical to writing the migration's own name, so NULL is harmless
  to them and this build reads nothing. Consequence either way: builds already installed
  keep their floor check, so the first migration after this lands refuses them once,
  with the old message. Writing the previous row's floor instead would admit them, at
  the cost of a dated compat write. Assumption: follow the brief (no write).
