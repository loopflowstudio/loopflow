Preferences were split across database overrides and files, making it difficult to predict which settings a launch would use. CLI and Desktop now edit the same file-backed declarations and explain the values used to prepare work.

<!-- loopflow:task-pr-context:start -->
> [!NOTE]
> **Task:** [Recover cleanly from model limits, and pick the right harness and model for each step · LOO-714](https://linear.app/loopflow/issue/LOO-714/recover-cleanly-from-model-limits-and-pick-the-right-harness-and-model)
> **PR lifecycle:** The pull request is published for review; no Task settlement is requested.
<!-- loopflow:task-pr-context:end -->

## What changes

- One typed catalogue covers LF preferences, validation, applicable destinations, merge behavior and lifecycle. Personal repository preferences live in the main checkout’s `lf.config.local.yaml`, shared by its Task worktrees; independent clones keep independent preferences.
- Every applicable `agent` choice beats every `default_agent` fallback. Within either class, resolve command line, environment, personal files, repository/Wave files and builtins, then scope and target. Named Graph-step settings retain their identity through composition and repetition. Session choices stay local to that conversation.
- There is no implicit Codex-first preference. With no authored choice, exactly one installed harness is selected automatically; zero or multiple installations leave the agent unresolved. Inspection still works, new launches explain how to choose, and existing Sessions retain their provider. Setup saves only an explicit personal choice.
- Account order, machine preferences and capacity declarations move to files. Migration preserves authored values and recovery evidence, reports collisions, and exports unavailable clone destinations for deliberate import. Runtime identity, observations and execution history retain their existing owners.
- Remote work carries the initiating personal profile, including skill and Graph metadata. Retention preserves independent executor edits and reports conflicts separately from the profile used for execution.
- Context inputs share `context.docs`, `context.exclude`, `context.diff`, `context.diff_files`, `context.paste` and `context.budgets` across files, definitions, setters and inspection. Leaf merge/portability policies are catalogued; the temporary released-format reader is tracked by removal Task 807ef7.
- Configuration subscriptions observe file edits. CLI inspection groups its colored table into Agents and context, Machines, Accounts, Desktop, Planning, Release and Loopflow; Desktop browses the same catalogue categories. Optional explanations retain provenance; fallbacks appear within the effective agent explanation, and Graph headings use step names. CLI inspection, the Desktop editor and launch preparation share the resolver and destination writer. Updated documentation describes the file ownership and precedence contract.

Automatic model/account-limit recovery remains the separate second PR.

## Checks

- The earlier file-backed-settings gate passed 422 Rust tests, architecture, migration-history validation, test inventory, formatting, Clippy with warnings denied, 72 website tests, and 680 headless Swift tests plus the isolated lifecycle fixture.
- Additional Task/Session launch, Wave setup, subscription, served-read, account-routing, rental and FFI checks passed. Standalone command fixtures were repaired to isolate in-process configuration and assert file-backed behavior; their remaining regression passed after correcting the typed account assertion. Those checks preceded the harness-selection refinement below.
- The harness-selection refinement passed all eight settings/Graph CLI tests, 48 Rust DTO fixtures, 141 configuration/prompt/Session tests, 51 Swift DTO checks, and the targeted resume-after-deleting-default test. The zero/one/multiple-harness test also proves Task creation and inspection without a choice. Current formatting and all-target Clippy passed.
- Remote transport uses an SSH substitute; providers use stand-ins. These checks do not establish configured-provider or installed-app acceptance. The historical Linux preview timeout and writer leak warning did not reproduce in the current macOS checks; their original causes remain unproved.
- The context namespace and grouped viewer passed 110 focused library tests during implementation and 35 affected integration cases, including real CLI setters/inspection, prepared documents, exclusions, reset/unset, Graph exports and context delivery/reconnect with provider stand-ins. Formatting, all-target Clippy, the Desktop build and two configuration Swift tests passed. Old fixture commands were updated to canonical setting paths and preview fixtures now select an explicit harness. The CLI demo is accepted by Jack Heart; mounted Desktop interaction was not demonstrated.
- Hosted full-matrix checks run after working notes are cleared for landing. Desktop linking emitted deployment-target/unwind warnings; the build and tests succeeded.

## Try it

Suggested walkthrough: run `lf set skills.implement.agent codex`, then inspect `lf config show --skill implement --explain`. The saved path should be the main checkout’s private file, including when run from a Task worktree. A personal agent choice beats a shared occurrence choice; changing it to `default_agent` lets an applicable shared `agent` win. Unset the personal key to reveal the lower declaration.
With multiple harnesses installed and no preference, `lf config show --skill implement` displays “Not selected.” Save an explicit fallback with `lf set --global default_agent claude`; a repository or step agent choice still takes precedence over that fallback.

## Rollout

Jack Heart accepted shipping and migration on November 4. After verified merge and a green dev build, use the normal installer, then migrate old preferences. Nine main repositories were audited: the personal file has no context-family keys, Loopflow and Hootro need root exclusions moved, and Loopflow gate/implement frontmatter needs context.docs. The patches preserve other declarations; Hootro has an unrelated legacy branch_names key. Live files retain the released shape until a compatible CLI is installed. Temporary reader removal belongs to Task 807ef7; automatic limit recovery remains outside this PR.
