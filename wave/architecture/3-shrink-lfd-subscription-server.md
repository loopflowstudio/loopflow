# Shrink `lfd serve` to the subscription hub — the hard cut

**Finish line:** `lfd serve` is only a guarded subscription server: it exposes a
defined, guarded external interface, pushes the event streams Concerto needs
(live wave-status, terminal-output), and **execs `lf`** for any behavior. The old
HTTP executor and query API are deleted, not shimmed.

## Why keep any server at all

Subscriptions. CLI-over-ssh is request/response; Concerto's live status badges
and terminal-output stream are *push* — the one thing a transient `lf`
invocation cannot be. Everything else lfd does today (query API, executor tmux
launch, triggers, queue) is exec-behavior that has moved into `lf`
([[2-lf-d-namespace]], [[3-lf-q-collapse]]).

## Migration posture: hard cut, no compat

This is an internal system — no external DB/API consumers. Per CLAUDE.md, do a
hard, irrecoverable cut: drop the old lfd-owned store/executor HTTP paths
outright rather than carry compat shims or a migration bridge. Existing local
dbs/sessions are disposable. Do **not** preserve the old HTTP-executor launch
path once `lf` owns tmux launch + registration.

## Done when

- `lfd serve` serves only subscriptions + a guarded external interface.
- It execs `lf` for launch/behavior; it reimplements nothing.
- The HTTP executor and query routes are deleted from the tree (git holds the
  history).
- Concerto keeps exactly one lfd connection — the subscription stream — and does
  reads/actions through `lf d` / `lf q`.

## Depends on

[[2-lf-d-namespace]], [[3-lf-q-collapse]], [[2-session-registry]] — the hard cut
is only safe once launch, reads, and the run registry all live in `lf`/`lfdb`.
