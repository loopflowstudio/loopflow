# 4b3e57: gate and implement run only reachable suites, one build at a time

Status: implementing (accepted direction: Jack Heart, 2026-10-27, in the Task brief).

## Approach

- sccache runs Machine-wide from `~/.cargo/config.toml` (Jack Heart,
  2026-10-27); the repository sets no wrapper because CI has none.

- Skill frontmatter `docs:` (string or list) names docs to include, resolved
  exactly like `--docs` and deduplicated with ambient docs. `Skill.docs` is
  parsed once in the catalog; prompt preparation appends it to requested docs.
- Builtin `gate.md` / `implement.md`: generic verification principles only.
  Builtins never use `docs:` or name repo-relative docs (Jack Heart, 2026-10-27):
  they run in repositories without those files, so they stay self-contained.
  `docs:` is for repository skills under `.lf/skills/`.
- `.lf/skills/gate.md` / `.lf/skills/implement.md`: Loopflow copies that add
  `docs: scripts/testing.md`, the one shared command list.
- TESTING.md deleted; person-facing content moved to owning module READMEs.
- AGENTS.md development block leads with `scripts/test.py`.

## Delete — do not maintain

- `TESTING.md` (content relocated, see below); references in README.md,
  AGENTS.md, `.lf/config.yaml`, `docs/architecture/delivery.md`,
  `release/UI_HOST_GATE.md`, `swift/README.md`, `scripts/check_architecture.py`,
  builtin gate.md, Wave memory (Jack Heart authorized, 2026-10-27).
  Shipped release notes keep their text.

## Relocation map

| TESTING.md section | New owner |
|---|---|
| CI, caches, Dependabot, nightly, scratch-clear | `.github/README.md` |
| runner, resource envelope, network isolation, installation harness | `scripts/README.md` |
| Rust, boundary checks, fixtures | `rust/loopflow/tests/README.md` |
| Swift, Loopflow UI | `swift/README.md` |
| Website | `website/README.md` |
| E2E | `tests/e2e/README.md` |
| Instruction decisions | `scripts/evals/README.md` |

## Remaining

- Before/after: rerun the gate/implement time queries after landing; baseline
  in the Task brief (gate 52.8 h / 204 steps; implement 391.6 h over 25 days).

## Concept review (2026-10-27, headless)

Model holds: actions (plan → focused test → affected suites once) map to one
runner and one command list; `docs:` is the existing `--docs` resolver reached
from skill frontmatter, joined where the skill is first known (by name in
`gather_context`, resolved in `prepare_prompt`; the paths are exclusive, not a
second resolver). No implement change.

Consequence carried, not an open choice for this Task: `.lf/skills/gate.md` and
`implement.md` are whole copies differing from the builtins by `docs:` and one
line each, so builtin edits stop reaching Loopflow's own runs until copied by
hand ("keep the two in step"). Jack accepted the copy override; a skill that
extends a builtin with frontmatter only would remove the mirror, but that
changes skill overriding for every repository and is a separate Task if wanted.

## Checks

- `cargo test -p loopflow --lib -- docs_frontmatter skill_frontmatter_docs builtin_skills_include_no_repository_docs` pass on merged tree 52c8a3d5; `pytest python/tests/test_gate_bounded.py` 51 pass; `check_architecture.py` pass; implement golden regenerated. Realign 2026-10-27: no drift, no rerun.
- Gate 2026-10-27 (`scripts/test.py --reuse-passing --rust-filter '…catalog|prompt|builtins|golden_prompt|documented_commands'`): architecture, rust (122 focused + clippy), website, swift pass; python 547/548, the miss `test_swift_test_limit::test_hung_test_is_named_and_stops_the_run` is a load flake (untouched by branch, 3/3 alone); python-sandbox test passed alone.
- Gate 2026-10-27 after 212e1f0ec2 (skill wording + goldens only): `cargo test -p loopflow --test golden_prompt` pass (112 s, the existing golden render) and `--lib builtins::` 12 pass; other suites' inputs unchanged since the gate above, not rerun.
