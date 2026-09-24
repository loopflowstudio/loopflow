# Check repository-context changes

When changing shared repository discovery or command dispatch, run the existing
Wave resolution consumers alongside the global-command tests:

```bash
cargo test -p loopflow --test wave_resolution_tests --test wave_resolution_matrix --test global_commands
```

Registered Wave directories need not contain Git metadata for cached PM reads.
Preserve those fixtures: adding a Git repository would hide a context regression.
