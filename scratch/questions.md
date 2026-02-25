# Open questions

- Interactive session terminal status handling: this implementation advances the wave on both `Ended` and `Failed` session states. Confirm whether `Failed` should instead fail the wave run for harness/provider errors.
- Daemon restart behavior: session watchers are in-memory tasks. If `lfd` restarts while a run is `Waiting`, there is no rehydration path yet to reconnect waiting runs to existing session state.
