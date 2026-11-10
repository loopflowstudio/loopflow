CI run 42738938541 on b221b4bf25d4cdcc15cbb823c0374a2aa4d52849 failed the Wave command registry, the Session help assertion on `attaching`, and the two-person owner display. The matching library and integration shards passed on main (run 42731866815).

- `lf sync`: clean, HEAD unchanged.
- `cargo test -p loopflow --lib wave_resolution_matrix::registry_is_complete`: reproduced the missing `plan board` registration before the repair.
- `cargo test -p loopflow --lib wave_resolution_matrix::`: passed both tests after registration, including all seven environments.
- `target/debug/lf session --help`: confirmed `attach` appears only within the new card description's `attaching`; the assertion now matches the command name.
- `cargo test -p loopflow --test two_people`: passed unchanged (31.64 s); the CI owner-display failure did not reproduce. The claim, exchange and owner projection paths are unchanged by this branch; CI reruns it on the repaired head.
- `cargo test -p loopflow --test integration session_cli_tests::session_cli_uses_one_truthful_resolution_contract`: passed (5.97 s).
- `cargo fmt` and `cargo clippy --all-targets -- -D warnings`: passed.
- `git diff --check`: passed; CI owns the full matrix on the published head.
