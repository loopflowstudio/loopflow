# Configuration ownership and resolution

```text
Files + invocation inputs
          │
          ▼
Typed declarations ── catalogue validation
          │
          ▼
One resolver ───────── effective values + explanation
          │                         │
          ▼                         ▼
Launch / domain consumer       CLI / Desktop inspection

CLI / Desktop edits ── catalogue validation ── authoritative file writer
```

[Configuration](../config.md) defines the user-visible behavior. This document
specifies its ownership and implementation boundaries. Both are the LOO-714
implementation contract; the checked inventory in the
[architecture reference](../architecture-reference.md#complete-ownership-map)
continues to describe current code until its consumers are migrated.

## Independent dimensions

```rust
ConfigSource { Builtin, Repository, Personal, Environment, CommandLine }
SettingScope { Global, Repository, Wave }
SettingTarget { General, Skill, GraphOccurrence }
```

These are the conceptual variants; payloads carry the appropriate identities.
Source expresses authority, scope expresses where a declaration applies, and
target selects work within that scope. A separate locator identifies the file
and authored key or invocation input. Choice versus fallback comes from the
setting definition (`agent` or `default_agent`). It does not depend on storage.

`Personal / Global` identifies a general declaration from the personal global
file. `Personal / Wave` identifies a private Wave declaration. Both use the
same source authority. Committed repository and Wave files share the Repository
source and differ in scope. Personal skill definitions also use Personal source.
A Graph occurrence keeps its defining Graph and authored local step ID through
composition, repetition and capture.

The old ConfigLayer mixes these dimensions. Remove its Global-versus-Personal
file/DB split and its Repo-versus-Wave source split. Migrate every consumer,
serializer, DTO and test to the same model. No legacy alias, conversion chain or
second rank table preserves those distinctions after cutover. A source name must
mean the same thing in Rust, Swift, inspection, help and documentation.

## One settings catalogue

A setting definition owns:

- Its typed value codec, validation, default and description.
- Applicability over source, scope and target, with reasons for exclusions.
- Combination behavior: replace, append/reset, keyed merge or choice/fallback.
- Persistence binding, lifecycle and portability, including local dependencies.

Declarations carry their address, authored value and locator. Resolution carries
effective values, contributing declarations and the reason for each disposition.
The resolver is the only owner of precedence; inspection must not infer a winner
from array position or reimplement comparison logic.

The catalogue derives schemas, discovery and interface metadata from the same
typed declarations. Dynamic keys use typed selectors. Private Config bookkeeping,
Graph topology and delegated provider fields are not generic preferences.
A tagged provider configuration is one validated value; editing it cannot combine
incompatible fields from different providers.

Remove parallel key lists, ad hoc scope checks, separately mirrored client schemas,
and schema inference from serialized defaults. A domain-specific editor remains
appropriate when it performs a domain operation; ordinary preference values still
use the common validation and file writer. Missing interface coverage needs a real
applicability/lifecycle reason, not an adapter that advertises an omission.

## Preference ownership

| Content | Authority |
| --- | --- |
| Personal global declarations | Personal config.yaml |
| Private repository and Wave declarations | Main checkout's lf.config.local.yaml, shared across linked worktrees |
| Shared repository and Wave declarations | Committed repository/Wave files |
| Skill and Graph occurrence declarations | Their authored definitions |
| Explicit CLI and environment choices | The invocation; never persisted as preferences |
| Account identity, credentials and observed availability | Existing account/secret domain owners |
| Current machine identity and observed process/execution state | Runtime domain owners |
| Reusable layout templates and selected defaults | Configuration |
| Actual saved Task layout | Persisted live workspace state |
| Remote delivery origin/version/last-applied evidence | Operational metadata, never a preference override |

File edits preserve unrelated keys/comments, validate before replacement and
serialize compare-and-write. The same invalidation path covers CLI, Desktop and
external file edits. Resolve private files through Git metadata, not a database
registry of whichever checkouts happened to be observed. Wave selectors preserve
Wave identity through rename/relocation and distinguish reuse of an old name.

No DB preference reader remains after migration. Configuration and bootstrap
inspection work without a compatible store. A missing runtime account or machine
is reported as resource availability, not as a missing preference source.

## Resolution and invocation

Agent choices precede all fallbacks. Within either class, compare source, then
scope, then target. For other settings, apply the catalogue's combination policy.
Keep false, explicit empty, omission and unset distinct. Lists can have multiple
contributors and resets; an explanation need not invent one winning declaration.

A Session's explicit agent selects that conversation. A Graph's explicit agent
selects its steps. Neither a resolved value nor historical execution attribution
becomes new preference authority for unrelated work. User-supplied environment
is explicit input; internally generated process plumbing is not.

## Remote requests and retained files

A remote request selects one personal profile: its initiating LF instance's
portable declarations. Executor-local bindings and runtime facts come from the
executor. Do not merge the destination's competing portable personal profile as
an unnamed precedence tier. Repository inputs retain their ordinary authority.

Transport preserves scope, target, authored key and source locator. A resolved
snapshot cannot preserve these semantics and must not be serialized into CLI
flags or saved as a personal file. Explicit invocation choices remain separate.
Placement resolves once to a durable destination identity; the current identity
executes locally. Preserve native provider/account continuity constraints.

Execution and retention have independent outcomes. A request may execute with
its originating profile while retaining neither it nor its changes because the
remote file was independently edited. Subsequent direct work uses the saved
remote file. Inspection exposes both outcomes and the profile actually used.

Retention refreshes only an unchanged last-applied portable projection or an
already equal one. Local deletions count as edits. Initial differing preferences
need explicit adoption. File writers check versions atomically; old deliveries
and competing origins cannot silently overwrite newer or independent changes.
Do not auto-merge ordered account lists or related configuration fields.

Host-local bindings are outside the portable projection. Partial payloads carry
coverage so an omitted field is not confused with a deletion. Saved delivery
metadata is evidence, not another writable configuration source. Direct remote
work while offline uses its saved declarations. No automatic reverse propagation
or global Home authority follows from retention.

## Migration and code parity

A migration preserves authored declarations, account order and identity bindings.
Exporting effective values destroys fallback and provenance. Before retiring DB
values, inventory collisions with files and the behavior of a former DB-global
override against a more specific file value. Coordinate database and file cutover
with recoverable receipts; neither partial failure nor an unavailable checkout
may discard the sole copy of a preference.

Repository-scoped private preferences belong to a clone and its linked worktrees,
not every clone with the same remote. Jack Heart accepted this scope change on
November 3. A canonical remote identifies the project but does not select a private
file from an arbitrary checkout registry. Runtime reads use the addressed checkout.

For migration, preserve the old origin-wide account policy as a recoverable export,
then materialize its declarations into explicitly identified existing clones after
checking file collisions. Copies become independent clone policies at cutover. A
route with no available checkout remains recoverable, not an active fallback.
Future clones do not automatically inherit it; explicit import can adopt it later.
Registration is not a prerequisite for ordinary config reads or writes. Migration
must report missing or ambiguous destinations without choosing one silently.

Delete replaced DB preference owners, key lists, parsers, rank logic and stale
compatibility paths in the same cut. Preserve required behavior through the
surviving owner. Keep the checked architecture inventory synchronized with actual
code; it must not claim a table is deleted while runtime consumers still use it.

Contract tests exercise file/CLI/UI parity, declaration provenance and actual launch
preparation. Include worktree sharing, hand edits, default/choice conflicts,
false/empty/unset, Wave identity changes, migration interruptions, origin-versus-remote
profiles and retention conflicts. Avoid tests that merely mirror the catalogue or
assert mock wiring. Documentation examples and code must agree before publication.
