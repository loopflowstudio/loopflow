# 337044 questions and assumptions

- **Dependency order, for Jack Heart.** The brief deletes `is_additive` after 843f5c
  puts the rule in `check_migrations.py`. On 2026-11-05 (realign) that rule exists on
  843f5c's branch, unlanded; if this lands first, nothing in CI stops a direct
  `DROP COLUMN` until it does. Assumption: land in either order and rebase the second;
  the gap is days of CI measurement, not a store risk. 843f5c was only read, not steered.
- **`reader_floor` writes carry forward (resolved by concept review, 2026-11-05).** The
  earlier assumption, no write, reversed: NULL refuses every installed build at the next
  additive migration, which main does not do, so it fails the brief's "harmless" test.
  Evidence and direction: "Next implement" in the working design. Jack Heart can overrule
  this by accepting one lockout in exchange for no dated write; the write comes out cleanly.
