# Subwave read review · 2026-09-28

**Gate: blocked.** Build step 1 is implemented; integrated tests cannot compile.
The accepted subwave design also requires parent discovery and the release split,
which remain unimplemented. This review does not establish the larger design's
Done When or authorize publication.

Reviewed code: `57490cd23`, against the recorded stack base `ce97003cf`.
The working tree was clean on entry; Infrastructure memory was already committed.
This gate changes only review/handoff notes and corrects the design's missing
transcript reference. Inherited LOO-298 implementation and prior scratch cleanup
are outside this gate's repair scope.

## What was implemented

A prompt scoped to `infrastructure/release` reads top-level Markdown from
`wave/infrastructure/`, then `wave/infrastructure/release/`, in its executing
checkout. Each directory orders README first, then other files alphabetically.
Memory travels through the same document pipeline as goals and notes. Ordinary
gathering excludes children, siblings and unrelated Waves; scratch stays recursive.

Native skill seeds and assembled prompts share the Wave renderer. The old
registry walk, origin-checkout memory reader, memory handle and preassembled
memory input are removed. Memory keeps its semantic context attribution.
README and Wave documentation describe the new read rule.

## Key choices and how it fits together

`gather_wave_docs` walks the selected relative path's ancestors without opening
the registry. `format_wave_sections` consumes those documents for both launch
formats; goal seeds contain the goal and available flows, leaving file inclusion
to prompt gathering. This keeps one collector and one renderer without adding a
store or lifecycle object.

Jack Heart's accepted design owns the directory rule. The short curation hint
is an implementation assumption; realign's descendant depth remains open.
The existing selector still supplies the Wave path. Single-segment stored names
and parent identity belong to build step 2 and must migrate selectors together.

## Review findings

- The removed origin reader could hide a checkout's own edits. The new collector
  reads the executing checkout and has a worktree regression fixture.
- Sharing Wave rendering avoids separate native-skill and assembled-prompt
  inclusion rules. The attribution path still identifies MEMORY.md as Memory
  with Wave scope; deleting its old transport did not erase that meaning.
- The earlier documentation-only gate note and a link to a deleted review
  transcript were stale. Both are corrected; the transcript remains in Git.
- No additional production-code correction was identified in this gate.
  Passing a reduced-type harness cannot establish integrated launch correctness.

## Checks

No prior whole-tree pass was reused. The changed-aware plan was inspected with
`uv run python scripts/test.py --base ce97003cf --list`. This gate selected the
affected context/renderer suites and documented static checks, rather than the
plan's entire Rust matrix and browser suite. No UI code changed.

| Check | Result |
| --- | --- |
| Resource envelope | Passed: 67.5 GiB free against 64 GiB floor before compilation |
| Changed-file rustfmt | Passed for all 11 surviving changed Rust files |
| Full `cargo fmt --all -- --check` | Failed in inherited `bin/lf.rs`, `lf/commands/install.rs`, `ops/pr.rs` |
| All-target Clippy, four jobs, warnings denied | Failed: 13 library and 20 library-test diagnostics; all eight diagnostic files identical to stack base |
| Focused nextest selection | Build failed: 12 library and 19 library-test errors; no tests ran |
| Architecture map | Failed: missing `lf catalog` and `auth_browser_bindings`, inherited omissions |
| README/index opening | Passed |
| Portable architecture HTML | Failed: stale `docs/architecture.html`; source, renderer and HTML unchanged from stack base |
| Current-source component harness | Passed ancestor order, inclusion/exclusion, single Wave rendering, scratch recursion, no-Wave context and checkout-local edits |
| Golden Wave fragment | Current-source component rendering matches `with_docs.md`; full golden test remains blocked |
| Diff whitespace | Passed |

Focused Rust command:

```sh
cargo nextest run -p loopflow --lib --test context_tests --test golden_prompt \
  -E 'test(engine::prompt::) | test(engine::flow::tests::render_goal) | test(skill_launch_seed) | test(attributed_context) | binary(context_tests) | binary(golden_prompt)' \
  --build-jobs 4 --test-threads 4
```

Logs: `/tmp/subwave-gate-{tests,clippy,fmt,architecture,docs}.log`.
The current-source harness and executable are under
`/var/folders/m6/r3tllnrs1yq7yfbwm680tss40000gn/T/lf-subwave-gate-mqevjw1e/`.
It extracts six current collector/renderer function bodies into the earlier
fixture's reduced types. Reproduce with `PROOF/proof PROOF
CHECKOUT/tests/parity/fixtures/with_docs`. This is component evidence, not a
real Task launch or disposable-Home proof. No latency or token reduction was
measured, and none is claimed.

## Risks, omissions and next proof

Compilation blocks context integration, native launch tests, attribution tests
and full golden verification. Golden regeneration depends on the same library
build and remains pending. The full Rust/website matrix stays with the repaired
integration gate and CI; no pass is inferred from the selected checks.

The design's disposable-Home Task prompt and parent rename proof cannot pass
this implementation: parent discovery and identity reconciliation remain to
build. Cron execution, the release Initiative/plan transfer and the installed
schedule move are untested and unchanged. No branch binary touched installed
Home, and this gate performed no publish, Task mutation or live-service action.

The read slice advances Infrastructure's checkout-isolation and single-owner
goals. After the LOO-298 build is repaired or integrated, run the selected suites
and goldens on those bytes, then complete parent discovery and the disposable-Home
proof before treating the full subwave feature as ready.
