# Usable database clones

Jack Heart authorized implementation and autonomous delivery on October 13, 2026.
The persisted brief is Task lf-6907009; LOO-462 remains independent.

Implement `lf clone create`, `run`, `show`, and `dispose` as the single owned path.
Creation uses SQLite backup under an OS write boundary. A private marker records
source identity, schema/build, captured history, omitted artifacts and limitations.
Only creation applies pending canonical SQL and this binary's drafts to the new
destination; ordinary opens remain validation-only. No published provenance is
fabricated. Incompatible subsequent builds require recreating the disposable clone.

macOS's inherited sandbox is the initial execution boundary: clone-only writes,
no external network, no source-client signals or Unix sockets. Unsupported hosts
fail explicitly. Source repositories stay readable and read-only; mutation demos
use repositories created inside the clone. Clear inherited credentials, routing,
claims and provider account selection; pin HOME, LF_HOME, provider homes and LF_BIN.
Desktop launches its executable directly inside the same boundary, inherited by
embedded terminals and nested commands. No global installation or launch service.

Retain copied Session identities and Task-primary relationships for navigation,
but expose copied Sessions as read-only history. Codex and Claude histories are
optional explicit inputs, copied without credentials or executable configuration.
Neither provider has verified independent continuation in this Task: refuse resume,
replace and retries of copied Sessions before launch. New disposable conversations
use explicitly supplied offline provider stand-ins; real-account and configured
native continuation remain unproved. No network exception in this first path.

The macOS process reader now uses the SDK kernel API: privileged `ps` cannot
execute under this boundary. The ordinary Linux reader remains. The performance
snapshot stays benchmark input; documentation distinguishes it from this path.

Remaining: publish, request normal auto-merge, reconcile verified merge, and
complete; follow-up lf-4e4cc84 owns native acceptance.
Native window/PTY draft retention is unproved here. Headless navigation/state and
helper-selection checks are distinct evidence; neither proves a displayed app.

Review findings fixed: SQLite read-only can still modify source SHM unless the OS
boundary is installed before backup; local socket bind must never authorize a
remote source endpoint; Codex startup must capture its native Session ID. Fixture
providers explicitly exercise the hook without an account. Credential removal is
checked in destination database bytes, not just rows.

Checks: cargo fmt/clippy, architecture map, clone(3), Session CLI(21), documented commands(3), human Session(29), migration(98), process ownership(11; 3 existing ignored), Desktop launcher/navigation(40) pass; native process identity(4) passes; configured native proof belongs to lf-4e4cc84.
