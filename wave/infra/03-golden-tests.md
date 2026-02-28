# 03: Golden Test Maintenance

**Finish line:** Golden tests pass on main. An `update_goldens` command exists and is documented.

## Scope

**Update stale goldens.** 6 golden `.md` files in `tests/goldens/` are currently out of sync with the engine. Run the Rust prompt engine against the fixture repos and update the golden files.

**Create update script.** The golden test panic message references `tests/goldens/update_goldens.py` which doesn't exist. Write it — run `lf-prompt` against each YAML case, capture output, write to the corresponding `.md` file. Or implement as a Rust binary/test flag that overwrites goldens in-place.

**Add workflow tests to CI.** `test_full_cycle.sh` and `test_rebase_conflict.sh` are the most realistic CLI workflow tests and run only manually. Add them to the `e2e-smoke` CI job.
