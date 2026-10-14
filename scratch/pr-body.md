A raw SQLite copy retains provider paths and live-client claims, and a development binary cannot migrate the source installation. `lf clone create/run/show/dispose` now creates a writable disposable Machine with an inherited macOS isolation boundary.

## What changes

- Back up SQLite under source write protection; apply canonical migrations and this build’s drafts only to the new destination. Draft SQL gains no published ledger provenance, and ordinary opens remain validation-only.
- Remove copied credentials, routes, PIDs, endpoints and attachment tokens; retain planning identities, Session history and Task-primary selection. Codex/Claude JSONL archives are explicit optional inputs. Copied continuation is truthfully unavailable; fresh offline conversations work with deliberately supplied stand-ins.
- Confine descendant writes, networking, Unix sockets and process control. Pin Desktop’s helper, embedded-terminal environment and direct copied-executable selection to the clone. Source repositories remain read-only.
- Replace privileged macOS `ps` with the SDK kernel reader, retaining birth/zombie evidence. Linux keeps its existing reader. Add macOS CI coverage for the clone boundary.

## Limits and review findings

Verified on macOS 26; macOS 15 cannot re-enter the inherited sandbox and is unsupported. Creation probes capability before reading source data, leaves an incomplete disposable directory on refusal, and never substitutes environment trust for kernel enforcement. There is no unsandboxed fallback. Native window/PTY draft switching remains unproved and is captured in follow-up Task lf-4e4cc84. No configured provider continuation, real-account probe, global installation or production migration is claimed.

Review caught and fixed source SHM writes from an unconfined read-only SQLite connection, Unix socket permission scope, copied-helper fallback, and Codex’s required native-ID callback. The fixtures preserve source DB/WAL/SHM, histories, files, repository state and live clients. They test credential-byte removal and forbidden writes, signals, network and source sockets. CI’s outer sandbox cannot nest its privileged `ps` exception with this boundary; these fixtures establish their own sandbox directly.

## Checks

- Formatting, `cargo clippy --all-targets -- -D warnings`, architecture map and diff checks pass.
- Rust: 4 clone, 21 Session CLI, 3 documented-command, 29 human-Session, 98 migration, 11 process-ownership and 4 native process tests pass; 3 existing process tests remain ignored.
- Headless Desktop launcher/navigation: 40 tests pass. Synthetic providers prove fresh Codex and Claude startup; the Codex fixture records its native ID through the real callback.
- Main’s parallel release integrated without conflicts. Hosted CI owns the full materialized matrix. CI covers early refusal on macOS 15 and full operation on macOS 26; both passed on the previous PR head. The 35 CI aggregation checks pass locally. A merge-queue failure exposed a recorder fixture racing its bounded 250 ms production drain; the fixture now waits for the existing acknowledgement. Its focused test and lint pass, without retries or production changes.

## Try it

Follow [the offline clone recipe](https://github.com/loopflowstudio/loopflow/blob/cf2b5fb44e3a09ba7b17493c246c700e46927e6a/docs/data-clones.md): create a new directory from an explicit source Machine, inspect its inventory, and enter it with `lf clone run`. Browse retained history or supply an offline executable for a new disposable conversation. The Desktop launch recipe is a suggested native acceptance path, not a claimed native demonstration.