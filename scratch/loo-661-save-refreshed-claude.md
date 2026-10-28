# LOO-661: Save refreshed Claude credentials reliably

Status (2026-10-28): **Implementation stopped at the writer design boundary;
native acceptance fails.**
The reader now decodes Unicode readback exactly. All eight mocked-refresh/native
storage cases execute and clean up: ordinary ASCII, Unicode and invalid-input
preservation pass; four large replacements still destroy old data. The defective
`security -i` writer remains, with no eligible replacement established. The reader
fix does not complete this repair or permit delivery.

Reconciled October 28 against `5eead1b8b`: implementation at `ed6c82b2b` closes
finding 40 with finding 41's Unicode proof. Finding 42 excludes only the examined
public CSSM construction, without mutation or proof of API exhaustion. Finding
38's boundary was implemented in `1abbf832a` and simplified in `b909e2f71`.
Finding 43 assesses public key enumeration as an alternative to finding 42's
private-layout mapping; it supplies no eligible pre-commit path. No acceptance
constraint or delivery boundary changed. Release's operation-entry
lesson applies: native synthetic proof and installed live acceptance are separate.

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
  Ordinary repair is authorized; the specific missing capability and next scope
  decision are recorded below. No replacement writer is ready for acceptance.
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
  `decode_security_password_output` removes only the tool's appended LF and decodes
  its all-hex rendering, preserving exact Unicode and whitespace bytes. JSON objects
  cannot be confused with unencoded all-hex text; invalid JSON remains invalid.
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

Complete findings 1–42, citations, rejected mechanisms, receipts and earlier
archive links remain at
`ed6c82b2bc02e3f9b0d6f3ca21a5a15179e33f4f:scratch/loo-661-save-refreshed-claude.md`,
“Evidence and excluded alternatives” and “Writer decision after native reproduction.”
This summary keeps the constraints needed to avoid repeating those probes; consult
that revision before proposing a variation. No public-API exhaustion is proved.

Native probes used unique disposable services, synthetic bytes, bounded/reaped
children and captured output; cleanup succeeded. They did not read real credentials
or change routes, Sessions or global trust. Source findings use Apple Security
`db15acbe6a7f257a859ad9a3bb86097bfe0679d9`; source inspection does not prove installed
binary equivalence. Read-only findings 36–37 and 42 added no mutation evidence.

- **Transport (1–2, 30):** `security -i` has a 4,096-byte input buffer. Initial
  64/1,242-byte round trips passed and one 2,048-byte failure preserved old bytes;
  later command probes contradicted preservation at 2/8 KiB. Published parsing
  executes each buffer read, consistent with partial replacement then error, but
  installed parsing was not instrumented. Detached `security … -w` truncated to
  128 bytes despite exit zero. PTYs, file/FD indirection and repeated flags supplied
  no append mechanism. Finding 39 establishes the compiled Rust failure below.
- **Native modification (3–5, 8–9, 11, 21, 27–29, 31, 33):**
  `SecKeychainItemModifyAttributesAndData` returned zero at 2/8 KiB but independent
  reads exited 128. Updates replaced `apple-tool:` with writer `cdhash:`; non-null
  data creates a new group and strips partition/integrity ACLs. `ModifyContent`,
  Apple-signed `osascript`/`SecItemUpdate`, copied-access restoration and
  `security-framework` supplied no safe replacement. Signing/trusting the writer
  grants no partition authority. Combined data/access updates lack encompassing
  rollback; pre-setting access affects only the old group. Null data changes no
  bytes, and creation rejects duplicates.
- **Creation/access (34–35):** copied access and reconstructed access omitting only
  integrity failed `-25293`, leaving candidates absent. Omitting partition, omitting
  both ACLs, or fresh access trusting `/usr/bin/security` created candidates whose
  independent reads timed out at 15 seconds. Originals retained exact bytes and
  semantic ACL fingerprints, not proven group identity; no promotion followed.
  `SecACLRemove`'s earlier `-67702` was an invalid-form tombstone, not a contrary
  result. Corrected probe/receipts:
  `66ae255389e2800111dfc056c5009d380bab512a:scratch/keychain-access-probe/`.
