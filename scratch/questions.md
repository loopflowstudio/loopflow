# Open questions — W2-129

## Directive v2 acknowledgement could not be recorded (blocked on W2-130)

`lf task acknowledge W2-129 --directive 2` fails on both the `lf` on PATH
(`~/.local/bin/lf` → `~/src/loopflow/local-bin/lf`) and this branch's own
`target/debug/lf`:

```
WARN loopflow::store: local store is incompatible … path="/Users/jack/.lf/loopflow.db"
Error: no Loopflow registry on this machine; start the owning Wave first
```

Directive v1's acknowledgement went through earlier in this same session, so the
registry broke underneath the run — the same executable/registry provenance
failure the wave thread already fed to **W2-130**. Not repaired here: the fix
belongs to W2-130, and deleting `~/.lf/loopflow.db` would destroy the real
ledger to unblock a receipt.

**Assumption, proceeding:** directive v2 changed nothing about the plan (it
confirmed the audit projection and forbade rebase/land/perf claims, all of which
this branch already honours), so the unrecorded receipt costs no work. Re-run the
acknowledge once W2-130 restores the registry.

## Live dogfood still owed (blocked by the same failure)

The design doc's last verification item — read the live product wave through both
surfaces — needs a served wave, which needs the registry. The pursue skill
forbids booting a server to get one. The hierarchy is instead validated against a
realistic long-running task transcript in both renderers (Swift and Rust tests).
Do the live read when W2-130 lands.
