# Machine naming, identity and sharing

Research for Jack Heart · November 3, 2026. Design input only; no live machine,
account, config, registration or execution changes. Jack requested Herdr,
Conductor, Omnara and Rex as references before settling file-backed machine config.

## Findings

**Herdr core: file-backed connections with separate identity.** A machine record
has a local name, transport target, optional remote session, permanent remote-core
ID, reported hostname and observed version. The writer atomically saves these to
`machines.json` under the core's state directory. This is not evidence of a fully
declarative config system: that record combines authored and observed fields.
SSH aliases and custom transports are supported. The local core aggregates remote
terminals; programs and files stay at their destination. A label names the local
connection, not a required remote hostname. No automatic cross-device distribution
of this catalogue was established by these reads.

Sources at `cd6ba77c710b07e97ea53aaaadcbc56f9814240c`:
[machine docs](https://github.com/herdrdev/herdr-core/blob/cd6ba77c710b07e97ea53aaaadcbc56f9814240c/docs/machines.md),
[record and writer](https://github.com/herdrdev/herdr-core/blob/cd6ba77c710b07e97ea53aaaadcbc56f9814240c/crates/herdr-core/src/machines.rs).

Herdr's older TUI is a separate runtime. Its preview docs describe opaque connection
profile IDs, labels, SSH targets, remote sessions and enabled flags, with observed
OS/executable details in a separate optional cache. Local presentation preferences
apply to the client, but commands/plugins run on the selected server; it does not
copy local config or executables onto SSH hosts. Do not conflate its profile IDs
with the newer core's permanent remote-core identity.
[TUI preview source](https://github.com/herdrdev/herdr/blob/ee5bfb5041bb3e4a49d591c72308aae3c81ecdde/docs/preview/website/src/content/docs/connecting-machines.mdx).

**Rex: a shared home directory of hosts, with stored overrides.** Devices choose a
home server and read its host list; local host entries take priority on label
collisions. Entries carry labels and endpoints. The local `home.json` records the
chosen endpoint and reported host identity. API host-record IDs and the runtime's
reported `host_id` serve different purposes; `host_id` describes shared filesystem
identity, not arbitrary physical hardware identity. Mutations of declared hosts
do not edit `init.lua`: they store replacements or hide declarations. The API
describes a separate host storage file; this research does not establish SQLite
as its implementation. That is a concrete example of competing declaration and
mutation layers, even without a DB. CLI inspection identifies declared, stored
and queued changes. Shared-home distribution is useful prior art; its mutation
contract is different from the proposed Git-style LF writer.
[CLI](https://www.superlogical.com/rex/docs/reference/cli),
[host API and server identity](https://www.superlogical.com/rex/docs/reference/api).

Clarification after Jack's follow-up: “shared home” means a host directory shared
across devices that choose the same ordinary Rex server, not a required vendor-hosted
service or an organization registry. Home defaults to this machine; any Rex server
can be selected and receives no special home role. The documented mechanism fits
one person's multiple devices. Rex separately offers shared-account and multi-account
login server modes for multiple people; those modes do not define the home mechanism.
[Server modes](https://www.superlogical.com/rex/docs/linux/server-modes).

**Omnara: organization registry and project grants.** Machine records are created
in a control plane. A daemon attaches using a machine token over an outbound
connection. Configs refer to stable machine names; runtime bindings use `mch_...`
IDs. Machines are organization resources explicitly granted to projects. The
documented machine name cannot be changed. Registration, availability and a
launch's binding are distinct. This offers shared naming via a service, not via
distributing personal configuration. It is not proof of a file-backed registry.
The current platform differs from older remote-coding Omnara docs still indexed
by search; this comparison uses the current repository/docs.
[Overview](https://github.com/omnara-ai/omnara/blob/d5d70e19e335318dfbac61ce7073c528afd3e839/docs/machines/overview.mdx),
[Connect/manage](https://github.com/omnara-ai/omnara/blob/d5d70e19e335318dfbac61ce7073c528afd3e839/docs/machines/connect.mdx).

**Conductor: organization environment, allocated sandbox.** The Cloud Computer
defines an organization's shared environment; new workspaces start from its active
build and existing ones retain their environment. Current docs say bring-your-own
cloud machines are not yet supported. Local repository setup files do not directly
configure cloud setup scripts. Thus this is useful for shared environment intent
versus individual allocations, but not evidence for a personal named-machine fleet
or automatic local preference propagation. Storage internals were not inspected.
[Cloud Computer](https://www.conductor.build/docs/cloud/cloud-computer),
[Cloud FAQ](https://www.conductor.build/docs/cloud/faq).

## Paseo follow-up

Paseo also separates durable daemon identity from connection registrations.
`$PASEO_HOME/server-id` identifies that daemon installation. Connected-host records
carry a registration ID, optional discovered/pinned server ID, label, connection,
metadata and status. CLI mutations call the selected daemon, whose serialized
writer atomically updates `$PASEO_HOME/hosts.json`. This is a concrete file-backed
host-directory API. The file also contains connection credentials and observed
state; it is not a directly shareable declarative profile to copy wholesale into LF.

Changing a connection retains the pinned destination identity unless explicitly
replaced. A connection reaching the owning daemon's own server ID is rejected as
`self_host`. LF's accepted behavior differs: placement selecting this machine
executes locally rather than rejecting a useful shared preference. Client transport
may use SSH, direct TCP or relay; pairing and machine-directory distribution are
different operations. These reads establish a daemon-owned directory/API, not an
automatic all-device settings synchronization contract.

Sources at `93b9ac9dedc122ab045b1724c782a778dd8119d9`:
[identity](https://github.com/getpaseo/paseo/blob/93b9ac9dedc122ab045b1724c782a778dd8119d9/packages/server/src/server/server-id.ts),
[directory writer and self check](https://github.com/getpaseo/paseo/blob/93b9ac9dedc122ab045b1724c782a778dd8119d9/packages/server/src/server/connected-hosts/index.ts),
[protocol](https://github.com/getpaseo/paseo/blob/93b9ac9dedc122ab045b1724c782a778dd8119d9/packages/protocol/src/hosts.ts),
[CLI docs](https://paseo.sh/docs/cli), [connectivity](https://paseo.sh/docs/connectivity).

## LF today

`durable::Machine` already separates `id`, `label`, `route` and timestamps, but
stores them together in SQLite. `machine add` probes the target's identity before
saving. A local row is identified by `route='local'`; remote resolution checks the
reported ID against the saved ID. Labels and IDs can select saved machines.
These are source observations, not a live fleet audit or installed-binary test.

Sources: `rust/loopflow/src/durable.rs`, `store/sqlite/durable.rs`,
`lf/commands/machine.rs`, `lf/commands/ssh.rs`.

## Accepted model — Jack Heart, November 3

Jack accepted the name/identity/connection/local-identity separation and file-backed
machine declarations after this review. Distribution and the lifecycle details below
remain open. This records design acceptance; no implementation or live migration has
been performed.

Separate four things:

1. **Name:** a person's/profile's convenient alias, such as `workstation`.
2. **Identity:** durable identity of the LF installation/environment holding work.
   Persist historical placement by ID. Renaming a label or changing an address
   does not change this identity. It is not an OS hostname or provider VM name.
3. **Connection:** how this origin reaches that identity, possibly through an SSH
   alias or another transport. Routes can differ by originating host.
4. **Local identity:** which LF installation this process belongs to. This is
   nonportable local state, never copied as a personal preference.

Put alias/identity references and connection declarations in the selected config
file. A registration command can discover and write the identity reference so
people need not type it. Keep local identity, credentials, observations and work
history in their runtime owners. Removing DB configuration does not remove durable
identity. Do not share a `route: local` declaration for a particular machine: compare
the referenced ID with this installation's own ID to determine local execution.

Resolve aliases at the launch origin and carry the selected ID. Do not reinterpret
the same alias against a destination's potentially different profile. A preference
for this current machine executes locally; assigned work does not route again.
An explicit relative `local` selector means the origin of a new placement decision,
then becomes a concrete ID, not a string reinterpreted at every hop.

Sharing config may share names and desired destinations; it must not clone a host's
own identity or credentials. File editing, file distribution and provisioning remain
distinct. A shared profile can retain the same alias/ID mapping while local SSH
configuration supplies different connection paths. Copying a whole DB is not an
identity-safe settings synchronization mechanism.

Remaining decisions: profile namespace for personal/team names; how to distribute
the files; whether routing declarations need local overrides beyond SSH config;
identity lifecycle when reinstalling/restoring/cloning an LF environment; rename
behavior for authored alias references. No reviewed source establishes LF's exact
choice/default or origin-preference propagation semantics; those remain LF design.

Validation: primary docs and source inspection only; no external software installed
or executed, and no research-driven implementation launched.
