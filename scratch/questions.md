# Assumptions

2026-10-11: If ownership cannot be read promptly, leaving the native name
unchanged is safer than renaming a Loopflow-owned conversation. This is an
implementation decision, not a recorded approval from Jack Heart.

The initial idea of omitting Process history contradicted the repository's
one-Process-per-command contract. The implementation instead preserves the
existing noninitializing observer with a short receipt wait.

The unattributed records-read report cannot be correlated without its original
error or trace. No status-performance redesign is inferred from the timing alone.
