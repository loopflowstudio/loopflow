# LOO-978 verify, 2026-11-18

Outcome: verified, with one follow-up (Task 49476c: sample the live Machine after the next `lf install`).

Merge 5de899f78f (PR #1979). `git diff HEAD origin/main` is empty at fd2bda372, so the release build stamped `0.16.0+fd2bda372` is main's tree. Base: installed `lf 0.16.0+7bb879aff`. Private Machines under `/tmp/lf-v978/{new,old}`, 30 Tasks, scripts from `git show 2f7aba5255:scratch/qa/`.

Post-merge CI for 5de899f78f: `CI` push run 44429463373 success (rust-test skipped on push; merge_group run 44428077417 ran it, success). `macOS on main` and `Packages (main)` were still in progress when read.

| Check | Command | Result |
|---|---|---|
| Computed once per file change | `cargo test -p loopflow-execution --lib -- configuration_changes_reach_an_existing_subscription readings_share_resolved_settings personal_settings_reach observe::tests::` | 19 passed |
| `wave_setup_tests::` | `cargo test -p loopflow --test integration wave_setup_tests::` | 8 passed |
| Edit, add, remove, invalid YAML, bad value, worktree override, `--setting` beside a served read | `python3 scen.py new` and `old` over each server's socket | every served answer identical; only difference is the bare CLI repeating a bad-file warning fewer times |
| Invalid file still reports | same run, steps 7 and 8 | `warning: …config.yaml:6: skipping malformed YAML entry…`, `…:2: skipping agent: invalid value` |
| `--setting` never shared | step 11 | bare `--setting agent=codex:gpt-5` prints `codex:gpt-5`; the served read after it still says `claude:sonnet` |
| `lf config settings --explain` | run on main's build | prints selected and fallback candidates with their files; with `--setting`, `agent codex:gpt-5 Command line` |
| `validate_value` gone per read | `python3 load.py <tag> 20 s` (8 subscribers, a write every 0.1 s, `sample <pid> 5`) | below |

| | base | main |
|---|---|---|
| server CPU | 177% | 94% |
| samples under `settings::validate_value` | 4,225 | 15 |
| samples under `resolve_with` / `resolve_layers` | 5,511 | 23 |
| samples under `SkillCatalog` | 0 | 0 |
| `wave.plan` median | 300 ms | 234 ms |
| server stderr lines over the scenario | 76 | 6 |

Not checked: the real Machine with about 40 live agents (needs this build installed; not installed over live Sessions), Desktop itself.
