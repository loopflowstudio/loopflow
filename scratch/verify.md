# verify 4bd948 on main 7c76751875 (PR #2006, merged 2026-11-20T18:05Z)

- `lf config settings --all` in a fresh repo, isolated LF_HOME/HOME: `yolo true Built-in` (also with `-a agy`, `-a codex`)
- `--setting yolo=false` → `false Command line`; `LF_YOLO_OVERRIDE=false` → `false Environment`; `.lf/config.yaml yolo: false` → `false Shared project`
- `cargo test -p loopflow-execution --lib -- harness:: config::settings agent::session` (LF_* unset): 155 passed, 0 failed (launch matrix, all harnesses)
- Agent renamed to `agy` on main (`Agent::parse("agy")`, launch_matrix harness "agy")
- docs/config.md and docs/security.md describe the new default; config.md keeps "run it on a dedicated Machine, VM or container"
- Post-merge push CI for 7c76751875: CI success. macOS on main pending, Packages (main) queued at check time — not observed passing
