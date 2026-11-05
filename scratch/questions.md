# 337044 questions and assumptions

- **Dependency order, for Jack Heart.** The brief deletes `is_additive` after 843f5c
  puts the rule in `check_migrations.py`. On 2026-11-05 (realign) that rule exists on
  843f5c's branch, unlanded; if this lands first, nothing in CI stops a direct
  `DROP COLUMN` until it does. Assumption: land in either order and rebase the second;
  the gap is days of CI measurement, not a store risk. 843f5c was only read, not steered.
- **`reader_floor` writes stop.** A NULL row reads as "names itself" in shipped builds
  (`applied_floor`), identical to writing the migration's own name, so NULL is harmless
  to them and this build reads nothing. Consequence either way: builds already installed
  keep their floor check, so the first migration after this lands refuses them once,
  with the old message. Writing the previous row's floor instead would admit them, at
  the cost of a dated compat write. Assumption: follow the brief (no write).
