# v0.12.1

v0.12.0 asked who runs the work and as whom it spends. v0.12.1 answers a narrower and older question: what, exactly, is a running piece of work, and who is allowed to steer it. Three overlapping models — interaction reviews, interactive handoffs, and child commands — collapse into one durable control spine of Work, Runs, Launches, Steers, and Turns. The second half of the release is doctrine rather than code: verification is now scoped to the smallest proof that can change the next decision, so agents stop burning hours rerunning suites nothing depends on. Net, this release deletes about 28,000 lines and adds 16,000.

## One spine for running and steering work

Loopflow had three ways to say "a human or parent is interacting with this body," each with its own store, ids, and Swift mirror, and they disagreed at the edges — a review id that outlived its flow step, a handoff that claimed attach on a surface that could not attach, a child command that could orphan. All three are deleted. What replaces them is a single chain: stable Work and Epoch identity, exact Run authority, provider/process Launches, Basis-fenced Steers, optional observed Turns, and Review as a *derived* attention state rather than a stored record.

The practical shape: direction is durable and delivery is best-effort. `lf task steer` appends direction that survives whether or not the provider is listening; interrupting the active boundary is a separate act from replacing what the work should do. Codex can live-send while Claude, OpenCode, and opaque TUIs seed a fresh boundary — the durable outcome and the fencing are identical either way.

- Steer, Send, and Basis are authoritative through one opaque Run capability that fails closed when it is missing, stale, or stopped — only the active parent Run can steer its child (#1073).
- `lf work status <kind> <id>` projects stable Work; `lf work close` advances the current interactive step under its Basis fence. There is no Review id and no disposition to pass.
- `lf launch list|status|attach|present|handback` operate on the real process the Run controller already owns. `attach` prints the descriptor without mutating state, `present` execs into it, and closing the terminal does not end the Launch. For an opaque TUI, `handback --outcome succeeded|failed|interrupted|unknown` records the observed result — process exit alone never claims success.
- Every body — new, recovered, or migrated legacy — registers its real process as a Launch under its mirrored Run, so interrupt and steering never fall back to Session lookup. Root assistant output and normalized usage live on Turn, and monitoring and spend derive from Run → Launch → Turn.
- Attention is serviced in a fixed order: direct input first, then the oldest child Review, then Wave/Project background work — and the background playhead survives the preemption for both live-send and seed-only providers.

Task and Project still execute through the existing Session controller in this landing; the spine is mirrored under them, not yet the only runner. A follow-up Task owns the one-way rewrite through shared Run `reserve | advance | stop` and the deletion of the Session lifecycle, body generations, child write leases, and the legacy authority env vars.

## Verification you can afford

Agent verification time was being spent on proofs nobody was waiting for: full suites rerun at every lifecycle phase, budget history recomputed from a durable ledger. The new doctrine gives each phase exactly one proof level — implement runs a focused behavioral proof, compress reruns only if behavior changed, lint is formatting and static analysis, rebase proves nothing when conflict-free, gate runs affected suites once, and CI/release owns the full matrix. Escalate only when the narrow proof fails or the change crosses a boundary it cannot exercise (#1091).

- `scripts/test.py --reuse-passing` reuses a prior pass only when tracked content, untracked content, the worktree, and the command plan are all identical — an exact-tree fingerprint, not a time window.
- `scripts/test_time.py --days N` reports where verification time actually went, merging parallel intervals per launch and printing only aggregate categories and skills — never commands, prompts, or output.
- The durable gate budget machinery (`GATE_BUDGET.md`, `--history`, HOLDING/NOT HOLDING verdicts) is removed; exact-tree and plan fingerprints replace it.
- A new `testing-audit` builtin skill finds low-value tests and redundant verification. TESTING.md is rewritten around focused proofs and the nine-job `tests-result` aggregate; the Verification Cadence doctrine lands in STYLE.md and the build skills.
- Nightly package smoke now exercises `lf --help` and `lf --list`.

## Operational notes

**Removed commands.** These have no compatibility shim — the underlying records no longer exist:

- `lf handoff open|present|list|complete|back` → `lf launch present|list|status|handback`
- `lf task review message|complete|reply` and `lf reviews catch-up` → `lf task steer` plus `lf work close`
- `lf task decide`, `lf task acknowledge`, `lf task receipt` → Steers are durable on append; there is nothing to acknowledge
- `lf task interrupt --message` → interrupt, then steer; the two acts are deliberately separate
- `lf task reconcile` → it repaired orphaned ChildCommands and gated completion on directive acknowledgement. No ChildCommand can orphan now, and completion reads Review/Basis, so the command was superseded rather than ported.

`lf task attach` still attaches to the live tmux process, but raw terminal input is provider transport, not the control ledger — prefer `steer` for anything that must survive a body.

**Wire and store.** The `InteractiveHandoff` DTO and its fixtures are gone, along with the Swift handoff surfaces and the Active Sessions census fixture. Migrations `0.11.030`–`0.11.035` land the spine: one spend grain, the durable input spine, Run/Launch/attention, typed CI runs, and the ChildCommand drop. As of v0.12.0's promotion boundary, only an official release install advances `~/.lf/loopflow.db`, under the drained-body promotion lock — upgrade through the release, never mid-turn.

## Small changes

- `docs/architecture.md` is substantially rewritten around the control spine; retired controls are removed from user docs, built-in skills, smoke tests, and fixtures rather than left as stale examples.
