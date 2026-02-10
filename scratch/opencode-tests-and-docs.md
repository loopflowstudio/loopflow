---
status: done
phase: 4
---
# OpenCode Tests and Docs

## Problem

OpenCode integration (PRs 01-03) is implemented and passes 15 unit tests. But the test spec identifies two gaps, and the roadmap PR table still references deleted files. Close these gaps so `cargo test -p loopflow -- opencode` covers every spec item and the roadmap is clean.

## Approach

Add two missing test categories from the spec, then clean up the roadmap. No new features — this is pure coverage and hygiene.

### 1. `build_agent_command` integration tests (agent.rs)

The spec requires end-to-end tests that exercise `parse_model` → `build_model_command` → prompt append in a single call. No backend currently has these tests, but the spec calls them out explicitly for opencode:

```rust
#[test]
fn build_agent_command_opencode_default() {
    let config = LaunchConfig { auto: true, ..Default::default() };
    let cmd = build_agent_command("opencode", "fix the bug", &config);
    assert_eq!(cmd[0], "opencode");
    assert_eq!(cmd[1], "run");
    assert_eq!(*cmd.last().unwrap(), "fix the bug");
}

#[test]
fn build_agent_command_opencode_with_variant() {
    let config = LaunchConfig { auto: true, ..Default::default() };
    let cmd = build_agent_command("opencode:anthropic/claude-sonnet", "fix the bug", &config);
    assert!(cmd.contains(&"--model".to_string()));
    assert!(cmd.contains(&"anthropic/claude-sonnet".to_string()));
    assert_eq!(*cmd.last().unwrap(), "fix the bug");
}
```

These prove `build_agent_command` correctly wires `parse_model` → `build_opencode_command` → prompt for the two model string forms.

### 2. Malformed JSON stream test (stream.rs)

The spec asks for "Stream parser: malformed JSON → `Passthrough`". The existing `non_json_passes_through` test covers completely broken input (non-JSON). Add one test with valid JSON missing the `part` wrapper — a plausible opencode format deviation:

```rust
#[test]
fn parse_opencode_malformed_text_passthrough() {
    let mut parser = StreamParser::new();
    // Valid JSON with sessionID but missing part.text — no crash, passthrough or skip
    let line = r#"{"type":"text","sessionID":"ses_abc","part":{"type":"text"}}"#;
    assert_eq!(parser.feed_line(line), ParseResult::Skipped);
}
```

This is actually `Skipped` (not `Passthrough`) because the `"text"` arm catches it via the sessionID guard, then `parse_opencode_text` returns `None` (missing `text` field). That's correct behavior — the spec's "malformed JSON → Passthrough" is covered by the existing `non_json_passes_through` test for truly broken input, and this test covers the graceful-degrade path for structurally-off opencode events.

### 3. Roadmap cleanup

The roadmap PR table at `roadmap/opencode/README.md` links to `02-stream-parser.md` and `03-context-injection.md`, both deleted in this branch. Remove the dead links, mark PR 04 as done, and collapse the table since all 4 PRs are complete.

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Add golden prompt test for opencode | Tests prompt assembly, not agent wiring | Golden tests verify `gather_context` + `format_prompt`, which are agent-agnostic. No opencode-specific prompt assembly exists. |
| Add parity tests | Python test infrastructure doesn't exist yet | Parity tests (`test_prompt_parity.py`) aren't implemented for any backend. Adding them for opencode alone would be premature. |
| Skip `build_agent_command` tests | Other backends don't have them either | The spec explicitly asks for them. They're cheap (4 lines each) and prove the full model→command→prompt pipeline. |

## Key decisions

- **Add `build_agent_command` tests only for opencode.** The spec asks for these specifically. Other backends can add them later, but that's not this PR's scope.
- **Malformed JSON test uses `Skipped` not `Passthrough`.** The actual behavior for a well-typed but structurally incomplete opencode event is `Skipped` (the `"text"` arm catches it, `parse_opencode_text` returns `None`). The spec's "malformed JSON → Passthrough" describes truly broken input, which the existing `non_json_passes_through` test already covers.
- **Roadmap cleanup is in scope.** Dead links in `roadmap/opencode/README.md` are a maintenance hazard. The PR table references files we deleted.

Following the wave's design priorities: "Same fidelity as existing agents" — these tests ensure opencode has the same integration-test coverage pattern. "Minimal config surface" — no new config, just verification.

## Scope

- In scope: 2-3 new tests in agent.rs and stream.rs, roadmap/opencode/README.md cleanup
- Out of scope: golden prompt tests, parity tests, tool event parsing, duration computation

## Done when

```bash
cargo test -p loopflow -- opencode    # 17-18 tests pass (was 15)
cargo fmt --check                     # clean
cargo clippy -- -D warnings           # zero warnings
```

Every item in the spec's test list is covered. Roadmap has no dead links.
