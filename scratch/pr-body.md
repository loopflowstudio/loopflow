Opening an ongoing conversation previously required choosing between separate preparation, connection and recency-based resume commands. `lf connect` now opens or creates the conversation for the current repository, Wave or Task, and can start a fresh conversation while keeping the previous client and history.

<!-- loopflow:task-pr-context:start -->
> [!NOTE]
> **Task:** [Make opening and continuing work conversations obvious · LOO-499](https://linear.app/loopflow/issue/LOO-499/make-opening-and-continuing-work-conversations-obvious)
> **PR lifecycle:** The pull request is published for review; no Task settlement is requested.
<!-- loopflow:task-pr-context:end -->

## What changes

- Make `resume` an exact alias of `connect`. Resolve the current scope from checkout ownership, with explicit repository, Wave, Task or Session targets overriding it.
- Replace `session ensure` and `session new` with `connect --prepare` and `connect --fresh`. Remove `--choose`; opening an exact Session leaves the Task's primary unchanged.
- Use the matching scope's session skill on first creation. Preparation launches no provider; explanation admits no Session. After an uncertain fresh response, ordinary connect recovers the selected conversation; another `--fresh` creates another.
- Move Desktop and remote callers onto the same contract. Desktop reset preserves the previous terminal and concurrent Session updates.
- Accept case-insensitive Task ticket identifiers through shared lookup and remote routing, preserving their recorded spelling.

## Known limitations

Unassociated linked checkouts require an explicit target. Their recovery command now uses the shell-quoted canonical repository path, with an executing same-name collision regression. Before registry creation, explanation reports `Local registry is absent` rather than a future destination.

## Checks

The broad Rust run passed 2,668 tests and exposed three failures, all repaired and verified separately. Final routing/CLI checks passed 147 tests; all 10 transport tests passed. The macOS clone checks passed after migrating copied-primary inspection to the documented read-only inventory. Website checks passed 75 tests; Desktop built and passed 601 headless tests plus the platform-boundary check. Formatting and Clippy pass. The populated 0.14.3 upgrade proof remains applicable: schema and upgrade implementation are unchanged.

Gate fixed remote alias flag routing and extended text/JSON preparation coverage to qualified and shorthand connect/resume. It also updated obsolete resume assertions and pinned the retained-request fixture to its stand-in provider. Preparation rejects --message before admission.

The initial changed-aware runner receipt is failed, not a clean full-suite pass: repaired tests were rerun at their affected scope. Its architecture errors come from two ignored historical prompt files; the final source-only architecture check passes. One passing SSH unit test reported a subprocess-leak warning in the targeted run. Hosted CI still owns the full candidate matrix.

Installed provider continuity remains unverified. Stand-in fixtures cover retained request/turn identity and preservation of the old live client, while Desktop checks cover app compilation and headless model/view interactions. They do not establish real-provider continuity or native rendering.

## Try it

Suggested walkthrough, not yet observed with a real provider: from the canonical repository checkout, run `lf connect`, enter a memorable phrase, exit and run `lf resume`. Confirm the same conversation and history return. Keep it open and run `lf connect --fresh` in another terminal; confirm a new conversation opens while the original remains usable. Subsequent ordinary connect should select the new conversation, while `lf connect SESSION` should still open the original.