# Open choices (4bd948)

Made headless; each is reversible in the diff. `scratch/concept-review.html`
shows them with numbered questions.

- **Unattended runs never ask, whatever `yolo` says.** `yolo: false` only
  changes conversations someone attends. This replaces the kickoff choice
  (keep the per-harness headless floors, delete the Task-checkout rule), under
  which a headless Codex Task step with `yolo: false` ran in `workspace-write`
  and could not reach the network. Also passed over: `yolo: false` meaning lf
  sends nothing even when unattended.
- **Lost on purpose:** `yolo: false` no longer gives a headless Codex outside
  a Task checkout the `workspace-write` sandbox. A `codex.sandbox` setting
  would be its own Task.
- **`--yolo` kept, no `--no-yolo` added.** Off is `--setting yolo=false`.
- **Not probed:** Claude's refusal of `--dangerously-skip-permissions` as
  root or under managed `disableBypassPermissionsMode`, which attended
  launches now meet. Also not probed: that a headless OpenCode run with
  allow-all `permissions` still needs `--auto`; it stays.

## From qa (2026-11-20)

- **Claude's "Adding a new directory" dialog under bypass.** Attended Claude
  launches pass the main checkout as `--add-dir` whatever `yolo` says. In a
  directory Claude has never trusted, every launch (bypass on too) opens on
  that dialog, and its answer lasts one Claude session. The plan says the
  roots matter exactly when `yolo: false`; dropping them when the launch skips
  permissions would remove the dialog, if bypass still reaches the main
  checkout without them (not probed). Left unchanged: it predates this branch.
