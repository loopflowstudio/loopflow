# Restore Wave fleet and roadmap loading

## Finish line

The configured Dev Mac app reads the same selected control Home as the `lf`
process that launched it. A cold launch shows the populated Wave roster and
roadmap, Activity leaves its loading state, and a failed first read shows the
subprocess reason without also claiming that the fleet is empty.

Proof must exercise the Mac app's production `RegistryQuery` through the real
launch environment and selected machine entry gate. Fixture-only Swift decoding
and a healthy shell `lf` do not count.

## Observations

- The selected control store is
  `~/.lf-dev/installed/local-04115a69e0c34b198bf110976b32f390/loopflow.db`.
  `lf ls --all --json` returns five Loopflow Waves; `lf roadmap --all --json`
  returns populated product planning.
- The Dev app bundles a validation-only release-provenance `lf`. With the
  LaunchServices environment produced by `scripts/loopflow-dev.py`, it falls
  back to `~/.lf/loopflow.db` because `_app_environment` forwards only
  `LF_HOME` and `LF_DB_PATH`, not the selected `LF_CONTROL_HOME` and
  `LF_CONTROL_DB_PATH`.
- Against that fallback store, the bundled helper reproduces both reported
  failures: `ls --all --json` and `roadmap --all --json` exit nonzero with
  `no such column: retired_at`. The fallback database is at migration
  `0.12.12.001_release`; Wave retirement arrives in `0.12.13.001_release`.
- After forwarding the selected Home, the rebuilt bundled helper reaches the
  intended database but still cannot read it. The Home is owned by the active
  installed development `lf` and carries its draft frontier; the bundled
  release-provenance reader reports incompatible schema and then fails on
  `work_state` / `work_kind`. The machine entry gate at `~/.local/bin/lf`
  successfully resolves the matching active binary and reads this store.
- `LF_CONTROL_BIN` is not usable as the app reader on this machine: it points
  to a stale pre-command `~/.lf/bin/lf`. The machine entry gate, not this legacy
  inherited path, is the current install authority.
- The primary Podium model preserves read failures and their reasons. Its
  console nevertheless renders `No Waves found.` whenever the failed fleet
  read has no last-good value, contradicting the adjacent unavailable signal.
- A stale Xcode-built Loopflow process from 2026-07-21 is also running from
  DerivedData and has no bundled `lf`. It is evidence of the reported machine
  state, but changing or deleting that build is not required to restore the
  separately identified Dev app path.

## Rejected hypothesis

Preserving the complete selected-Home environment across `open --env` will make
the bundled helper read the populated control store it was launched to operate.
The first half holds; the second fails because a release-provenance helper is
not the binary that owns an installed development store.

## Restored model

Resolve `lf` from the GUI-enriched PATH first. `~/.local/bin/lf` is Loopflow's
machine entry gate, so it dispatches to the binary matching the active install
and store. Keep the bundled helper only as the no-install fallback. Preserve the
selected-Home environment so development launches retain their explicit
authority. Restrict fleet and Activity empty copy to successful reads.

## Restoration proof

- `uv run python scripts/loopflow-dev.py run` rebuilt, signed, and launched
  `~/Applications/Loopflow Dev.app` with the selected control Home preserved.
- The installed-bundle capture at `/tmp/loo-274-installed-restored.png` ran
  `~/Applications/Loopflow Dev.app/Contents/MacOS/Loopflow` with
  `LOOPFLOW_UI_TEST_MODE=live`, the real `LF_CONTROL_HOME` /
  `LF_CONTROL_DB_PATH`, and the machine entry gate first on `PATH`. After a
  40-second settle it showed five Waves and populated Work rows, matching
  `lf ls --all --json` and `lf roadmap --all --json` from the selected
  development store. It is byte-identical to the independent source-build
  proof (`sha256 7c6e6dd98991885511077c7b9d5d193578f037fd508d8e2bd93e5cf07860a714`).
- Activity left `Reading Activity…`. Its current wire mismatch surfaced as
  `Activity unavailable` with the exact missing-`run_id` decode reason, and did
  not render `No Activity in this window`. This distinguishes restoration from
  the reported indefinitely loading or plausibly empty surface.
- `/usr/bin/time -p lf activity --since 7d --limit 50 --json` returned 50 items
  in 6.67 seconds. The command completes with populated JSON; the visible Mac
  failure is a named DTO mismatch rather than an empty ledger.
- Focused proof: `uv run pytest python/tests/test_loopflow_dev.py` passed; Swift
  launcher and Podium model filters passed 24 tests. The documented signed
  `xcodebuild build-for-testing` gate compiled and signed the app plus
  `LoopflowUITests`, including `PodiumStateTests`.

## Review

- Reads and controls share `controlLfPath()`; there is no second Mac query
  implementation. The active machine entry gate owns installed-Home selection,
  and the validation-only bundle remains the offline fallback.
- No database migration or user-state edit was needed. The fallback and active
  stores remain intact, and the stale Xcode process was left untouched.
- Capture targeting changes only automated capture mode. Ordinary launches
  still restore the user's repo-scoped Sessions surface.
- The missing `run_id` Activity DTO mismatch remains deliberately visible and
  outside this restore. Repairing that wire contract is separate follow-up,
  not a reason to hide the fleet and roadmap recovery.

## Near-misses

- Migrating, rebuilding, or discarding the fallback `~/.lf` database.
- Using the bundled release-provenance helper as the peer for a draft
  development store.
- Treating authored `wave/*/GOAL.md` placeholders as proof that registry reads
  recovered.
- Showing a zero/empty fleet alongside an unavailable read.
- Capturing before the live reads settle. The legacy nanoseconds sleep returned
  early for long delays here; the duration-based sleep now holds the declared
  capture interval and returns on cancellation instead of taking a false frame.
