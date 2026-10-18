# Offline data clones

```sh
cargo build -p loopflow --bin lf
./target/debug/lf clone create /tmp/lf-demo --source "$LF_HOME" \
  --codex-home "$CODEX_HOME" --claude-home "$CLAUDE_CONFIG_DIR"
./target/debug/lf clone show /tmp/lf-demo
./target/debug/lf clone run /tmp/lf-demo -- /bin/zsh -f
# Inside this shell, lf and its nested commands use the clone.
lf session list --all --history --json
```

Supply the source Machine directory explicitly (`~/.lf` for a default installation).
Omit either provider-home option when that history is unavailable. Empty environment
variables are not paths: replace the examples with the configured directories.
The destination must be new and outside all source directories. This path is
verified on macOS 26. macOS 15 cannot re-enter the inherited sandbox and is
unsupported. Creation probes this capability before opening source data and
reports unavailable instead of creating a seemingly usable snapshot.

`create` backs up SQLite under the macOS sandbox, applies this build's pending
canonical migrations and drafts to the destination, removes credential tokens and
provider routes, clears copied process PIDs/endpoints/attachment tokens, and gives
the clone a fresh Machine identity. Copied Machine connections lose their routes.
Historical IDs, planning relationships and Task-primary selection remain. Existing
Task admissions are history; they do not authorize execution on the new Machine.

The manifest lists retained capture files and Codex/Claude JSONL history counts,
alongside omissions and changed files. Credentials, provider configuration, live
sockets and symlinks are excluded. Native history is copied to `archive/`; Loopflow
capture artifacts remain in `runs/`. Provider history is a best-effort file capture,
not an atomic snapshot with SQLite. Private per-account histories need their own
explicit source home; absence is never evidence of an empty conversation.

Copied Sessions display an unavailable continuation action. Connecting, replacing
or resuming them fails before provider launch. Neither Codex nor Claude has a
verified independent native-history continuation path here. Archive copies do not
claim a supported provider fork. Read their retained history or start a new offline
conversation. No account probe, login or credential transfer takes place.

`run` clears inherited credentials and pins `HOME`, `LF_HOME`, `LF_BIN`, provider
homes, temporary files and executable search paths. Descendants inherit the OS
boundary even if they alter environment variables. Selecting the copied `bin/lf`
also selects its clone, including from a shell with the source `LF_HOME`. Files outside the clone are
read-only except its private temporary transport directory. External networking,
source Unix sockets, source-client signals and Apple-event/most Mach service calls
are denied. Unix sockets are available inside the clone and its transport directory;
the latter uses a short path to fit the OS socket limit. Real providers requiring
external APIs cannot operate in this mode.
No unsupported-platform or unsandboxed fallback exists.

Source repositories keep their original paths and remain read-only. Create a
repository inside the clone for mutation tests. Git publication (including local
remotes outside the clone), external planning, scheduling and installation cannot
write outside the boundary. Failed operations report their actual errors.

## Disposable conversation and Desktop recipe

Place a deliberately selected offline provider executable at `/tmp/lf-demo/bin/claude`
or `/tmp/lf-demo/bin/codex`. Nothing discovers or copies an installed provider.
A tiny startup stand-in proves wiring without an account:

```sh
cat > /tmp/lf-demo/bin/claude <<'SH'
#!/bin/sh
printf 'Offline conversation ready. Type a disposable draft: '
read -r draft
printf 'Received: %s\n' "$draft"
SH
chmod 755 /tmp/lf-demo/bin/claude
./target/debug/lf clone run /tmp/lf-demo -- /tmp/lf-demo/bin/lf -i -a claude : 'Disposable conversation'
```

Build the matching development Desktop bundle without installing it. Launch its
executable directly through the boundary; `open` targets an existing application
and is not a clone launcher:

```sh
./target/debug/lf clone run /tmp/lf-demo -- /path/to/Loopflow.app/Contents/MacOS/Loopflow
```

Desktop selects the clone's pinned helper before its bundled/development helper.
Embedded terminals retain the same Machine and provider paths. Browse repository,
Wave and Task rows; the retained Task primary shows its continuation limit. Use
new disposable conversations to exercise draft switching. Native window/PTY draft
retention and configured account continuation require separate acceptance: the
headless fixtures do not prove them. Task lf-4e4cc84 owns this native acceptance.
The restrictive Mach policy may expose
additional native service requirements. Never loosen the policy on production data.

Close clone clients, then remove the owned directory:

```sh
./target/debug/lf clone dispose /tmp/lf-demo
```

A different schema/build may refuse an existing clone. Recreate it; ordinary
opens remain validation-only and no clone marker grants production migration
permission. Draft SQL never enters the published migration ledger. Creation
failure leaves an incomplete private directory that `dispose` can remove.
The performance script's SQLite snapshot remains benchmark input, not this
isolated execution environment.

## Proof and remaining acceptance

Disposable fixtures cover populated navigation and Task-primary selection, copied
Codex/Claude refusal, fresh stand-in conversations (including Codex's native-ID
callback), migration, repeated entry, and disposal. They compare source DB/WAL/SHM,
provider history and repository state, and attempt forbidden writes, signals,
network and source-socket connections. Credential removal is checked in database
bytes. These are synthetic-provider proofs, not configured native acceptance.

An unconfined SQLite read-only connection can change source shared memory; creation
therefore installs the write boundary before backup. Privileged macOS `ps` cannot
run inside it; the shared process reader now uses the SDK kernel API, retaining
birth identity and zombie checks. Neither failure justifies relaxing isolation.
