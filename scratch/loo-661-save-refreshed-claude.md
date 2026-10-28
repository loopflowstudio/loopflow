# LOO-661: Save refreshed Claude credentials reliably

Status: **Implementation blocked on a verified native writer; diagnostics only.**
Jack Heart authorized bounded repair and autonomous delivery of
[LOO-661](https://linear.app/loopflow/issue/LOO-661/save-refreshed-claude-credentials-reliably-on-macos).
Investigation baseline: `0105aa8688764d08d7cdb4a685bbdaccb3aa91a8`;
reviewed checkout: `6ab664a7aa` (includes LOO-657 and v0.14.19).
No remote freshness is claimed.

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

No tested writer meets the preservation contract. The current `security -i`
writer still has its input ceiling; diagnostics do not repair it. Prove a supported
mechanism before production cutover. Findings 22–27 below reject the proposed
outer-transaction verification and explicit-access alternatives without exhausting
public interfaces. No product decision or additional authorization is pending.

## Demo and decision (2026-10-27)

Jack Heart's preservation requirements are accepted; the replacement mechanism
remains unselected. The demo is a naturally required Claude refresh followed by
`lf account claude --json` accepting the same identity and capacity, with native
Claude still usable without reauthentication. Infrastructure owns installed proof.
Before that demo, a disposable native item must survive production replacement
with exactly 2,048 and 8,192 serialized bytes and remain readable by `security`.

**This slice: compatible-writer proof, blocked.** The missing evidence is one
supported replacement operation that preserves existing reader access and leaves
the old credential readable on failure. Neither a successful write status nor
post-write repair establishes that contract. The remaining implementation below
is conditional on this proof; the diagnostic changes alone are not the repair.
The existing design and rejected alternatives remain binding evidence.

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

The writer still validates JSON, resolves service/account, sends hex through
`security -i` stdin and verifies readback. Four diagnostic tests cover hostile
output, mismatch, numeric IO errors and unstructured errors; they prove redaction,
not native preservation. No replacement writer, native acceptance fixture,
database migration, DTO change or Swift change exists.

## Evidence and excluded alternatives

Full evidence before this consolidation is preserved at
`31c2ed7565da481c0a2274ef448189fe15f70397:scratch/loo-661-save-refreshed-claude.md`
(blob `47bcef7ebe8d8f75a0e7f9526169c891914d0110`), including earlier archives,
source URLs, process IDs and probe blobs. The conclusions below group the findings
by mechanism so the next investigation need not replay them.
The pre-compression plan is at
`e0fbae22bb4b4e3bee33a62a51dcf12f30a7b2a6:scratch/loo-661-save-refreshed-claude.md`.
The earlier uncommitted memory/assumptions are preserved as Git blobs
`a7a634b98304be1599c7039139496e8eb8dec209` /
`5ddde1b3c97907dcfb356bd1fde5ab3e64022705` before reconciliation/checkpointing.
Apple source review used Security commit
`db15acbe6a7f257a859ad9a3bb86097bfe0679d9` unless an archived link says `main`;
source inspection does not prove
installed binary equivalence or exhaust every public interface.

Native probes used disposable `loopflow-LOO-661-*-probe-<uuid>` services and
synthetic data, bounded/reaped subprocesses, captured outputs and successful cleanup.
No real credential was directly read or printed. Findings 8–20 were source/header
review except read-only installed dispatch inspection in 18.

- **Input ceilings (1–2):** `security -i` has a 4,096-byte buffer. Values of
  64/1,242 bytes round-tripped; 2,048 failed with old bytes retained. Hex alone
  fills the buffer at 2 KiB; raw JSON cannot fit 8 KiB. Help/parser expose no
  file/FD indirection. Detached `security … -w` returned zero at 128/2,048/8,192
  bytes but truncated large inputs to 128 bytes (`_PASSWORD_LEN + 1`).
  Source recheck on October 27 also excludes a PTY or repeated password options
  as a larger transport: `security.c::main` uses the same fixed input buffer
  with or without a PTY; `isatty` only controls the prompt. Each command receives one password
  buffer, and `-w`/`-X` replace it rather than append. Multiple commands perform
  separate replacements. This is source evidence, not another native probe.
  Sources: [security.c](https://github.com/apple-oss-distributions/Security/blob/db15acbe6a7f257a859ad9a3bb86097bfe0679d9/SecurityTool/macOS/security.c),
  [keychain_add.c](https://github.com/apple-oss-distributions/Security/blob/db15acbe6a7f257a859ad9a3bb86097bfe0679d9/SecurityTool/macOS/keychain_add.c).
- **Access loss (3–5, 8–9, 11, 21):** `SecKeychainItemModifyAttributesAndData`
  returned zero at 2/8 KiB but the `security` reader exited 128; `ModifyContent`
  left reads timing out. Native writes replaced `apple-tool:` with the writer's
  `cdhash:`. Non-null data creates a new group and strips partition ACLs; null
  leaves bytes unchanged. `security-framework` 3.7.0 uses the same modifier.
  Combined updates commit data before restoring access. Apple-signed
  `osascript`/`SecItemUpdate` and copied `SecAccessRef` restoration returned zero
  but left reads timing out. Explicit partition editing succeeded locally, then
  applying access failed with `-25293` under disabled UI. Initial JXA `-50`
  errors were bridge failures. Apple-anchored `com.apple.security` gets
  `apple-tool:`; other Apple code gets `apple:`. Neither a signed interpreter nor
  disabled writer UI proves reader access. Apple's update guide supports
  `SecItemUpdate` with `kSecAttrAccess`, but promises no atomic preservation of
  existing partitions; pinned `keychain_add.c:131–155` modifies data first.
- **No public encoder found (10, 12–15):** raw `CSSM_DL_DataModify` reaches
  `SSUniqueRecordImpl`, bypassing encrypted `SSDbUniqueRecordImpl::modify`;
  raw JSON would replace encoded storage. Public passthroughs expose no password
  encoder; private `DB_MODIFY_WITHOUT_ENCRYPTION` expects encoded blobs.
  `CSSM_EncryptData` still needs private group/label/IV/ciphertext composition.
  `SecKeychainSetBatchMode` is private. Public FileDL `TOGGLE_AUTOCOMMIT`,
  `COMMIT`, `ROLLBACK` and `DB_GET_HANDLE` do exist; their high-level limitation
  is established below. Private envelope reconstruction/internal C++ ABI remain out.
- **Other interfaces (16–20):** import handles key/certificate containers; copy
  strips access/integrity ACLs and inserts a new group; creation rejects valid
  duplicates. macOS 26.0.1 (25A362) read-only dispatch inspection resolves arm64e
  `KCSetData`/`KCUpdateItem` to staged `SecKeychainItemSetData`/`SecKeychainItemUpdate`,
  whose published paths create new groups. This proves dispatch, not full source
  equivalence. `SecAddSharedWebCredential` uses a different website/account
  authority and pinned macOS source returns `errSecUnimplemented`. None establishes
  compatible replacement; no mutation probe through these interfaces ran.
- **Outer transactions cannot verify replacement before commit (22–26):**
  published `Item.cpp:809–905` creates a new group/removes partition ACLs and
  wraps replacement in `CSPDLTransaction`, which defers settlement when autocommit
  is already disabled (`CSPDLTransaction.cpp:39–73`). Disposable Keychains seeded
  by `security -i` with default access tested public FileDL controls around that
  modifier, with native UI disabled. Modifications returned zero at 2,048/8,192
  bytes, but retained-item reads and fresh `SecKeychainFindGenericPassword`
  lookups returned `-25300`; metadata reads succeeded.

  A subsequent public CSSM probe read new encoded records by unique ID (2,084/8,228
  bytes), while group-key queries returned `CSSMERR_DL_ENDOFDATA` (`-2147413715`).
  Fresh CSPDL/FileDL password queries returned the original 52-byte encoded record
  and its key. Thus the replacement record was not simply absent; its new group
  was unavailable to decoding. Published `AppleDatabase.cpp:1634–1670` routes
  `DbModifier::getRecord` to modified tables but `createCursor` to the pre-write
  snapshot; `securestorage.cpp:253–277` uses that cursor for group-key lookup.
  This explains the published implementation's split and matches the probe,
  without establishing installed binary equivalence or post-commit behavior.

  Rollback and autocommit restoration returned zero; fresh `security` readers
  recovered exact old bytes after each attempt. This proves those recoveries,
  not general rollback safety, unchanged ACL identity or verified replacement.
  No commit or access-restoration probe ran. Reacquiring an item or opening another
  reader cannot supply the proposed pre-commit verification; committing first
  abandons that boundary and leaves partition-access failure unresolved. Reject
  this sequence rather than repeat another lookup/transaction wrapper.

  Probe source blobs: `f8fd3265b3c6a5375cb0cdd0ca86cb5d08cd0c40` (22),
  `0c32548b0168c0e38e08948ca991b2f6905c1334` (23),
  `1755401719a564fefa372d652310292725e084c8` (25). Finding 25's runner/results:
  `4db66134850366576a038ce689b1527d6060ddff` /
  `d6c97155d988c0d1453f82bc4c108f24f11e8d70`; reconstruct its temporary directory
  locator beside the compiled probe. All transaction subprocesses were bounded
  to 10/15 seconds; outputs contained only status, lengths and comparisons.
- **Explicit access is not an update escape hatch (27):** at the same pinned
  Apple revision, `ItemImpl::addWithCopyInfo` passes `mAccess` into
  `updateSSGroup`; `ItemImpl::update` calls it without that argument. The latter
  copies the old group's access and removes partition/integrity ACLs.
  `SecKeychainItemCreateFromContent` supplies initial access to a new item;
  `SecKeychainItemSetAccess` immediately changes the current group's access,
  rather than staging access for the next data update. Thus an explicit-access
  branch inside the private helper is not a public replacement mechanism.
  Creation still does not replace a valid duplicate (finding 16), and setting
  access before modification does not avoid the modifier's removal. This is
  source evidence, not a new native mutation result or proof of installed binary
  equivalence. Sources: [Item.cpp](https://github.com/apple-oss-distributions/Security/blob/db15acbe6a7f257a859ad9a3bb86097bfe0679d9/OSX/libsecurity_keychain/lib/Item.cpp),
  [SecKeychainItem.cpp](https://github.com/apple-oss-distributions/Security/blob/db15acbe6a7f257a859ad9a3bb86097bfe0679d9/OSX/libsecurity_keychain/lib/SecKeychainItem.cpp).
- **Live failure remains undiagnosed (6):** a recorded launch reported native
  write/readback failure without status; two others reported expired OAuth.
  Supported installed 0.14.18 polls accepted jack@loopflow.studio and
  jackstah@gmail.com at `1793140722/1793140723`; 0.14.19 accepted both at
  `1793152261/1793152262` and `1793156935`, with verified capacity and no diagnostic
  or recovery instruction. The uninspected local row remained unavailable.
  Captured output was reduced to identity, verification, diagnostic presence and
  observation timestamps. These reports expose neither refresh occurrence nor
  storage stage. Payload size/causality remain unknown; no browser sign-in
  dependency is established. Successful polls cannot explain the earlier failure.

Argv credentials, broad trust changes, delete/recreate, file migration and
unprotected post-commit ACL repair violate the brief (7); provider-owned refresh
would select LOO-325. Local preservation must prove readable old bytes and unchanged
access, but cannot prove server validity after OAuth rotation: `fresh_claude_token`
receives the refresh response before saving, with no provider/Keychain transaction.
Live acceptance must verify actual login use. Neither limitation relaxes Jack
Heart's preservation requirement or selects the excluded concurrency redesign.

## Remaining implementation and acceptance

1. Prove a supported mechanism that establishes encoding, access and failure
   preservation, then replace the transport and its exclusive fixture in one cut.
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
   ordinary unit tests isolated. Use the shared environment lock and a child process
   for process-global Keychain UI settings, without a production test-only backend.
   Mocked OAuth refresh must reach that writer through `fresh_claude_token` and
   existing identity/usage/account-report consumers. Save failure must produce
   sanitized actionable `unavailable`, not `rejected`, with old data readable.
   Preserve managed file, replacement-race and native-activation coverage.
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
With inherited `LF_*` unset and stdin `/dev/null`, run:

```sh
uv run python scripts/test.py --reuse-passing --rust-filter 'test(provider_auth::) | test(subscription::) | test(provider_account::) | test(lf::commands::account_status::) | test(lf::commands::account::)'
```

The native fixture still needs implementation:
`cargo test -p loopflow --lib claude_keychain_native -- --test-threads=1`.
This is a planned command, not an existing passing check. Acceptance requires
nonzero executed native cases: ordinary `cfg(test)` login dispatch uses files,
so naming/filtering a test does not establish native storage coverage. The fixture
must demonstrate the refresh-to-native-writer-to-account-report path described
above, as well as independently reading the item through `security`.
Register/filter a dedicated native binary if isolation requires one. File-backed
or Linux tests cannot replace macOS proof; unavailable Keychain infrastructure
is unavailable, not green. Publish/ship only the complete bounded repair through
the selected Graph, without another release publisher. Do not repeat checks solely
because a lifecycle phase changes.

Delivery also depends on LOO-664: installed 0.14.19 sync cannot stage edits because
of its ignored `.lf/tmp` exclusion. Until the repair is installed, checkpoint
intended step edits with `lf commit <paths>` before sync, preserving unrelated
staged/unstaged work and normal validation. Do not force-add telemetry or implement
a competing repair; retain the dependency if checkpointing fails.

Check — kickoff (2026-10-27): `git diff --check` passed; `lf context` reports memory/scratch within both limits. Prose-only clarification; no native mutation or live-account poll repeated. Prior diagnostic checks remain applicable; gate owns affected suites, and native replacement plus installed refresh acceptance remain unproved.
