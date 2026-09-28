# Release installer test reconciliation

2026-09-28 · LOO-298 · Bounded contribution requested by Jack Heart.

The supplied CI failure identifies head `e13f29909`, run `36482349277`, job
`109130805119`. CI was not fetched or rerun here. The same failure reproduced
locally at the `--daemon-source` assertion.

## Change and rationale

Removed exactly two assertions from
`test_release_installer_uses_the_promotion_boundary_to_activate_the_binary`:
the required `--daemon-source "$daemon_src"` and
`--daemon-target "$daemon_dst"` strings. Cut I removed lfd; neither argument
belongs to the current installer.

Retained all three activation assertions: the downloaded `$src` invokes
`install promote`, it receives `--cli-target "$dst"`, and the installer does
not directly activate with `mv -f "$tmp" "$dst"`. No production code or
behavioral fixture changed, and no new test was added.

Read `TESTING.md`, `release/install.sh`, and the existing shell-installer tests.
Their candidate stub exercises promotion with temporary targets; curl/tar and
macOS verification/mount commands are stubbed. The existing CLI case checks the
complete promotion argument list, including `--sync-skills`; the digest case
requires failure before promotion; the app case checks app and legacy targets.
These provide the useful behavioral coverage without duplicating it in new
source-text assertions. Review found no reason to change those tests.

## Commands and results

Before editing, this exact command reproduced **1 failed in 0.04s**, at the
obsolete daemon-source assertion:

```sh
uv run python -c 'import os, subprocess, sys; env = {k: v for k, v in os.environ.items() if not k.startswith(("LF_", "LOOPFLOW_"))}; sys.exit(subprocess.run(["uv", "run", "pytest", "-q", "python/tests/test_release_automation.py::test_release_installer_uses_the_promotion_boundary_to_activate_the_binary"], env=env, timeout=120).returncode)'
```

After editing, this exact command passed **4 tests in 2.18s** (exit 0):

```sh
uv run python -c 'import os, subprocess, sys; env = {k: v for k, v in os.environ.items() if not k.startswith(("LF_", "LOOPFLOW_"))}; sys.exit(subprocess.run(["uv", "run", "pytest", "-q", "python/tests/test_release_automation.py::test_release_installer_uses_the_promotion_boundary_to_activate_the_binary", "python/tests/test_shell_installer.py::test_downloaded_candidate_owns_activation", "python/tests/test_shell_installer.py::test_digest_mismatch_aborts_before_promotion", "python/tests/test_shell_installer.py::test_macos_release_promotes_the_verified_app_with_the_control_plane"], env=env, timeout=120).returncode)'
```

Both pytest processes had inherited `LF_*` and `LOOPFLOW_*` variables removed;
fixtures then supplied their own temporary targets. uv reported an inherited
`VIRTUAL_ENV` mismatch and explicitly ignored it in favor of this project's
`.venv`. The subprocess timeout was 120 seconds.

Scoped `git diff --check` passed. Diff review confirms only the two obsolete
assertions were removed from the test. Other concurrent edits belong to the
managed worker and were untouched.

## Limits and handoff

This proves the retained source contract and three existing simulated installer
behaviors. It does not prove real download, signature verification, installation,
promotion, installed-Home compatibility, or hosted CI. No real downloader,
candidate binary, Home command, service, Rust/Swift build, commit, push, Task
comment, or worker launch ran. Only the assigned test and this note were edited;
both are left uncommitted for the managed worker.
