# Selected runtime and retained execution store

LOO-334 · 2026-09-30 · Decision pending with Jack Heart in this conversation.

## Observed conflict

The disposable normal-promotion proof preserves the pending review and both
complete Flow copies. Discovery identifies the original execution directory,
then the selected installed development executable refuses its production
database. Review completion and the second worker were not reached. See the
recorded-installation-copies sections of
[the working design](resolve-tasks-from-linear-and.md).

Read-only source inspection at `1fd99a284` confirms that
`store/mod.rs::installed_execution_database` reaches the general development
database guard during path resolution. `store/sqlite.rs::open_with` already
contains an explicitly addressed execution path that validates the schema with
a read-only connection before opening it for writes and disables migrations.
No behavioral proof was rerun in this consultation.

## Choice presented

1. Recommended: permit the verified currently selected installation, including
   development installations, to continue in an explicitly routed compatible
   original database without migration. Preserve arbitrary branch-binary
   isolation. Ordinary execution writes are permitted; this is not a read-only
   execution contract.
2. Keep production isolation and select the retained released runtime for that
   execution, changing the selected-runtime-at-each-boundary requirement.
3. Authorize explicit ownership transfer to the copied database, changing the
   unchanged-execution-directory requirement and requiring a real handoff.

Jack's answer is pending. The recommendation is not authorization. No runtime
policy, source implementation, host installation or host store was changed.
The separate Linear relationship-repair decision remains independent.

## Next action after a decision

Record Jack's selected contract here and reconcile the working design and
questions. The waiting caller implements it through the existing installation
and store owners, then repeats the disposable normal-promotion proof through
exact review completion and two actual worker boundaries with poisoned inherited
runtime settings. Require selected executable evidence, unchanged invocation and
execution directory if option 1 is selected, unchanged copied execution, refusal
of incompatible schema without migration, and continued arbitrary branch-binary
isolation. Retain the divergent-copy assertion. No host experiment is needed.

Session readiness must report whether the decision remains pending; readiness
does not complete the Session, resume the caller or choose Flow navigation.
