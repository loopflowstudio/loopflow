# Close the remaining direction D native gaps

Jack said "go for it" to these gaps from scratch/visual-study/polish/native/README.md:
1. Render the real 13-step builtin Feature Flow natively (catalogue from `target/debug/lf flow list --json`, or expand the builtin in a test fixture). Capture it at ~1500pt and ~1100pt, including the narrow three-row fallback. No clipping, and the tail through `pr land -c` must be visible.
2. Restyle the Metrics section to D's single table row (see polish/index.html ?v=d, surface=wave).
3. Restyle Comments (collapsed disclosure, count, Unavailable/Retry state) and the Monitor pane to D.
4. Capture a Session with the new breadcrumb (Wave / Task / Session name, membership chip), plus Wave/Task/Session in dark mode. Fix any dark-mode contrast issues you find.
5. Run the supervised Xcode fallback compile per scratch/implementation-cycle/final-build-plan.md (`uv run python` build_plan/run_plans for "loopflow" only). Report preflight failures honestly; don't bypass them.

The same constraints as brief-native.md apply: presentation only; no Rust/DTO/control/ownership changes; preserve other writers' edits (the parent tuple work); don't commit. Rerun the focused Swift suites from native/README.md once at the end. Put captures in scratch/visual-study/polish/native/ and update native/README.md with results, comparison against ?v=d, and remaining gaps.
