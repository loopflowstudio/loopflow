# Machine configuration — supporting evidence

Research for Jack Heart, November 3. Full sources, pinned commits and findings:
`d6ddc0aa59:scratch/machine-config-prior-art.md`. settings-catalog.md now owns the
accepted file-backed machine, identity and remote preference implementation.

Herdr core separates local name, transport target and permanent remote identity in
machines.json; older TUI behavior is a different runtime. No cross-device config
sync was established. Rex lets an ordinary personal server be home for a host list;
API edits can replace/hide Lua declarations without editing init.lua. Its host-list
IDs differ from runtime host identity. Home is independent of its shared-account
and multi-user login modes, which Jack excludes. Home is not an LF prerequisite.
Omnara uses an organization registry/project grants; Conductor uses shared cloud
environments. Neither establishes LF-style portable personal configuration.
Paseo separates daemon server-id from connection registration IDs and writes
hosts.json; mixed secrets/observations make copying that whole file inappropriate.
Its self-host rejection differs from LF's accepted execute-locally behavior.

LF audit: durable::Machine already has id/label/route/timestamps; machine add probes
identity; SSH checks saved identity. File-backed declarations retain alias, ID
reference and connection, while each installation's own identity stays local.
Resolve placement once at origin; current ID executes locally. Config does not
provision tools/credentials or clone runtime identity. Connection reachability can
differ by host; use local bindings. No reviewed system proves LF's exact semantics.

Primary docs: [Rex](https://www.superlogical.com/rex/docs/reference/cli),
[Paseo](https://paseo.sh/docs/cli), [Herdr](https://github.com/herdrdev/herdr-core/blob/cd6ba77c710b07e97ea53aaaadcbc56f9814240c/docs/machines.md).
