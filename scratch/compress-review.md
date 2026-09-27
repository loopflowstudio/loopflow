# Installer model review — 2026-09-26

Reviewed the active branch from `102522bc2` through `e75ba2dc2`, the full
newcomer design, and the installer owners and callers. No meaningful structural
reduction found. Production code and public contracts remain unchanged.

## Ownership map

| Fact | Owner | Consumer |
| --- | --- | --- |
| Retained executable bytes and bundle identity | `machine_install::ArtifactSet` | Staging, validation, promotion and recovery |
| Prior and candidate installation/store | `SwitchReceipt.prior` and `.target` | Promotion and recovery selection |
| Installed paths to replace | `ActivationTargets` | Entry registration, process shutdown and app reopening |
| App was running before shutdown | `SwitchReceipt.app_was_running` | Reopen decision across retries; monotonic receipt validation |
| Which installation may reopen | Store-advance boundary plus prior/target selection | `app_to_resume`, which verifies the selected bundle |
| Coordinator executable location | Current process plus retained binary staging | Published bundled helper re-exec before shutdown |

## Reductions considered and retained boundaries

- Retained paths and activation paths are different locations. Keeping both
  lets first installation find a copied app without a prior receipt and lets
  recovery stop processes from either installation. No new path registry exists.
- Restart intent cannot be derived after shutdown. Keep the existing persisted
  boolean and its monotonic update; helper activity alone does not imply a window.
- `app_to_resume` is shared by production reopening and behavioral fixtures.
  It derives a result from the receipt without adding another state model.
- Bundle re-exec reuses content-addressed staging and the existing hop bound.
  Removing it would leave the coordinator inside the shutdown/replacement target.
- Promotion flags remain consumed by the shell installer and local/delegated
  promotion. The retired app target still supports installed upgrades; there is
  no evidence permitting its removal. Receipt schema stays unchanged.
- Swift selects the bundled helper; it does not mirror the switch receipt.
  Published update destination retention is still missing work in the design,
  not an implemented representation to simplify in this pass.

## Review and proof

Corrected stale scratch status that claimed no implementation existed. Existing
design evidence records four passing app lifecycle tests, four artifact tests,
formatting and clippy. Their production and test source is unchanged, so no
behavioral tests were rerun for this documentation-only pass.

The lifecycle fixtures do not prove helper re-exec, real app reopening, signed
bundle activation or the clean-account journey. Those limits remain in the
design; this review does not establish completion of LOO-312.
