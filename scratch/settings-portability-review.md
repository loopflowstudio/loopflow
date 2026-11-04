# Settings surface audit — historical evidence

Jack Heart's November 3 audit is preserved at
`d6ddc0aa59:scratch/settings-portability-review.md`. settings-catalog.md is now the
accepted implementation plan; settings-scope.md retains the earlier field matrix.
The audit's DB preference/distribution proposals and running-Graph claims are
superseded, not implementation requirements.

Inventory anchors: config.rs; config/settings.rs (LAUNCH_KEYS, REPO_ONLY_KEYS,
ADDITIVE_KEYS, validate_scope); lf/commands/setting.rs (separate account/machine
routes); lf/commands/config.rs (last-candidate rendering); work/wave/setup.rs;
Desktop AppearanceStore; machine_records.rs; lf/commands/ssh.rs. Source inspection
found general setters local-only, machine-choice copying separate, and history
export not a settings synchronization mechanism. These are dated source findings,
not proof of installed behavior.

Preserve launch/inspection agreement, typed validation, additive reset provenance,
bootstrap settings, theme Apply/Cancel, account order and native history continuity.
Dormant summaries/summary_tokens had no runtime consumer. A portable setup needs
referenced assets/local bindings; resolved snapshots erase source/default intent.
The implementation must inventory current consumers and close meaningful coverage
gaps across all settings, not just agent selection. No per-skill accounts.
