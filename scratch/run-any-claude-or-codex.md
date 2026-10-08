# Run installed skills on either harness

LOO-420, PR 2. Jack Heart's October 8 comments
`5f149330-5f5d-4a71-bb05-289b13fe2d94` and
`76407cd0-b271-45e7-8953-62abe7f9df2b` supersede the busy-conversation expansion.
The outcome is ordinary `lf audit` / `lf audit -a codex` in a repository with
an installed skill and no `.lf` configuration. The selector remains `--agent/-a`.
Native same-harness invocation, translated ports, exact arguments, reachable
assets and declarations remain required. LF builtins stay inline. Review remains
the delivery boundary; no landing or Task completion is authorized.

## Existing behavior to retain

One engine catalog selects repository, personal and embedded sources for launch,
list/help, Flow capture and export. Native declarations and arguments travel with
the selected source. Export preserves third-party files. Generated builtin
exports do not shadow the running binary. Removed external/npx/rams resolvers
stay removed.

Claude headless launches separate gathered user context from the native command.
Context acknowledgements cannot finish the skill. Captured Flow source bytes
survive source removal; sibling links do not preserve a removed resource bundle.
Native Claude snapshot paths and parent-relative resources still need fidelity
work. Existing cross-harness declarations are reported but not fully translated.

Independent continuation fixes remain: saved Session placement supplies context,
provider and replay cwd; Process provenance keeps the caller cwd. Input reservation
and driver claim commit together, preserving active captures on rejection and
unpublished reservations on publication failure. Ordinary resume never reruns
the original skill. Existing native history, fences and engine settlement remain.

## Delete — do not maintain

Removed: Codex dispatch identities/receipts, queue and engine-restart probes,
Claude PTY/inbox injection, and their exclusive tests. Exact deletion inventory
and historical observations: `7dd9819b2:scratch/run-any-claude-or-codex.md`
and `1e4ae02a5:scripts/benchmarks/skill-invocation/README.md`.

Also removed: `ClaudeHarness::native_skill`, its plugin startup/context-message
branches, and `native_input_tests`. Prompt preparation creates a native invocation
only for headless Claude, which dispatches through `_run_agent_once`, not
`ClaudeHarness`. The deleted test bypassed that boundary by assigning private
state. Baseline Claude Session input/result accounting stays intact; the ordinary
LF/Flow fake-API proofs exercise the surviving file-backed input path.

## Remaining implementation

1. Direct native invocation for installed matching-harness sources, including
   Codex headless and both terminal launch paths. Keep gathered context separate
   from exact skill arguments; use existing capture/launch ownership.
2. Translate cross-harness arguments, tool/model declarations and available
   controls. Report genuinely unavailable equivalents in one line. Supporting
   assets use the original directory; unfamiliar declarations remain instructions.
3. Prove ordinary lf entry points with unchanged third-party skills on both
   harnesses, selected source in help/list, exact arguments, declarations and
   bundled reads. Preserve Flow-captured selection and inline builtins.

No temporary repository catalog mount, engine-global root replacement, new
Session owner or busy-terminal transport is selected.

## Compression review

The dispatch ledger and recovery additions are absent from the surviving Codex
harness, connection and event store: those files match the active PR base.
ClaudeHarness production code also matches that base; its only diff is the new
optional AgentConfig field in two existing test literals.
The retained resume transaction is an independent preservation fix, not a pending
queue. The public reconnect fixture retains stale-client rejection, native history,
sibling work and ancestry checks. Its tool-name and current CLI-flag repairs stay.
The Claude request fixture now tests LF launch and Flow capture only; historical
provider-only alternatives are archived. Release child memory was read selectively
for its operation-entry lesson; unrelated release sections were not re-reviewed.

Before reduction against `35bb84ef1`: 56 files, +5,818/-1,833; Rust production
+2,297/-1,419. This pass starts at 51 files, +2,852/-1,914 and removes another
88 net lines from ClaudeHarness. After this pass: 51 files, +2,769/-1,911,
including reconciled notes. This reduces speculative execution machinery while
preserving the one-catalog implementation.
The full third-party/Codex/terminal outcome is not yet ready for review.

Check (2026-10-08, isolated LF authority): `cargo build -p loopflow --bin lf`, `cargo test -p loopflow --lib harness::claude` (31 pass, 2 live-login ignored), `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, and network-contained `request_mapping.py --lf target/debug/lf` with/without `--flow` pass; unaffected checks retained at `7dd9819b2`; full fidelity and gate remain.
