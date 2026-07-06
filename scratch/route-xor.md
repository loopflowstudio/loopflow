path: done

The branch is ship-ready. The reported break ("`lf op pm show --wave
architecture` demands `asana_project` despite `pm.provider: linear`") was
diagnosed as operational, not a source bug: HEAD's provider path already honors
Linear from `GOAL.md`; the error came from a stale `lf 0.9.12` in the active
`Loopflow.app` bundle shadowing the fresh `~/.local/bin/lf` on PATH.

The scoped fix lives entirely in `scripts/install.py`: `_resolve_applications_dir()`
promotes `local --use` into the app bundle already on PATH (so a rebuild can't be
shadowed by a stale bundle), and the obsolete `uv tool install` global-wheel step
is dropped (the Python wheel has no console entrypoint). Verified: 12/12
install-script tests pass, ruff clean, and post-fix `lf --version` is 0.10.0 with
`lf op pm show --wave architecture` listing the Linear roadmap.

No provider-selection code and no lfd hard-cut route-contract code changed, per
scope. The design review, PR copy, and validation transcript are complete. Ready
to land.
