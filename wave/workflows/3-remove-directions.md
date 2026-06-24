# Remove Directions

**Finish line:** `direction` no longer exists as a wave/config/DTO field. The
perspective text it carried has been redistributed — most into the step-skill
bodies it shaped, some into agent docs — and `-d/--direction` is gone from the
CLI. No code path assembles or injects a direction.

## Context

Deferred out of the steps-as-skills milestone (see
`release/unreleased/DECISIONS.md`, 2026-06-19 and 2026-06-24). Once steps became
vendor Skills and the assembled prompt retired, Directions lost their delivery
channel — they were always prompt-injected perspective. But removing them is a
**wire-format migration, not a flag deletion**: `direction` is threaded through
DTOs, SQL migrations, HTTP routes, and the Rust/Python/Swift mirrors. It needs
its own pass under the DTO fixture discipline (`tests/fixtures/dto/`, round-trip
tests in all three languages), not a cleanup tucked into a handoff PR.

## Redistribute, don't just delete

The direction *text* still has value — it just belongs where the perspective
applies, not in a separate orthogonal field:

- **Most direction text → the relevant step-skill bodies.** A perspective that
  shapes how a particular step is done lives in that step's `SKILL.md`.
- **Some direction text → agent docs (AGENTS.md / STYLE.md).** A standing point
  of view that should be always-on for a repo or wave lives in the agent doc.

## Machinery to delete

- `direction` config field, wave-YAML key, `-d/--direction` CLI flag
- `builtins/directions/`, the direction loader, the prompt-injection path
- DTO fields + fixtures, SQL migrations, Swift/Python wire mirrors

Scope is broader than it looks — ~50 Rust references plus the Python and Swift
mirrors and DTO round-trip fixtures. Migrate everything to the new shape; per the
repo style, no compatibility shim for the old field.

## Done when

- No `direction` field on any wave, config, or DTO; `-d/--direction` removed
- DTO round-trip fixtures updated and green in Rust, Python, and Swift
- Builtin direction text relocated into step-skill bodies or agent docs, with
  nothing silently dropped
- `cargo test`, `uv run pytest python/tests/`, and `swift test` pass with the
  field gone
