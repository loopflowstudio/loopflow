# Website CI repair

2026-09-28 · LOO-298 · Bounded contribution for CI checkpoint `e13f29909`,
run `36482349277`, job `109130805085` (reported 2 failures / 76 passes / 3 skips).

The security assertion is repaired and portable HTML is regenerated. **One
additional obsolete assertion remains outside this contribution's write scope:**
`website/tests/test_portable_architecture.py:15` requires `Home-local Run record`.
The current accepted architecture source no longer contains that text. The
renderer freshness check passes; the portable test now reaches and fails this
content assertion. Main should replace it with a current architecture concept
(for example the documented Exec, AgentSession and FlowSession owners), then
rerun that test. Restoring Run prose just to satisfy the assertion would contradict
the accepted model.

## Changes and ownership

- `website/tests/e2e/test_docs.py`: replace the retired `per-process control
  capability` assertion with the documented explicit SSH-agent forwarding option
  and its default-off statement. Keep the other forwarded-authority assertions.
- `docs/architecture.html`: generated only with
  `scripts/render_architecture_html.py`; no handwritten HTML changes.
- This note records evidence and the remaining handoff. All three files remain
  uncommitted.

Read `TESTING.md`, `website/dev.py`, `website/tests/conftest.py`, the relevant
website tests and `docs/security.md`. Source inspection of
`rust/loopflow/src/lf/commands/ssh.rs::run`, `reject_nested_ssh` and
`ssh_connection_args` confirms the retained path: nested SSH is rejected before
transport, the option adds `-A`, and broker socket forwarding uses owner-only
permissions and fails when forwarding cannot be established. This is source
evidence, not a live SSH/security proof. No listener or daemon model was restored.

Main retains Rust, Swift, schema, builds, Git and source Markdown ownership.
The release-test contributor's `python/tests/test_release_automation.py` was not
edited. No additional workers, provider calls, real Homes, installation, Git
mutations, Task comments or GUI capture executables were used. The existing
website fixture ran its own headless Playwright Chromium and temporary server;
its usual ignored docs synchronization is the only generated website cache work.

## Commands and results

Each subprocess below ran with all inherited `LF_*` and `LOOPFLOW_*` variables
removed and `PYTHONDONTWRITEBYTECODE=1`. The pytest subprocess also removed
`VIRTUAL_ENV`, selecting the website environment explicitly. The outer uv command
reported that an inherited environment belonged to another checkout and ignored
it; it selected `website/.venv`. No Loopflow executable ran.

From `website/`, using `uv run --project website --extra test python` from the
repository root as the environment-scrubbing subprocess wrapper:

| Command | Result |
| --- | --- |
| `uv run python dev.py sync-docs` | Exit 0 |
| `uv run python ../scripts/render_architecture_html.py --check` before generation | Exit 1: HTML stale |
| `uv run python ../scripts/render_architecture_html.py` | Exit 0: generated HTML |
| `uv run python ../scripts/render_architecture_html.py --check` after generation | Exit 0 |
| `uv run --extra test pytest -p no:cacheprovider tests/e2e/test_docs.py::test_docs_security_page_states_forwarded_authority_guarantees tests/test_portable_architecture.py -q` | **1 passed, 1 failed**, 1.49 s; security passed, portable failed only at the obsolete content assertion above |

The wrapper bounded synchronization/rendering subprocesses at 60 seconds and
pytest at 120 seconds. The fixture synchronized canonical docs again before
starting its server. No full website suite, Rust/Swift suite, hosted CI, rendered
desktop or configured-provider acceptance is claimed. No extra broad rerun is
needed before the remaining assertion changes.

Review: the changed test checks a retained user-facing option rather than a
deleted control service. HTML was generated from unchanged canonical inputs,
and its exact freshness proof is green. The remaining failure is preserved,
not hidden by weakening the portable test or modifying source documentation.

## Generation inputs

SHA-256 values were identical before sync and after generation. The architecture
entry and CSS are direct document inputs; area/reference hashes additionally
record the source-doc snapshot for main's continuing work. Website rendering
code/content are included so later generation drift is attributable.

```text
104e55ef55038664eab364b2f612229b8bf4591ddfffc9339a046830bcfa2fda docs/architecture.md
a23ec45c08bb5f6e9e6626a4db78adda450ec51c33a5452d1dfb528975804113 docs/architecture.css
a5e252ef51a808313bca48de64120fafae0dba3891090d5c5e5f4fc4f3f23749 docs/architecture-reference.md
cdcd90c75a810c11bd37b905f4f4bc28980a554b19bfdc76c5074ac140133827 docs/architecture/codebase.md
599de93812166b6290b1f806abb63614090151922fec313001e90b69a1b36c89 docs/architecture/data.md
4d4e9e47b96a3f3a2d324dd51ab59be4449cada019d7000b668bda339f6b28a7 docs/architecture/delivery.md
49a0113f19e88c9ffbf98456e11ae91ff2c80135d9842e55d4b31a34bf31894f docs/architecture/execution.md
d88a5e3992d2aa90941aa0e5c582578810513ee57a5dbd9ca3f5a8c4b334a0f2 docs/architecture/homes.md
aa87e620702a3e52f700249578f6efd737e3901766c90930f745a7228898fa70 docs/architecture/planning.md
28e3bbcc5cab5577fa676afd9e14304fb07cf916acd7672d59095598481669b3 docs/security.md
dd6d943c4a4b971f1eceea24c400f591dba35f97b52c13128b01842e242e8e41 scripts/render_architecture_html.py
d27f193f8fe132426895fd4d78510280636cadea00d56f35e939e53c5c9c7f7d website/main.py
5e6aed86db41ed3c8e1e74f277af53abcf652bb06e1a651324b355e2563d9d0e website/internal_pages.py
f9ab0125f5b9dda4b592eafb67b2e1f5126121fca3ef0872f924af430ae17da1 website/content.yaml
6b900accc6e07d2dfac248ace7faccc980af2ea9fefc395f3e3c2a27296a6d52 website/static/style.css
```

Generated `docs/architecture.html` SHA-256:
`ce0bb345a825da96203697c2ed8d498095b45b1aa243069addf3fdc6ed8cc7e7`.

If main changes architecture source or rendering inputs, sync docs and regenerate
with the existing script again, then run `--check` and the focused portable test.
This generation does not certify later source edits.
