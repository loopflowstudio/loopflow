# Open questions

- Daemon restart behavior: session watchers are in-memory tasks. If `lfd` restarts while a run is `Waiting`, there is no rehydration path yet to reconnect waiting runs to existing session state.
