# Subwave read slice · 2026-09-28

## Finish line

A nested Wave prompt includes each ancestor's top-level Markdown, then its
own, exactly once, from the executing checkout. Ordinary runs exclude children,
siblings and unrelated Waves. Scratch stays recursive. Both assembled prompts
and native skill seeds use the same collected documents. No registry read is
needed to collect files, and no separate memory input remains.

## Observations

- `lf task status LOO-329 --json` failed: `no Task exists for "LOO-329"`.
  Task reporting is unavailable from this shell; no auth or Home repair attempted.
- The inherited collector read memory from the origin checkout and walked the
  registry separately. Its worktree test explicitly expected origin memory.
- An existing uncommitted edit to Infrastructure memory predates this slice;
  it is left untouched and excluded from this slice's checkpoint.

## Assumptions

- Use the short hint “Curate wave/<path>/MEMORY.md in this checkout. Ancestor
  files provide inherited context.” and point to realign. This is implementation
  wording, not a recorded Jack Heart approval of exact prose.
- Parent realign's explicit child reading remains with its skill; descendant
  depth does not change ordinary context gathering in this slice.
- Preserve README-first ordering within each directory, then alphabetical files.
- No publication: preserve the supplied local review boundary.

## Evidence

- Resource envelope passed: 74.2 GiB free against the 64 GiB floor.
- `cargo test -p loopflow --test context_tests --lib wave_` could not compile:
  12 library errors and 19 library-test errors in inherited files. No tests ran.
- `cargo clippy --all-targets --message-format short -- -D warnings` also
  failed before completion: missing Task/Flow functions and types, removed
  install daemon fields, stale WorkCatalog/RunSpec consumers, PR moves through
  a shared reference, and unused `mut`. All diagnostic files are unchanged
  from HEAD by this slice. Log: `/tmp/subwave-read-clippy.log`.
- `cargo fmt` was run. Its unrelated formatting changes to `bin/lf.rs`,
  `lf/commands/install.rs` and `ops/pr.rs` were restored to HEAD. Full
  `cargo fmt --check` still reports those inherited differences; the changed
  Rust files pass direct rustfmt checking. Log: `/tmp/subwave-read-fmt.log`.
- Architecture check failed on inherited map omissions for `lf catalog` and
  `auth_browser_bindings`; neither surface is changed here.
- An isolated Rust harness compiled the exact source bodies of the Wave and
  scratch collectors and Wave renderers. It passed ancestor order, complete
  top-level Markdown inclusion, sibling/child/unrelated exclusion, single
  rendering, scratch recursion, no-Wave behavior and checkout-local edits.
  This is component evidence with reduced surrounding types, not an integrated
  launch, Task-selection or disposable-Home proof. Reproduction is
  `PROOF/proof PROOF CHECKOUT/tests/parity/fixtures/with_docs`, where PROOF is
  `/var/folders/m6/r3tllnrs1yq7yfbwm680tss40000gn/T/lf-subwave-read-odnm90m2`.
  `proof.rs` retains the exact extracted bodies and assertions.
- The affected `with_docs.md` golden's Wave fragment was rendered by that
  harness; its former Wave-doc fragment was removed from explicit docs.
  Full golden regeneration/verification remains blocked by compilation.
- `git diff --check` passed. No branch binary touched installed Home.

## Review findings

The separate memory input was a second authority and a checkout mismatch;
removed it from launch inputs, Work context, prompt components and goal seeds.
The final reference audit found the old `Memory` file handle had no remaining
callers; deleted that module and its missing-file test.
Native skill launches now render the same collected Wave files. During review,
keep Wave memory's semantic attribution (`Memory`, Wave scope and Wave bucket)
while using ordinary documents; otherwise this reduction would silently change
context accounting. The implementation retains that attribution.

No new type, store or registry owner is introduced. The existing document
pipeline owns gathering, and the two launch formats share rendering. Existing
Infrastructure memory changes remain untouched. No checkpoint includes them.

## Remaining exit

Build step 1 is coded but **not integrated-verified**. Repair or integrate the
LOO-298 build first, then run the context integration suite, prompt/goal/native
seed/attribution unit tests and prompt goldens, plus formatting and all-target
Clippy on final bytes. Parent identity/discovery, Task-bound prompt selection,
rename identity, cron execution and installed release split are not proved here.
No publication, Task completion, installation or live schedule mutation occurred.

Compression removed the single-field goal context and document copies during
rendering. Exact-source component comparison passed 128 context combinations
and three flow lists (reduced types, user-context stub); artifacts:
`/var/folders/m6/r3tllnrs1yq7yfbwm680tss40000gn/T/lf-subwave-compress-hd4is3i1`.
Changed-file rustfmt and diff checks passed. All-target Clippy remains blocked
by inherited compilation errors; full fmt reports the same three inherited
files. Logs: `/tmp/subwave-compress-{clippy,fmt}.log`. Integrated proof remains open.
