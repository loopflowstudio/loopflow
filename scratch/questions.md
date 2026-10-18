# Decisions

2026-10-18: No earlier design was present. The LOO-508 directive supplies scope.
Diagnostic files use Process IDs under the existing logs directory because
waiting launchers and terminal viewers do not own a unique Session capture.
The scoped descriptor handoff includes raw parent/helper output as well as tracing.
