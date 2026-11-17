
- ci-fix 2026-11-17: locally, under a full parallel `task_graph_launch_tests::` run, `successful_final_graph_keeps_arrival_when_completion_fails_and_never_replays` and `task_ship_defers_to_a_live_ship` each failed once and passed alone; CI did not fail them on e0fe42d795. Not investigated.
