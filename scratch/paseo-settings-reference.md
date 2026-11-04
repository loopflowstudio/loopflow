# Paseo: configuration ownership and editing

Research for Jack Heart · LOO-714 catalogue design · 2026-11-03

Primary source snapshot: getpaseo/paseo
`93b9ac9dedc122ab045b1724c782a778dd8119d9`, plus current official docs.
Source and test inspection only; Paseo was not installed or executed. This answers
the storage/authoring question, not a general product comparison.

## Finding

For daemon preferences, Paseo's CLI and settings API edit the same configuration
file. The inspected path does not introduce a parallel database override tier.
Client-local remembered choices and project configuration are separate owners,
so “everything lives in one config” would also be inaccurate.

## The relevant paths

| Content | Durable owner | Editing path |
| --- | --- | --- |
| Daemon settings, providers and named agent profiles | `$PASEO_HOME/config.json` | CLI get/set/unset and daemon settings patches |
| Repository workspace setup/scripts | Project `paseo.json` | File edits and project-config API |
| Remembered new-agent provider/model/mode choices | Client AsyncStorage | Creation-form preference service |
| Client appearance and layout preferences | Client settings storage | App settings |
| Agent/project/workspace/timeline replicas | Client IndexedDB or native SQLite | Replication cache, not daemon config authority |

### CLI edits the file; API patches also edit the file

`paseo daemon config set` calls `editPersistedConfig`, which validates the field
against the persisted schema, edits the configured object, then saves it. The
daemon's mutable settings owner separately merges only requested fields into
that same file. An in-memory resolved snapshot is not persisted wholesale.
[CLI source](https://github.com/getpaseo/paseo/blob/93b9ac9dedc122ab045b1724c782a778dd8119d9/packages/cli/src/commands/daemon/config.ts),
[persistence](https://github.com/getpaseo/paseo/blob/93b9ac9dedc122ab045b1724c782a778dd8119d9/packages/server/src/server/persisted-config.ts),
[settings owner](https://github.com/getpaseo/paseo/blob/93b9ac9dedc122ab045b1724c782a778dd8119d9/packages/server/src/server/daemon-config-store.ts).

Tests explicitly cover preventing unrelated edits from persisting resolved defaults
or one-launch overrides, serializing overlapping patches, and rollback on a failed
transition. These are source assertions reviewed here, not tests executed here.
[Tests](https://github.com/getpaseo/paseo/blob/93b9ac9dedc122ab045b1724c782a778dd8119d9/packages/server/src/server/daemon-config-store.test.ts).

### Saving and taking effect are separate results

Configured-value reads report whether the key is set. After saving, the CLI attempts
one reload and distinguishes application, restart requirements, deployment overrides
and a saved file whose daemon could not apply it. Managed launches remove inherited
daemon-setting env vars; foreground deployments may deliberately use them.
[Configuration docs](https://paseo.sh/docs/configuration).

This is a strong pattern for LF regardless of storage: explain the authored value,
effective value, and when the edit takes effect. A successful write can still be
shadowed by a stronger declaration. A default is not an override merely because
it was the value visible when the UI opened.

### Shared schema, with remaining hand-maintained policy

`PersistedConfigSchema` supplies field validation, CLI path lookup and a generated
JSON Schema for editors. But mutable patches, reloadable paths and persisted-to-
runtime path mappings still have separate lists in the settings owner. Paseo
therefore demonstrates useful shared typing, not a complete single catalogue of
all lifecycle and applicability policy.
[Schema generation](https://github.com/getpaseo/paseo/blob/93b9ac9dedc122ab045b1724c782a778dd8119d9/packages/server/scripts/generate-config-schema.ts),
[owner](https://github.com/getpaseo/paseo/blob/93b9ac9dedc122ab045b1724c782a778dd8119d9/packages/server/src/server/daemon-config-store.ts).

### Client-local preferences remain another input

The new-agent form persists provider, per-provider model/mode and per-model thinking
choices in AsyncStorage. These are remembered client selections, separate from
daemon profiles. They can affect a new launch, so the file is not the complete
explanation of everything a person sees in the composer.
[Form preferences](https://github.com/getpaseo/paseo/blob/93b9ac9dedc122ab045b1724c782a778dd8119d9/packages/app/src/create-agent-preferences/preferences.ts),
[storage](https://github.com/getpaseo/paseo/blob/93b9ac9dedc122ab045b1724c782a778dd8119d9/packages/app/src/create-agent-preferences/storage.ts).

The documented client replica persists agent/workspace/project/timeline data using
IndexedDB or native SQLite; it is not another persisted daemon-config override.
[Data model](https://github.com/getpaseo/paseo/blob/93b9ac9dedc122ab045b1724c782a778dd8119d9/docs/data-model.md).

### Repository and account choices

The project-config API writes `paseo.json` on disk and checks an expected revision.
That repository file describes workspace setup; it is not a demonstrated copy of
LF's same-setting global/repo/Wave/skill precedence matrix.
[Project config API](https://github.com/getpaseo/paseo/blob/93b9ac9dedc122ab045b1724c782a778dd8119d9/packages/server/src/server/session/project-config/project-config-session.ts).

Multiple provider/account setups can be represented as provider aliases in daemon
configuration. This is not evidence that LF's managed identities, usage observations
or account failover should become file config.
[Provider profiles](https://github.com/getpaseo/paseo/blob/93b9ac9dedc122ab045b1724c782a778dd8119d9/docs/custom-providers.md#multiple-profiles-for-the-same-provider).

Profiles are saved on a host and affect future selections, not already-launched
agents. Clients accessing that host share the host's configuration; this is not
proof of profile synchronization between independent hosts.
[Agent profiles](https://paseo.sh/docs/agent-profiles).

## Consequences for the LF design

Two coherent storage choices remain:

1. **General DB overrides.** `lf set` always writes the DB layer; personal files
   remain a lower-authority declaration source. Close appropriate CLI/UI/file/DB
   gaps, and always explain the destination and any shadowing.
2. **Ordinary preferences in files.** CLI/UI become editors of the same personal
   config. Repository files keep shared intent; DB retains registrations, routing
   and execution state. `lf set` could be restricted to the DB-backed domain choices,
   with an explicit config-edit operation for ordinary preferences. Existing
   `scope_settings` values would need migration, not silent abandonment.

Paseo makes option 2 more credible: file ownership does not require hand editing
or giving up a settings UI. It does not settle LF's richer precedence policy, nor
show that accounts/machines are the only legitimate DB settings. Storage remains
an LF design decision, not a consequence of building a catalogue.

For either option, adopt the useful contracts: one typed definition used by input
surfaces; preserve authored intent when writing; report saved versus effective;
make client-local remembered selections visible in launch provenance. LF's catalogue
should additionally own applicability and lifecycle policy, avoiding Paseo's
remaining parallel lists.

Jack accepted closing coverage gaps. That goal means every appropriate read/write
surface reaches the common semantics. It does not by itself mean every preference
must have a DB copy. No LF storage migration or writer-policy change is accepted.
