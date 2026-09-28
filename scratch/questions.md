# Session opening: assumptions and unresolved evidence

Kickoff decisions on 2026-09-28. These are implementation assumptions, not
additional approvals attributed to Jack Heart.

- Continue the existing Claude prevention. No Wave/PM lookup is needed to design
  this repository change; incident Task names identify prior evidence, not new
  coordination work.
- Automatic replacement covers positively absent history, including a readable
  empty file. A nonempty metadata-only or unknown file remains unresolved. This
  narrower recovery fixes all three recorded incidents while protecting history
  whose meaning the inspector cannot establish.
- A prepared boundary is already visible before native persistence. Release
  startup ownership once the client is registered, without calling that client
  resumable. Opening a still-starting client waits boundedly and leaves it alive.
  This intentionally replaces the old single startup/resumability predicate.
- Task claims embed the position version; the claim API forbids claiming human
  review positions. Retain existing unclaimed-position ordering for rebinding.
  A concurrent advancement/claim defeats stale recovery without seizing authority
  or requiring a new claim-transfer protocol.
- Old local managed receipts use their recorded account Home. An old ambient or
  forwarded receipt with unprovable storage location cannot authorize replacement.
  New private receipts record the effective history root to remove this ambiguity.
- Exact historical TUI bytes are not recoverable from old split-channel captures.
  Preserve their archived system/task content; new TUI captures record the actual
  seed. Do not rebuild failed launches from changed repository context.
- Native probes use disposable config/workspace directories, no injected
  credentials, disabled tools/MCP and print mode. They establish loader behavior,
  not an authenticated TUI or Loopflow end-to-end acceptance. The latter remains
  required after implementation; no production review should be used for it.

Still unknown: why the original three clients exited; whether desktop renders
the restored command; Codex/OpenCode persistence contracts; how future Claude
schemas affect recognition. None authorizes guessing or destructive recovery.
The design's conservative path preserves these uncertainties explicitly.