- **Transactions (22–26):** FileDL deferred successful modifications, but password
  reads failed `-25300`; metadata and by-ID encoded-data reads succeeded. Queries
  saw an old snapshot missing its decoding group. Rollback/autocommit restoration
  recovered old bytes without proving ACL/group identity or general rollback safety.
  No commit/access-restoration probe ran; pre-commit verification remains unproved.
- **Authority (36–37):** partition edits require an ACL operation's passphrase/key
  `KEYCHAIN_LOCK` sample even when unlocked. Prompt samples, `CSSM_DL_Authenticate`
  and `SecKeychainUnlock` supplied no bridge; creation uses temporary group
  credentials. Internal group-preserving modification is reached only with null
  data; public raw modification bypasses the encoder. File-keychain updates do
  not handle the reviewed LAContext credential attributes. Neither reference
  retention nor data-protection authentication proves a compatible write path.
- **Excluded interfaces (10, 12–20, 42):** raw CSSM modification bypasses encryption;
  envelope reconstruction and `SecKeychainSetBatchMode` cross the private-interface
  boundary. Import, copy, shared web credentials, TN3137's file shim and
  data-protection storage supplied no compatible atomic replacement. Public
  `CSSM_ChangeKeyAcl` needs a `CSSM_KEY`; the examined password-item mapping relies
  on private `aclBearer`/SSGroup layout. Its public database-handle/record-ID bridge
  does not establish that mapping or prove every CSSM construction impossible.
  The proposed FileDL transaction plus password-authorized ACL restoration uses
  private `SecKeychainItemSetAccessWithPassword` and is excluded. No password was
  requested/read or private entry invoked. A transaction alone grants no authority.

**Native acceptance (38–41, October 28):** ordinary file and stand-in tests missed
refresh-to-native persistence. The existing explicit fixture closes that boundary
through mocked HTTP and one disposable native home per isolated child, with no
production backend/endpoint switch. All eight cases execute and clean up. Two
512-byte ASCII replacements pass identity/usage after one refresh; invalid JSON
is rejected before writing and preserves the old bytes. Four 2,048/8,192-byte
replacements leave 2,010 bytes with short accounts or 1,994 with long accounts,
losing old data and reporting sanitized `unavailable` without reconnect advice.
Account metadata remains unchanged, which proves neither ACL nor group retention.

The Unicode case originally wrote exact bytes but failed readback because
`security -w` hex-renders non-printable bytes. Finding 41's decoder fixes that
512-byte case: exact readback, identity/usage and cleanup now pass. The observer
decodes independently; unit coverage retains escaping, whitespace and invalid
output. This reader repair changes none of the writer failures or preservation gaps.

**Public key enumeration (43, October 28; source review only):** assessed a
different bridge to `CSSM_ChangeKeyAcl`: enumerate symmetric keys before and after
content replacement inside the FileDL transaction, then authorize the newly
created key without decoding the password record's private envelope.

