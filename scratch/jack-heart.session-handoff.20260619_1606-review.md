# Gate review — session handoff

## What was implemented

Added the vendor-session handoff path for loopflow steps:

- `lf --tui` / `lf --ide` and `session.launch: tui | ide` choose whether interactive handoffs open in the terminal vendor TUI or the native vendor app.
- `lf op sync-skills` mirrors resolved steps into repo-local `.claude/skills` and `.agents/skills` by default, with opt-in global sync through `--global --yes` or an interactive confirmation.
- Named steps pre-sync repo-local skills and launch with a compact vendor skill seed: `/step` for Claude, `$step` for Codex. The seed carries only surface run-mode instructions, voice, and message context; the step body and branch orientation load from the synced skill.
- The loopflow operating manual moved out of assembled prompts and into this repo's agent doc (`STYLE.md`, reached through the `CLAUDE.md` / `AGENTS.md` symlinks). `LOOPFLOW_DOC`, `ORIENTATION_DOC`, and seed-level orientation were removed.
- Headless flow ops can run `op: sync-skills`, and the verification script creates a probe step, syncs it, and optionally invokes Claude/Codex live.
- Deprecated mobile pairing surfaces were removed in favor of the discovery/setup flow.

## Key choices

- **Skills are generated with provenance.** Synced `SKILL.md` files include `loopflow: true` and `loopflow-step`, so pruning only deletes loopflow-generated skills and leaves user-owned skills intact.
- **Global writes stay explicit.** Repo-local sync is automatic and safe; global sync requires `--global --yes` or a TTY confirmation.
- **Codex gets `$step`, Claude gets `/step`.** Codex's interactive composer reserves `/` for built-in commands, while `$step` works for Codex handoffs. Claude uses slash invocation.
- **Ambient context moved to the right layer.** Repo-operating rules live in the repo agent doc; branch orientation lives in the step bodies that need it; only voice remains in the launch seed.
- **Rust SQLite dependency is pinned back to stable-buildable versions.** `rusqlite` is held at `0.39` so CI's stable toolchain can build without the unstable `cfg_select` use in `libsqlite3-sys 0.38.x`.

## How it fits together

Step discovery still resolves the same loopflow step names. Before launching a named step, `run.rs` calls `sync_skills`, clears the assembled system prompt, and replaces the task prompt with a harness-specific skill invocation plus the small handoff context. The skill sync layer transforms step frontmatter and bodies into vendor-specific `SKILL.md` files under the Claude and Codex skill directories; launch code only decides the invocation sigil and where to open the session.

## Risks and bottlenecks

- Global skill sync has only been script-verified structurally here; live global vendor discovery still depends on Claude/Codex behavior outside this repo.
- Namespaced skill invocation (`/gstack/office-hours` for Claude, `$gstack/office-hours` for Codex) relies on vendors accepting nested skill directories.
- `npx/*` and `rams/*` remain on assembled prompts until external skill namespace semantics are designed.
- Directions removal is intentionally deferred; the branch still carries existing direction machinery.

## What's not included

- Automatic model invocation by skill description.
- Direction DTO/config removal.
- Flow-as-skill conversion.
- Concerto UI affordances for opening vendor apps beyond the CLI/deep-link handoff.

## Validation

```bash
cargo fmt --all -- --check
cargo test -p loopflow launch_prompt
cargo test -p loopflow golden_prompt
cargo test -p loopflow context_tests
cargo test -p loopflow skill
```

Passed during this gate on 2026-06-24.

```bash
cargo clippy --all-targets -- -D warnings
uv run python scripts/verify_skill_sync.py
```

Passed during this gate on 2026-06-24.

Previously passed in this branch gate on 2026-06-20:

```bash
.venv/bin/pytest python/tests/
tests/e2e/test_smoke.sh
.venv/bin/python scripts/verify_skill_sync.py
.venv/bin/python scripts/check_vendor_session_launch.py
swift test --package-path swift
.venv/bin/python scripts/check_swift_multiplatform_boundaries.py
```

`cargo test --all` and `uv run pytest tests/e2e/test_api_smoke.py tests/e2e/test_concurrent_clients.py -v` were attempted in the earlier gate. Both were blocked in that sandbox by `PermissionDenied` on local listener creation (`127.0.0.1:0` / Unix listener); `cargo test --all` reached 918 passed, 2 ignored, then 53 listener-binding tests failed, and the API e2e suite failed during `LfdRuntime` port reservation. Re-run those two commands in a normal local or CI environment before landing.

`scripts/verify_skill_sync.py --live` was not rerun during gate; the non-live probe verifies generated files, and the branch notes record prior live Claude/Codex sentinel verification.
