# Paseo configuration — supporting evidence

Research for Jack Heart, November 3. Complete primary sources and analysis:
`d6ddc0aa59:scratch/paseo-settings-reference.md`. Current LF decisions are in
settings-catalog.md; the earlier storage alternatives are superseded.

At getpaseo/paseo `93b9ac9dedc122ab045b1724c782a778dd8119d9`, daemon CLI set/unset
and settings API patches edit the same config.json, preserving authored fields
rather than serializing resolved defaults. Project API edits paseo.json with an
expected revision. Source tests cover overlapping patches and rollback; none were
executed here. Saving, effective value and reload/restart outcome are distinct.
Zod schema shares typing, but mutable/reloadable lists remain separately maintained.
Client remembered provider/model choices in AsyncStorage are another launch input;
SQLite/IndexedDB client replicas do not establish a daemon preference override.
Host profiles are not evidence of automatic multi-host synchronization.

Primary paths: packages/cli/src/commands/daemon/config.ts,
packages/server/src/server/{persisted-config.ts,daemon-config-store.ts}, and
project-config/project-config-session.ts. [Configuration](https://paseo.sh/docs/configuration).
