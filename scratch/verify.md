# Verify LOO-995 (PR #1997, merge 41023c8e55)

Outcome: **Verified.** Checkout tree == origin/main (`git diff HEAD origin/main` empty).

Built `lf` from that tree, ran in `/tmp/lfv.hNWu` with its own HOME/LF_HOME, `LF_*` unset:
- `lf wave create billing`, `lf wave create billing/refunds` → added and committed.
- `lf task create --wave billing/refunds --title "Refund a mischarged invoice"` → `224dfa`.
- `lf wave status billing/refunds` → `Project   refunds (local) · started` with the Task under it.
- `lf wave rename billing/refunds billing/credits` → status shows `Project   credits (local)`.
- Store: `waves ⟕ projects` → one Project per Wave (billing 1, credits 1).
- `lf wave plan bind` / `update-project` / `edit-project` → rejected by Clap; `rename --help` has no `--title`;
  `lf wave plan connect [WAVE] [PROJECT]` takes the optional Linear Project.

Post-merge CI on 41023c8e55: push `CI` success (45582106718), merge_group CI success;
`macOS on main` and `Packages (main)` still queued at check time.

Deferred (needs a Linear-connected store with multi-Project Waves): first `lf wave sync` after upgrade,
Linear Project create/rename/link, Desktop header link. Unit/integration coverage ran in CI.