- [KCCursorImpl::next](https://github.com/apple-oss-distributions/Security/blob/db15acbe6a7f257a859ad9a3bb86097bfe0679d9/OSX/libsecurity_keychain/lib/KCCursor.cpp#L255-L281)
  filters group keys out of higher-level searches. Those searches do not expose
  the required `SecKeyRef` bridge.
- [SSDLSession::DataGetFirst](https://github.com/apple-oss-distributions/Security/blob/db15acbe6a7f257a859ad9a3bb86097bfe0679d9/OSX/libsecurity_apple_cspdl/lib/SSDLSession.cpp#L303-L367)
  does return `CSSM_KEY` references for lower-level symmetric-key queries. Thus
  finding 42 must not be interpreted as saying public APIs cannot return such
  keys. Returning keys still does not associate one with the target password.
- [DbModifier::createCursor](https://github.com/apple-oss-distributions/Security/blob/db15acbe6a7f257a859ad9a3bb86097bfe0679d9/OSX/libsecurity_filedb/lib/AppleDatabase.cpp#L1753-L1791)
  queries the pre-modification snapshot during a transaction; by-ID reads use
  modified records. `AppleDatabase::dataGetFirst` uses that cursor. Therefore the
  proposed before/after query cannot discover the new group key before commit
  through this path. The content-update API does not return that key's ID.

Committing to make the new key discoverable would move partition repair outside
the preservation boundary. Guessing record IDs, parsing group labels from the
password envelope, or using internal object access would restore the private
dependency that this candidate was meant to remove. No enumeration or mutation
was executed against any Keychain. The two locally examined implementation files
match fresh downloads at the pinned revision; this is source evidence, not an
installed-binary trace or public-API exhaustion. Database authentication and full
group/ACL rollback remain unproved even if a different bridge is found.

**Live diagnosis (6, 32):** the original save/readback failure had no status.
Subsequent 0.14.18/0.14.19 capacity polls proved neither refresh nor causality.
On October 27, supported `lf account claude --json` rejected jack@loopflow.studio
at `1793169073`; an allowlisted diagnostic at `1793169147` confirmed HTTP 400
`invalid_grant` before saving. jackstah@gmail.com retained three windows. No token
was inspected or forced to expire. Supported browser reconnection through
`lf account connect claude jack@loopflow.studio` remains a dependency; no sign-in
or route change was attempted. Neither historical save causality nor the reason
for token invalidation is established. Repeating polls without recovery adds no proof.

## Writer design decision needed

The missing capability is **a public file-Keychain operation that replaces
arbitrary password bytes while retaining the existing readers' partition access,
or a public way to authorize the replacement's partition before a rollback-safe
commit**. For the examined transaction approach this specifically requires a
supported password-to-new-key reference and database credentials on that ACL
operation, plus proof that failures preserve the old readable bytes and access.
An unlocked Keychain, a copied access object, or a returned key handle alone does
not supply that combination.

The current scope remains the safe default. Further implementation needs a
supported mechanism answering those requirements, then the existing native
fixture's exact large-value and injected-failure proof. The concrete decision for
Jack Heart and Infrastructure is whether to obtain Apple guidance for that
public-interface path, or commission a separately reviewed provider-owned refresh
design under LOO-325 if no supported path can be established. This step neither
contacts Apple nor selects LOO-325. Provider-owned refresh is an investigation
choice, not a proven fix, and does not waive the storage, identity, secrecy, trust,
Session or failed-save requirements. Private-API use and trust changes remain
outside the accepted design.

Implementation cannot advance to delivery on the reader repair. Browser
reconnection remains a separate dependency for live diagnosis; Infrastructure
retains installation and final installed verification. No new production change
or native mutation is justified by finding 43.

## Remaining implementation and acceptance

1. Select and prove an eligible writer mechanism before replacing the production
   transport and exclusive fixture. The private password-taking candidate is excluded.
   Preserve JSON validation, readback, managed login locking, replacement checks,
   omitted refresh tokens, unknown fields and the 15-second HTTP timeout. No extra
   OAuth request or automatic retry. Bound new subprocesses to 15 seconds, close
   stdin and reap on every exit. Report uncertain commits and re-read before another
   write; storage failure alone never requires sign-in or deletion.
2. Retain finding 41's passing Unicode coverage while replacing the writer. Reader
   decoding alone supplies no failed-save preservation or complete repair.
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

## Delete — do not maintain

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
- Removed `trim_security_password_output` and its trim-only test; the surviving
  decoder owns exact output handling and the native observer decodes independently.

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

Check (October 28 implementation; source review and prose only) — `git diff --check` and `lf context`: PASS; no Rust changes or test reruns. Prior `ed6c82b2b` decoder: PASS (1), native acceptance: FAIL (8 executed, Unicode passes, 4 large-value failures, all cleaned), format/Clippy: PASS. Gate retains affected suites and complete acceptance; Infrastructure retains installed verification.
