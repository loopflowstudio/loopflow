# lfd restart / orphan cleanup

Picked from `wave/agentapi/02-hardening.md`.

## Problem

`SessionRuntime` lives in memory only. Active sessions become orphans on lfd restart. Events survive in the store but sessions remain in `active`/`starting` state permanently. OpenCode adds orphaned `opencode serve` processes.

## Done when

- lfd restart doesn't leave orphaned active sessions or orphaned `opencode serve` processes

## Scope

Startup recovery pass: on lfd boot, scan the session store for sessions in `active` or `starting` state and mark them `failed`. For OpenCode, also clean up orphaned `opencode serve` processes.

## Context

- Session store persists events and session metadata across restarts
- `SessionRuntime` (harness + broadcast + seq counter) is in-memory only — lost on restart
- All three harnesses are affected, but OpenCode has the additional concern of orphaned HTTP server processes
- From README risks: "Active sessions become orphans on restart. Events survive in the store but sessions need a startup recovery pass to mark orphaned `active`/`starting` sessions as `failed`."
