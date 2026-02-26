# Conformance traces and schema pinning

Picked from `wave/agentapi/02-hardening.md`.

## Problem

The OpenCode adapter shipped with unit tests for event mapping but no recorded-trace replay tests. The defensive multi-key fallbacks (`sessionID`/`sessionId`/`session_id`, etc.) are a temporary hedge against an inferred schema — they mask bugs by silently matching the wrong field.

## Goal

- Record real traces from a live OpenCode server
- Add replay tests matching the Claude/Codex conformance pattern
- Strip fallbacks to canonical field names once traces confirm the real schema
- Consider bundling fixed OpenCode binaries (or recorded trace fixtures) so CI can validate without a live server

## Done when

- Conformance replay tests pass for all three harnesses with recorded traces
- OpenCode defensive field-name fallbacks replaced with canonical names backed by recorded traces

## Open questions

See `scratch/questions.md` for the two unresolved schema questions referenced in the wave plan.
