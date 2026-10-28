# LOO-661: Save refreshed Claude credentials reliably

Status: **Diagnostics only; replacement writer unproved.**
Jack Heart accepted bounded repair and autonomous delivery on 2026-10-27 for
[LOO-661](https://linear.app/loopflow/issue/LOO-661/save-refreshed-claude-credentials-reliably-on-macos).
Investigation baseline: `0105aa8688764d08d7cdb4a685bbdaccb3aa91a8`;
Full plan and evidence before this reduction:
`e3f012685c3fa12791da4deac9b71b9979576d6f:scratch/loo-661-save-refreshed-claude.md`.
PR #1749's CI selection, LOO-657 and v0.14.19 are integrated;
this establishes no installed upgrade.

## Required outcome

Installed lf 0.14.18 reported `failed to persist refreshed credential` for
jack@loopflow.studio. Repair native persistence while preserving service/account
identity, reader format/access, prior usable credentials on failure and active
Sessions. The separately reproduced size defect does not establish the live cause.

Never put credentials in argv, logs, chat or assertion output. No global trust
changes, real-account deletion, forced sign-out, account-route changes or alternate
credential authority. LOO-325's native refresh-concurrency redesign remains
unselected; preserve LOO-312's onboarding choices and integrated LOO-657 notices.
Infrastructure's primary conversation owns release, CLI/Desktop/helper installation
and installed acceptance. This Task serves its reliable-auth objective and KR about
advancing work without plumbing failures; no quantitative Wave target is selected.

**Next: prove a compatible writer.** No tested replacement preserves reader access
and readable old credentials on failure. The `security -i` ceiling remains;
successful status or post-write repair is insufficient. Keep the accepted outcome
and acceptance sequence below. No product decision or authorization is pending.
Reopen a rejected route only with a mechanism addressing its counterexample;
the exclusions below do not exhaust public interfaces. Diagnostics alone cannot
deliver LOO-661.

## Surviving implementation

- `provider_auth/mod.rs::read_claude_login` and `write_claude_login` select native
  Keychain storage for native macOS homes and private `.credentials.json` files
  for managed homes. Ordinary unit tests use files even on macOS.
- The scoped service is `Claude Code-credentials-<first eight SHA256 hex digits
  of config-directory path>`; the ambient default home falls back to unsuffixed
  `Claude Code-credentials` when no scoped item exists. Existing account metadata
  wins; `USER` supplies new items. Readers use `security find-generic-password`.
- `subscription.rs::fresh_claude_token` retains the original `SecretString` and
  serializes the refreshed value once. Keep `ensure_claude_credential_unchanged`
  and the `claude_persistence_error` redaction boundary.
- Closed storage stages and numeric statuses survive error conversion; arbitrary
  errors stay redacted. Save failures are `Unavailable`, not `NeedsLogin`.
  Readback failure and mismatch differ; neither proves rollback or safe retry.
  Input/wait failures reap the child without claiming rollback.
- `provider_account/activation.rs::write_login` shares the native writer;
  `lf/commands/account_status.rs::managed_rows` observes and polls native identity.
  Existing diagnostic/recovery fields carry failures. SQLite account evidence is
  derived, not credential authority.

Four diagnostic tests cover hostile output, mismatch, numeric IO errors and
unstructured errors; they prove redaction, not native preservation. No replacement
writer or native acceptance fixture exists. This repair adds no migration, DTO or
Swift change; merged Task/Graph work supplies no credential acceptance.

## Evidence and excluded alternatives

Full findings, probe source/runner/result blobs, timestamps and source URLs:
`0288f9d611755301b85bd36316a7b310ed8d9d3a:scratch/loo-661-save-refreshed-claude.md`.
Earlier raw evidence: `31c2ed7565da481c0a2274ef448189fe15f70397:scratch/loo-661-save-refreshed-claude.md`
(blob `47bcef7ebe8d8f75a0e7f9526169c891914d0110`). Previously uncommitted
memory/assumptions were preserved as blobs `a7a634b98304be1599c7039139496e8eb8dec209`
and `5ddde1b3c97907dcfb356bd1fde5ab3e64022705`.

These conclusions retain the rejected mechanisms so investigation need not repeat
them. Native probes used disposable `loopflow-LOO-661-*-probe-<uuid>` services,
synthetic bytes, bounded/reaped children, captured output and successful cleanup.
No real credential was directly read or printed. Apple source evidence uses
Security commit `db15acbe6a7f257a859ad9a3bb86097bfe0679d9`; it does not prove
installed binary equivalence or exhaust public interfaces. Findings 8–20 were
source/header review, except read-only installed dispatch inspection in 18.

- **Transport (1–2):** `security -i`'s 4,096-byte input buffer round-tripped
  64/1,242-byte values; 2,048 bytes failed with old bytes retained. Hex fills the
  buffer at 2 KiB; raw JSON cannot fit 8 KiB. Detached `security … -w` returned
  zero but truncated larger values to 128 bytes. Source excludes PTYs, file/FD
  indirection and repeated password options: `-w`/`-X` replace rather than append;
  multiple commands perform separate replacements.
- **Access (3–5, 8–9, 11, 21, 27):** `SecKeychainItemModifyAttributesAndData`
  returned zero at 2/8 KiB but the `security` reader exited 128; `ModifyContent`
  left reads timing out. Native writes replaced `apple-tool:` with writer
  `cdhash:`. Non-null data creates a new group and strips partition ACLs; null
  leaves bytes unchanged. `security-framework` 3.7.0 uses that modifier. Combined
  updates commit data before restoring access. Apple-signed `osascript`/
  `SecItemUpdate` and copied `SecAccessRef` restoration returned zero but reads
  timed out. Applying edited partition access failed with `-25293` under disabled
  UI; initial JXA `-50` errors were bridge failures. Apple-anchored
  `com.apple.security` gets `apple-tool:`, other Apple code `apple:`.
  Public creation supplies explicit access but rejects valid duplicates;
  `SecKeychainItemSetAccess` changes the current group immediately, not the next
  update. Source's explicit-access helper branch is called by creation, not
  replacement. Neither signed interpreters nor disabled UI prove reader access.
- **Encoding and other interfaces (10, 12–20):** raw `CSSM_DL_DataModify` bypasses
  encrypted modification; JSON would replace encoded storage. No public password
  encoder was found; `CSSM_EncryptData` still needs private group/label/IV/ciphertext
  composition. Private envelope reconstruction/C++ ABI remain excluded.
  `SecKeychainSetBatchMode` is private; public FileDL transaction controls exist
  but have the limitation below. Import handles keys/certificates; copy strips
  access/integrity ACLs and creates a new group. macOS 26.0.1 (25A362) dispatch
  resolves `KCSetData`/`KCUpdateItem` to staged APIs whose published paths create
  new groups. Shared web credentials use another authority and the pinned macOS
  source returns `errSecUnimplemented`. No mutation probe through these alternatives
  established compatible replacement.
- **Outer transactions (22–26):** public FileDL controls deferred commit around
  2,048/8,192-byte modifications, which returned zero. Retained-item and fresh
  password reads returned `-25300`; metadata succeeded. Record-by-ID reads found
  new encoded bytes (2,084/8,228), but group-key queries returned end-of-data
  (`-2147413715`); fresh password queries found the original 52-byte encoded record
  and its key. Published source reads modified records by ID but queries the
  pre-write snapshot, matching the unavailable decoding group. Another lookup
  or transaction wrapper cannot supply the proposed pre-commit verification.
  Rollback/autocommit restoration returned zero and fresh `security` readers
  recovered exact old bytes. This proves those recoveries, not general rollback
  safety or unchanged ACL identity. No commit/access-restoration probe ran;
  committing first abandons verification and leaves partition access unresolved.
- **Live failure (6):** launches reported one native write/readback failure
  without status and two expired OAuth errors. Later supported 0.14.18/0.14.19
  polls accepted jack@loopflow.studio and jackstah@gmail.com with verified capacity
  and no diagnostic/recovery instruction; the uninspected local row remained
  unavailable. Latest supported `lf account claude --json` observation:
  `1793163454` (2026-10-27), exit 0, both managed accounts accepted with three
  verified windows each and no diagnostic or recovery instruction. Polls expose neither
  refresh occurrence nor storage stage, payload size or causality. They explain
  neither the earlier failure nor a browser sign-in dependency.
- **Access/API boundaries (28–29, 2026-10-27):** `kSecAttrAccess` documents access
  updates without guaranteeing atomic data/access preservation.
  `SecKeychainItemSetAccessWithPassword` is private (in `SecKeychainItemPriv.h`,
  absent from the installed public header), requires a Keychain passphrase and
  changes access without replacing data. TN3137 describes `SecItem`'s default
  file-Keychain shim; selecting data protection changes storage/access authority
  and abandons the provider item. Neither route resolves the partition failure.
  These were source/header/documentation reviews only; full citations remain in
  `e3f012685c:scratch/loo-661-save-refreshed-claude.md`, findings 28–29.

Argv credentials, broad trust changes, delete/recreate, file migration and
unprotected post-commit ACL repair violate the brief (7); provider-owned refresh
would select LOO-325. Local preservation must prove readable old bytes and unchanged
access, but cannot prove server validity after OAuth rotation: `fresh_claude_token`
receives the refresh response before saving, with no provider/Keychain transaction.
Neither limitation relaxes Jack Heart's requirements.

## Remaining implementation and acceptance

1. Prove a supported writer's encoding, access and failure preservation, then
   replace the transport and its exclusive fixture in one cut.
   Preserve JSON validation, readback, managed login locking,
   intervening-replacement checks, omitted refresh tokens, unknown fields and the
   15-second HTTP timeout. No extra OAuth request or automatic retry. Bound new
   subprocesses to 15 seconds, close stdin and reap on every exit. Report uncertain
   commits and re-read before another write; storage failure alone never requires
   sign-in or deletion.
2. Prove production create/replace at ordinary, exactly 2,048 and exactly 8,192
   serialized bytes, including Unicode/escaping and unrelated fields. Use disposable
   items created by `security` with default access; both candidate and existing
   readers must return exact bytes without intervention. Verify scoped/ambient
   selection, account and access preservation. Invalid JSON, injected pre-commit
   failure and denied/locked storage must leave old data readable; cover partition
   and partial-commit failures. Compare booleans/lengths, never values in output.
   Do not apply an unproved candidate to real credentials.
3. Compile and explicitly invoke the production native writer in tests while keeping
   ordinary unit tests on files. Require nonzero executed native cases; a test
   name/filter or file-backed success proves no native coverage. Use the shared
   environment lock and a child process for process-global Keychain UI settings,
   without a production test-only backend.
   Mocked OAuth refresh must reach that writer through `fresh_claude_token` and
   existing identity/usage/account-report consumers. Save failure must produce
   sanitized actionable `unavailable`, not `rejected`, with old data readable.
   Preserve managed file, replacement-race and native-activation coverage. Register
   a dedicated native test binary if isolation requires one. Unavailable Keychain
   infrastructure is unavailable, not green; Linux cannot replace macOS proof.
4. Diagnose a recurring installed failure through supported account operations,
   recording sanitized stage/code, storage kind and whether refresh occurred. Never
   force expiry, revoke tokens, switch routes or damage a real item to reproduce it.
   Report browser sign-in only when provider evidence requires it; no headless
   bypass. Keep `docs/subscriptions.md` recovery aligned. On the repaired installation,
   a naturally required refresh must leave `lf account claude --json` accepting the
   same identity/capacity and native Claude usable without reauthentication.
   Infrastructure owns this demo. Unexpired polls and synthetic success cannot
   establish live acceptance or complete the Task.

## Delete — do not maintain

- Replace the `security -i` spawn, hex script and command-string escaping restrictions
  inside `write_claude_keychain_credential` once a compatible writer is proven.
  Replace its exclusive stand-in in
  `claude_persistence_keychain_diagnostics_distinguish_write_readback_and_mismatch`
  with the surviving writer's diagnostic fixture, retaining stage/secrecy checks.
  Keep one writer behind `write_claude_login`, shared by refresh and activation;
  no small-value fallback, size dispatch, dual writes or bypass.
- Retain shared readers, service/account parsers and tests, managed file writes,
  explicit profile-capture cleanup and race/rotation tests; they have other consumers.

## Verification and delivery

Gate owns affected suites, formatting and `cargo clippy --all-targets -- -D warnings`.
PR #1749 skips macOS jobs for some Rust-only changes, but its classifier selects
`macos-process-test` for this branch because `provider_auth/mod.rs` contains macOS
code. That selection does not supply the missing native acceptance fixture.
With inherited `LF_*` unset and stdin `/dev/null`, run:

```sh
uv run python scripts/test.py --reuse-passing --rust-filter 'test(provider_auth::) | test(subscription::) | test(provider_account::) | test(lf::commands::account_status::) | test(lf::commands::account::)'
```

No native acceptance fixture exists yet. Record its actual command once implemented
and require nonzero executed cases meeting steps 2–3; a speculative filter can pass
with zero tests.
Publish/ship only the complete bounded repair through the selected Graph, without
another release publisher. Do not repeat checks solely because a lifecycle phase
changes.

The authorized LOO-664 sync bridge and archive receipt live in `scratch/questions.md`.

Check — compress (2026-10-27): `git diff --check` and `lf context` pass; prose-only reduction, no Rust rerun. Gate owns affected suites; native-writer and installed-refresh acceptance remain unproved.
