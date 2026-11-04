# Config distribution — evidence and accepted boundary

Research for Jack Heart, November 3. Full comparison and primary sources:
`d6ddc0aa59:scratch/config-distribution-research.md`; the HTML is the historical
review page. settings-catalog.md is the current implementation plan.

VS Code Remote uses client preferences with explicit remote overrides. Its separate
Settings Sync merges across devices and pauses on conflicts. Mutagen distinguishes
safe one-way updates, forced replicas and peer sync. Syncthing receive-only permits
local edits and conflict copies. chezmoi uses explicit Git/update/apply/merge.
Rex's home directs host-list mutations and queues offline changes; this is not
arbitrary-config merge evidence. Git rejects divergent pushes by default.

Jack accepted origin personal preferences for forwarded work and the remote's saved
preferences for directly initiated work; the executor supplies its environment.
Different entry points may differ. The implementation plan chooses retention with
refresh only while the retained portable content is unchanged, preserving independent
remote edits, including deletions. Forwarding can succeed while persistence needs
resolution. No automatic reverse sync, peer merge or Home role is required.

Full merge would need semantics for ordered accounts, deletions exposing lower
layers, agent/default_agent changes and compound records. Transport preserves
source/scope/target/strength; a resolved snapshot is not a portable authored profile.
Prior sources do not prove LF's exact semantics. Source/doc research only; no external
software or live config was modified.

[VS Code Remote](https://code.visualstudio.com/docs/remote/ssh#_ssh-hostspecific-settings),
[Settings Sync](https://code.visualstudio.com/docs/configure/settings-sync),
[Mutagen](https://mutagen.io/documentation/synchronization),
[Syncthing](https://docs.syncthing.net/users/foldertypes.html),
[chezmoi](https://www.chezmoi.io/user-guide/daily-operations/),
[Rex](https://www.superlogical.com/rex/docs/reference/cli).
