# LOO-661: Save refreshed Claude credentials reliably

Status (2026-10-28): **Implementation incomplete; native acceptance fails.**
An isolated mocked-refresh fixture now invokes the actual native writer through
account verification. All eight cases execute and clean up; ordinary ASCII
replacement passes, large replacements destroy old data, and Unicode exposes a
readback encoding failure. The defective `security -i` writer remains, with no
eligible replacement established. This is a failing acceptance boundary, not a repair.

Reconciled against `b909e2f71` on 2026-10-28: finding 38's missing boundary is
implemented in `1abbf832a` and simplified in `b909e2f71`. The writer candidate remains
excluded, and findings 39–40 retain the resulting failures. No acceptance constraint
or delivery boundary changed.

Jack Heart authorized bounded repair and autonomous delivery on 2026-10-27 for
[LOO-661](https://linear.app/loopflow/issue/LOO-661/save-refreshed-claude-credentials-reliably-on-macos).
Infrastructure owns release, CLI/Desktop/helper installation and installed acceptance.
LOO-325's refresh-concurrency redesign remains unselected; preserve LOO-312's onboarding
choices and integrated LOO-657 takeover notices. No quantitative Wave target is selected.
Source integration of LOO-664, LOO-665 and v0.15.0 establishes neither installation nor
Claude recovery. The temporary sync bridge and archive receipts live in
[questions.md](questions.md).

## Required outcome and next decision

**Demo:** after a naturally required refresh, `lf account claude --json` accepts
the same identity with capacity, and native Claude continues using that login.
Synthetic gate coverage must first demonstrate exact 2,048/8,192-byte replacement
and readable old credentials after failed saves through the production writer.

Repair native persistence while preserving service/account identity, reader format/access,
previously usable credentials on failure and active Sessions. Never expose credentials in
argv, logs, chat or assertions. No global trust changes, real-account deletion, forced
sign-out, account-route changes, alternate credential authority, delete/recreate, file
migration or unprotected post-commit ACL repair.

Two independent gaps remain:

- **Writer mechanism:** findings 35–37 identify no supported creation with the
  necessary partition authority or replacement retaining the encoding group/access.
  A password alone is unproved. Before implementation, select a mechanism addressing
  partition authority and failed-save preservation together, then test it on disposable
  items. This plan records requirements and evidence, not an executable design.
  Ordinary repair is authorized; no concrete product choice is ready for decision.
  Private interfaces, trust changes and provider-owned refresh remain unaccepted
  alternatives. Neither public-API exhaustion nor a scope change is established.
  Finding 39 closes finding 38's missing test boundary using test-only native
  dispatch for one disposable home per child. HTTP stays mocked; the writer,
  reader, active-home selection and account-report consumers are real.
- **Live diagnosis:** jack@loopflow.studio needs browser reconnection; `invalid_grant`
  returns before `write_claude_login`. The historical save failure remains unexplained.
  Reconnection cannot prove a writer, and local rollback cannot undo server-side
  OAuth rotation: refresh and saving share no transaction.

Diagnostics and repeated rejected probes cannot close these gaps or justify shipping.

## Surviving implementation

- `provider_auth/mod.rs::{read_claude_login,write_claude_login}` select Keychain for
  native macOS homes and private `.credentials.json` files for managed homes. Ordinary
  unit tests use files. Native read errors propagate; an absent item permits file fallback.
- Scoped services use `Claude Code-credentials-<first eight SHA256 hex digits of config
  path>`; the ambient default home falls back to unsuffixed `Claude Code-credentials`.
  Existing account metadata wins; `USER` supplies new items. Readers use `security`.
- `subscription.rs::fresh_claude_token` retains the original `SecretString`, serializes
  once, checks intervening replacement and maps save failures to `Unavailable`.
  `claude_persistence_error` retains only closed storage stages/numeric statuses;
  other errors remain redacted. Readback failure and mismatch prove no rollback or
  safe retry. Input/wait failures reap the child, but waits remain unbounded.
- `provider_account/activation.rs::write_login` shares the writer;
  `lf/commands/account_status.rs::managed_rows` observes and polls native identity.
  Existing diagnostic/recovery fields carry failures; SQLite evidence is derived.
  Four diagnostic tests prove redaction, not preservation. No migration, DTO or Swift change.
- The explicit native fixture exercises refresh, native storage, identity, usage and
  account reporting with mocked HTTP. Its eight cases cover scoped replacement,
  two account-name lengths, Unicode and invalid-input rejection. It observes old
  bytes after each case, but only invalid-input rejection currently passes a
  failed-save preservation assertion. Account metadata equality proves neither ACL
  nor encoding-group preservation. Creation through the production writer, ambient
  selection and injected storage failures remain uncovered.

## Evidence and excluded alternatives

Full findings, citations, dependency history and archive chain:
`108095b26cfe5950c7ce42b6115e936601547ede:scratch/loo-661-save-refreshed-claude.md`,
“Evidence and excluded alternatives.” Findings 1–37 retain that lookup.
This archive also points to retired harnesses and complete source reviews.

Probes used disposable `loopflow-LOO-661-*-probe-<uuid>` items, synthetic bytes,
bounded/reaped children, captured output and successful cleanup. They neither
read real credentials directly nor changed routes, Sessions or global trust.
Source conclusions use Apple Security `db15acbe6a7f257a859ad9a3bb86097bfe0679d9`;
they establish neither installed binary equivalence nor public-API exhaustion.

- **Transport (1–2, 30):** `security -i` has a 4,096-byte input buffer. Early
  64/1,242-byte round trips passed and a 2,048-byte failure retained old bytes.
  Later exact-command probes twice contradicted preservation at 2,048/8,192 bytes
  with five-/six-character accounts: write exit 1, read exit 0, neither old nor
  intended bytes. They did not invoke compiled Rust. Published parsing executes
  each buffer read, consistent with partial replacement then error; installed
  parsing was not instrumented. Detached `security … -w` returned zero but truncated
  larger values to 128 bytes. PTYs, file/FD indirection and repeated `-w`/`-X` do
  not append a password across commands.
- **Native access (3–5, 8–9, 11, 21, 27–29, 31, 33):**
  `SecKeychainItemModifyAttributesAndData` returned zero at 2/8 KiB, but independent
  reads exited 128. Writes replaced `apple-tool:` with writer `cdhash:`;
  published non-null updates create a group and strip partition/integrity ACLs.
  `ModifyContent`, Apple-signed `osascript`/`SecItemUpdate` and copied-access
  restoration supplied no safe replacement. `security-framework` 3.7.0 uses the
  same modifier. Partition edits failed `-25293` with UI disabled; JXA's initial
  `-50` was a bridge error. Signing/trusting the writer grants no partition
  authority. Combined data/access updates modify data before access without
  encompassing rollback; pre-setting access affects only the old group. Public
  creation rejects duplicates. Null-data updates leave bytes unchanged.
- **Other interfaces (10, 12–20):** raw `CSSM_DL_DataModify` bypasses encryption;
  `CSSM_EncryptData` needs private envelope composition; `SecKeychainSetBatchMode`
  is private. Import handles keys/certificates; copy strips partition/integrity
  access on group creation. Read-only macOS 26.0.1 dispatch inspection connected
  `KCSetData`/`KCUpdateItem` to staged new-group paths. Shared web credentials,
  TN3137's file shim and data-protection storage supplied no compatible atomic
  replacement. The TN3137 backup-entropy change left this conclusion unchanged.
- **Outer transactions (22–26):** FileDL deferred commit of successful 2/8 KiB
  modifications, but password reads failed `-25300` while metadata succeeded.
  By-ID reads saw new encoded data; queries saw the old snapshot without its
  decoding group, preventing pre-commit verification. Rollback/autocommit
  restoration recovered old bytes, not proof of unchanged ACL identity or general
  rollback safety. No commit/access-restoration probe ran.
- **Copied-access creation (34):** `SecKeychainItemCreateFromContent` failed
  `-25293` at both sizes with UI disabled; originals stayed exact and candidates
  were absent (44). No promotion/transaction was tested. This rejects copied
  access, not every explicit-access construction.
- **Explicit access (35):** eight 2,048/8,192-byte cases reconstructed access with
  `SecAccessGetOwnerAndACL` / `SecAccessCreateFromOwnerAndACL`, or used
  `SecAccessCreate` trusting `/usr/bin/security`. Omitting integrity only failed
  creation `-25293` (candidate absent, 44). Omitting partition, both partition and
  integrity, or using fresh trusted access created candidates but independent
  readers timed out at 15 seconds. Those candidates lacked original `apple-tool:`
  partition access. All originals retained exact bytes and semantic ACL fingerprints
  (not group identity); none was replaced. No promotion/transaction followed.
  Published `changeAcl` requires database-authenticated credentials for partition
  edits, while creation supplies temporary group credentials. Integrity installation
  also checks partition membership. Earlier `SecACLRemove` failure `-67702` reflects
  an invalid-form tombstone, not a contradiction of fresh construction.
  Corrected probe and receipts:
  `66ae255389e2800111dfc056c5009d380bab512a:scratch/keychain-access-probe/`.
- **Authentication/group retention (36):** pinned-source review found
  `KeychainDatabase::checkCredentials` requires a passphrase/key in the ACL operation's
  `KEYCHAIN_LOCK` sample even when unlocked. It ignores prompt samples and
  `CSSM_DL_Authenticate`'s `mCred`; `SecKeychainUnlock` does not authorize later ACL
  edits. Creation supplies a random setup-password sample and accepts no database
  credential argument. `SSDbUniqueRecordImpl::modify` can retain `SSGroup`, but
  `ItemImpl::updateSSGroup` reaches it only with null new data. Public
  `SSDLSession::DataModify` dispatches through raw `DbUniqueRecordImpl`, not that
  encoder. Retaining references cannot alter dispatch; internal C++/envelope
  reconstruction crosses the private-interface boundary.
- **Local authentication context (37):** read-only documentation/source review
  found LAContext externalization in the data-protection implementation. The
  separate file-keychain update modifies content before access, without handling
  `kSecUseAuthenticationContext` or `kSecUseCredentialReference`. This identifies
  no context-to-database-credential bridge or atomic preservation path; it is not
  an installed securityd trace or public-API impossibility proof. Findings 36–37
  performed no authentication, unlock, mutation or real credential read.
- **Live observations (6, 32):** launches reported a write/readback failure without
  status and two expired OAuth errors. Later 0.14.18/0.14.19 polls accepted both
  managed accounts with capacity, including three verified windows at `1793163454`;
  the local row stayed unavailable. Polls establish no refresh, size or causality.
  On October 27 at `1793169073`, supported `lf account claude --json` rejected
  jack@loopflow.studio with recovery/no windows; an allowlisted diagnostic at
  `1793169147` confirmed `refresh grant rejected` (HTTP 400 `invalid_grant`, before
  the writer). jackstah@gmail.com retained three windows. This diagnoses neither
  the historical save failure nor token invalidation. No token was inspected,
  forced to expire or put in argv. Supported
  `lf account connect claude jack@loopflow.studio` needs browser sign-in; no sign-in
  or route change was attempted. Further polling without recovery adds no acceptance.
- **Earlier acceptance gap (38, 2026-10-28; closed by 39):** source inspection at `f1daf112a`
  confirms `read_claude_login` and `write_claude_login` compile out their native
  dispatch under `cfg(test)`. The direct writer diagnostic fixture substitutes
  `security` through PATH, while `fresh_claude_token` tests use files. Combining
  those passing suites would still leave the refresh-to-native-save boundary
  untested. A standalone test binary alone does not solve this: production OAuth
  URLs ignore the test endpoint variables. Finding 39 resolves this with the
  library test build's mocked HTTP and a test-only native-home selection, without
  adding a second writer or enabling production endpoint overrides.
  This finding adds no native experiment or evidence about the live failure.
- **Production native acceptance (39, 2026-10-28):** the explicit
  `native_refresh_replaces_exact_bytes_and_preserves_failed_saves` test executes
  eight isolated cases through the compiled native implementation. Two 512-byte
  ASCII replacements pass exact readback, identity and usage with one refresh each.
  Both 2,048/8,192-byte cases with a five-character account overwrite old data with
  2,010 bytes; both long-account cases leave 1,994 bytes. Account reporting returns
  sanitized persistence `unavailable` without reconnect advice; old bytes are lost.
  Invalid JSON is rejected before writing and old bytes remain exact. All services
  were absent before setup and absent after cleanup; account names remain unchanged.
  Parent/children use the shared environment lock, loopback-only network boundary,
  captured output and 15-second process-group deadlines. Each child's only native
  home is synthetic; ordinary tests keep file storage and production has no test
  endpoint/backend switch. No real OAuth request, credential read or Session change.
- **Unicode reader failure (40, 2026-10-28):** the Unicode case is a
  512-byte refresh preserving an unrelated Unicode field. It writes the exact new
  bytes but reports persistence failure: `/usr/bin/security -w` renders this value
  as hex, while the production reader expects raw JSON. The fixture's independent
  observer decodes that rendering and compares bytes without printing them. The old
  seed uses an ASCII JSON Unicode escape, so native reading and refresh start normally.
  This is distinct from the large-value truncation; it also violates failed-save
  preservation. [Apple's reader source](https://github.com/apple-oss-distributions/SecurityTool/blob/main/keychain_find.c)
  documents the rendering branch; installed fixture results establish this case.

## Writer decision after native reproduction

The concrete combination assessed on October 28 is native content replacement
inside a FileDL transaction, followed by password-authorized restoration of the
copied access/partition ACL before commit, rolling back on either failure. It
would need to address both authority and preservation, rather than repair ACLs
after an unprotected commit. **It is not an accepted or proven implementation.**

The password-taking entry is declared in Apple's private
[`SecKeychainItemPriv.h`](https://github.com/apple-oss-distributions/Security/blob/db15acbe6a7f257a859ad9a3bb86097bfe0679d9/OSX/libsecurity_keychain/lib/SecKeychainItemPriv.h)
(`SecKeychainItemSetAccessWithPassword`, lines 190–199). Its implementation supplies
passphrase credentials to the ACL edit. The accepted public-interface constraint excludes that candidate;
no password was requested or read, no private entry invoked, and no previously
rejected mutation probe repeated. A transaction alone supplies no partition
authority, and findings 22–26 still leave group/ACL rollback and pre-commit
verification unproved. The fixture is ready to judge an eligible writer, but no
mechanism meeting the existing constraints has been established. Writer implementation
and delivery remain incomplete; this does not establish API exhaustion or authorize
a scope/trust/storage change. No concrete product decision is ready for acceptance.

## Remaining implementation and acceptance

1. Select and prove an eligible writer mechanism before replacing the production
   transport and exclusive fixture. The password-taking candidate above is excluded.
   Preserve JSON validation, readback, managed login locking, replacement checks,
   omitted refresh tokens, unknown fields and the 15-second HTTP timeout. No extra
   OAuth request or automatic retry. Bound new subprocesses to 15 seconds, close
   stdin and reap on every exit. Report uncertain commits and re-read before another
   write; storage failure alone never requires sign-in or deletion.
2. Correct native reader handling of `security -w`'s hex-rendered Unicode bytes,
   retaining exact JSON bytes and unrelated fields. This distinct readback defect
   remains even with a large-value writer; fixing it alone supplies no failed-save
   preservation or complete repair.
3. Complete the existing native fixture's acceptance: exact ordinary/2,048/8,192-byte
   production creation and replacement, Unicode/escaping, scoped/ambient selection,
   account/access preservation and varied service/account prefix lengths. Disposable
   existing items retain `security`'s default access; candidate and existing readers
   must return exact bytes without intervention. Extend invalid-input coverage with
   injected pre-commit, denied/locked, partition and partial-commit failures, asserting
   old readable bytes and sanitized `unavailable` on failed saves. Keep managed-file,
   replacement-race and native-activation coverage. Output stays limited to
   booleans/lengths; no unproved candidate reaches real credentials. Missing native
   infrastructure remains a failed check; Linux cannot replace macOS proof.
4. Diagnose recurrence through supported installed account operations, recording
   sanitized stage/code, storage kind and whether refresh occurred. Never force
   expiry, revoke tokens, switch routes or damage a real item. Keep recovery docs
   aligned; report browser dependency without bypassing it. Infrastructure's demo
   must meet the demo above. Unexpired polls and synthetic success cannot
   establish live acceptance or complete the Task.

## Replacement boundary

- Retired command-boundary and `scratch/keychain-access-probe/` harnesses stay in
  Git at the references above. Do not restore them as acceptance coverage or
  extend rejected constructions without a mechanism addressing partition authority
  and failure preservation.
- Replace the `security -i` spawn, hex script and command-string restrictions in
  `write_claude_keychain_credential` together with its exclusive stand-in in
  `claude_persistence_keychain_diagnostics_distinguish_write_readback_and_mismatch`.
  Retain stage/secrecy checks on the surviving writer. Keep one writer behind
  `write_claude_login`, shared by refresh and activation; no small-value fallback,
  size dispatch, dual writes or bypass. No production deletion is ready until a
  compatible replacement is proved.
- Retain shared readers, service/account parsers/tests, managed-file writes,
  explicit profile-capture cleanup and race/rotation tests; they have other consumers.

## Verification and delivery

Gate owns affected suites, formatting and `cargo clippy --all-targets -- -D warnings`.
The macOS CI classifier selects `macos-process-test` for `provider_auth/mod.rs`,
which does not run this explicitly ignored native fixture. With inherited `LF_*` unset
and stdin `/dev/null`, run:

```sh
uv run python scripts/test.py --reuse-passing --rust-filter 'test(provider_auth::) | test(subscription::) | test(provider_account::) | test(lf::commands::account_status::) | test(lf::commands::account::)'
```

The explicit native boundary (currently red; asserts all eight cases executed):

```sh
uv run python scripts/test_network.py cargo test -p loopflow --lib native_refresh_replaces_exact_bytes_and_preserves_failed_saves -- --ignored --nocapture < /dev/null
```

Publish/ship only the complete repair through the selected Graph,
without another release publisher or repeated checks merely for a lifecycle change.

Check — `git diff --check`: PASS; reused `b909e2f71` native command above: FAIL (8 cases, 5 storage failures, all cleaned; build passed with an ld unwind-size warning), `cargo fmt` / `cargo clippy --all-targets -- -D warnings`: PASS. No code changed; gate retains complete acceptance and Infrastructure installed verification.
