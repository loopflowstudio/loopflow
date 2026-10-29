# Demo: LOO-669 slice 3 — regressions hold stable, `lf release status` names the streams (2026-10-29)

Stand-in demo for Jack Heart (headless; no one present). Design: `scratch/release-streams.md` Build order slice 3 and its concept-review addendum.

## Observed

- Live, read-only: branch `lf release status` (fresh `LF_HOME`, so no draft migration touched the real store) against loopflowstudio/loopflow printed
  `Preview: v0.15.0 (published 2026-10-28)` and `Stable: v0.15.0`, then workflow, notes and GitHub Release lines. Matches reality: v0.15.0 is both newest and Latest; nothing soaking, no hold.
- Fixture path (`cargo test -p loopflow --test release_tests -- stable status regression`): 9 pass, including
  `release_status_names_the_task_holding_stable` (`… holds v0.9.3`, `v0.9.2 soaks until …`),
  `an_open_regression_task_holds_stable_below_the_release_it_regressed_in`, and `daily_run_moves_stable_to_the_newest_soaked_release`.
- Not exercised live: marking a real regression (`lf task create --regressed-in`) would create a real Task and run a draft migration on the live store; the fixture covers it.

## Decision (stand-in)

Accepted. Slice 3 and the status view do what the plan says; the live output reads cleanly. No design change.
Open assumptions stay in `scratch/questions.md` (Linear mapping for `regressed_in`, no `Dev:` line).

## Next

Publish slice 3 (gate → PR), then slice 4 (`lf install` follows a stream) per Build order.
