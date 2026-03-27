# Gate review: gstack stage 1 import

## What was implemented
- Added a Python converter that imports generated gstack `SKILL.md` files into `.lf/workstyles/gstack/steps/`, extracts the shared gstack voice into a built-in direction, and preserves converted metadata in loopflow frontmatter.
- Wired loopflow discovery to treat `.lf/workstyles/<name>/steps/` as a first-class skill source so `gstack:<step>` resolves in discovery, listing, and prompt loading.
- Added built-in `gstack` and `openclaw` directions and committed the initial 29-step gstack workstyle bundle.
- Polished the import output so converted steps now refer to loopflow step names and local workstyle files instead of original gstack slash commands and `SKILL.md` paths, and removed leftover retro analytics/eureka instructions that stage 1 is supposed to strip.

## Key choices
- Keep converting generated `SKILL.md` artifacts instead of templates. That stays aligned with the stage-1 design and avoids recreating gstack's resolver stack.
- Treat imported voice as explicit directions rather than hidden runtime state. That makes `-d gstack` and `-d openclaw` reusable outside the imported workstyle.
- Rewrite user-facing cross-step references during conversion. The imported prompts now say `gstack:eng-review` and `.lf/workstyles/gstack/steps/eng-review.md` instead of `/plan-eng-review` and `~/.claude/skills/gstack/.../SKILL.md`, which matches the shipped loopflow surface.
- Strip leftover analytics-only retro content at conversion time rather than documenting it away. Stage 1 should ship methodology, not gstack telemetry instructions.

## How it fits together
- `python/loopflow/workstyle/convert.py` parses each gstack skill, removes preamble/voice/wrapper sections, rewrites imported references, and writes loopflow-native step markdown plus `workstyle.yaml` metadata.
- `rust/loopflow/src/lf/discovery.rs` and existing step loading treat `.lf/workstyles/gstack/steps/*.md` like a prefixed external skill source, so `lf --list`, `find_skill`, and prompt loading all resolve the imported steps without special-case runtime logic.
- The committed `.lf/workstyles/gstack/steps/*.md` files are the generated artifact reviewers and users will actually run.

## Risks and bottlenecks
- Some imported steps still mention original gstack helper binaries such as `gstack-review-read`, `gstack-review-log`, and `gstack-config`. Those are not loopflow-native yet, so the remaining references are documentation/runtime debt for later stages rather than fully integrated behavior.
- The converter now performs more opinionated cleanup and reference rewriting. If upstream gstack changes section phrasing significantly, the cleanup rules may need another pass.
- Because the workstyle bundle is committed output, any future converter adjustment should be followed by re-running the conversion so the artifacts and converter logic stay aligned.

## What's not included
- No loopflow-native replacements for gstack helper binaries, dashboards, or review-log storage.
- No sync workflow or re-import command; stage 3 still owns repeatable syncing.
- No browser/runtime integration beyond preserving browser requirements metadata on imported steps.

## Validation
- `cargo fmt --check`
- `cargo clippy -- -D warnings`
- `cargo test --all`
- `uv run pytest python/tests/`
- `tests/e2e/test_smoke.sh`
- `target/debug/lf --list | sed -n '/gstack/,+31p'`
- `target/debug/lf-prompt --repo . --step gstack:office-hours --surface headless --lfdocs false --diff false --diff-files false | rg 'gstack:(ceo-review|eng-review|design-review)'`

## Done-when check
- `lf gstack:office-hours` path is resolvable via the workstyle source and the loaded prompt now points at loopflow step names.
- `lf --list` shows the 29 gstack steps under the `gstack` source.
- Python and Rust test suites pass after the converter cleanup and reference rewrites.
