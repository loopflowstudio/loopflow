# LOO-661: Save refreshed Claude credentials reliably

Status: **Implementation blocked on a verified native writer; diagnostics only.**
Jack Heart authorized bounded repair and autonomous delivery of
[LOO-661](https://linear.app/loopflow/issue/LOO-661/save-refreshed-claude-credentials-reliably-on-macos).
Investigation baseline: `0105aa8688764d08d7cdb4a685bbdaccb3aa91a8`;
reviewed checkout: `e4aedd2b70` (2026-10-27, includes LOO-657 and v0.14.19).
No remote freshness is claimed. Installed 0.14.19 accepted both managed accounts
in a later poll, without proving refresh or explaining the original save failure.

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
writer still has its input ceiling; diagnostics do not repair it. Findings 22–23
prove old-reader recovery after specific rollbacks, but replacement reads failed.
The next implementation needs a supported mechanism that demonstrates exact large
replacement, existing-reader access and old-data recovery on failure together.
No product decision or additional authorization is pending, and the exclusions
below do not exhaust every public interface.

## Demo and decision boundary

After a naturally required refresh, `lf account claude --json` still accepts
Jack Heart's same account and reports capacity; native Claude continues using that
login without sign-in. A disposable production-path fixture independently proves
exact 2,048- and 8,192-byte replacement and readable old data on a failed save.
Both are required: successful account polls do not prove a save occurred.

Retain one native writer behind `write_claude_login`, shared by refresh and
activation, with current readers and credential authority preserved. Its transport
remains unselected; a transaction wrapper alone does not establish compatibility.

## Surviving implementation

- `provider_auth/mod.rs::read_claude_login` and `write_claude_login` select native
  Keychain storage for native macOS homes and private `.credentials.json` files
  for managed homes. Ordinary unit tests use files even on macOS.
- The scoped service is `Claude Code-credentials-<first eight SHA256 hex digits
  of config-directory path>`; the ambient default home falls back to unsuffixed
  `Claude Code-credentials` when no scoped item exists. Existing account metadata
  wins; `USER` supplies new items. Readers use `security find-generic-password`.
- `subscription.rs::fresh_claude_token` retains the original `SecretString`,
  preserves unrelated JSON and serializes the refreshed value once. Endpoint
  selection precedes request construction. The final replacement check and save
  sit beside the returned credential; a single-use wrapper added no policy.
  Keep `ensure_claude_credential_unchanged` and the `claude_persistence_error`
  redaction boundary.
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

The complete plan before this compression is Git blob
`ef24036b138f056bd810b03ad31ee0f7dd447ff2` (`git show <blob>`).
The preceding consolidation is Git blob
`f663e4041d2d971bc7e1d2f36950b3df71f193ff` (`git show <blob>`).
The earlier pre-reduction design is Git blob
`301929d2de5a4f0cf9c2b0b752ba7d62ea5c8df4` (`git show <blob>`).
Earlier full evidence, source URLs/line ranges, binary UUIDs/offsets and archives:
`7cb7a5a5cb34755998f3886cc88044c9544397ce`;
prior planning notes: `35b317fbb392f030531d9178a550e9eb85251842`.
Apple source review used Security commit
`db15acbe6a7f257a859ad9a3bb86097bfe0679d9` unless an archived link says `main`.
Source inspection does not prove installed downstream implementation equivalence.

All native probes used disposable `loopflow-LOO-661-*-probe-<uuid>` services and
synthetic data. Cleanup in `finally` returned zero; reads were bounded and reaped.
No real credential was directly read or printed. Retain these counterexamples:

- **Input ceilings (findings 1–2):** `security -i` has a 4,096-byte buffer.
  64/1,242-byte values round-tripped; 2,048 bytes failed, retaining the old value.
  Hex alone fills the buffer at 2 KiB; raw JSON cannot fit 8 KiB. Installed help
  and the pinned parser expose no file/FD indirection. Detached `security … -w`
  returned zero at 128/2,048/8,192 bytes but truncated large inputs to 128 bytes
  (`_PASSWORD_LEN + 1`). Success status alone proves nothing.
- **Access loss (3–5, 8–9, 11):** `SecKeychainItemModifyAttributesAndData`
  returned zero for 2/8 KiB but `security -w` exited 128; `ModifyContent` left
  readback timing out at 10 seconds. Native writes changed `apple-tool:` to the
  writer's `cdhash:`. Non-null data selects a new group and strips partition ACLs;
  null data leaves the password unchanged. `security-framework` 3.7.0 uses the
  same modifier. Combined updates commit data before separate access restoration.
  Apple-signed `osascript`/`SecItemUpdate` and copied `SecAccessRef` restoration
  returned zero but left reads timing out; explicit partition editing succeeded
  locally, then applying access failed with `-25293` under disabled UI. Initial
  JXA `-50` errors were bridge failures. No trust/password bypass was attempted.
  Apple-anchored `com.apple.security` gets `apple-tool:`; other Apple code gets
  `apple:`. A helper/interpreter or disabling writer UI does not prove reader access.
- **Missing public encoder (10, 12–15):** raw `CSSM_DL_DataModify` reaches
  `SSUniqueRecordImpl`, not the encrypted `SSDbUniqueRecordImpl::modify` path.
  Raw JSON would replace encoded storage. Public passthroughs expose no password
  encoder; private `DB_MODIFY_WITHOUT_ENCRYPTION` expects encoded blobs.
  `CSSM_EncryptData` still needs private group/label/IV/ciphertext composition.
  `SecKeychainSetBatchMode` is private. Public FileDL `TOGGLE_AUTOCOMMIT`,
  `COMMIT`, `ROLLBACK` and `DB_GET_HANDLE` do exist, qualifying the batch finding,
  but settle supplied bytes without proving high-level Keychain rollback.
  Finding 22 adds a high-level mutation/rollback probe and narrows this uncertainty.
  Private envelope reconstruction/internal C++ ABI remain excluded.
- **Other interfaces (16–20):** `SecItemImport` handles key/certificate containers;
  copy strips access/integrity ACLs and inserts a new group; creation APIs reject
  valid duplicates. None supplies atomic replacement. Read-only inspection on
  macOS 26.0.1 (25A362) resolved arm64e `KCSetData`/`KCUpdateItem` to Security's
  staged `SecKeychainItemSetData`/`SecKeychainItemUpdate`; published targets use
  the rejected new-group path. Dispatch is proved, full binary/source equivalence
  is not. `SecAddSharedWebCredential` targets a different website/account authority;
  pinned macOS source returns `errSecUnimplemented`. No import, copy, creation,
  shared-web or staged item mutation probe ran.
- **Live evidence (6):** a supported installed 0.14.18 account poll accepted
  jack@loopflow.studio and jackstah@gmail.com at usage observations
  1793140722/1793140723, without proving refresh; the local row is not server
  verification. Process `a9687148-c55c-4385-a2c0-332a49f796cf` reported native
  write/readback failure without status. `0afde787-992e-4b29-943c-4fa50d5916ca`
  and `34143676-fa11-4fa3-a072-d14ad07fe06b` reported expired OAuth. Payload size
  and causality remain unknown; no browser sign-in dependency is established.
  The implementation continuation observed installed lf 0.14.19. One supported
  `lf account claude --json` invocation accepted both managed identities at
  1793152261/1793152262, with session/weekly/weekly:fable capacity verified and
  no diagnostic. Its local uninspected row remained unavailable. This report
  does not expose whether refresh occurred; it neither reproduces nor explains
  the original save failure. Output was captured and reduced to identity,
  verification, diagnostic-presence booleans and observation timestamps.
- **Current API contract (21, 2026-10-27):** Apple's
  [update guide](https://developer.apple.com/documentation/security/updating-and-deleting-keychain-items)
  supports replacement with `SecItemUpdate`; its
  [`kSecAttrAccess` reference](https://developer.apple.com/documentation/security/ksecattraccess)
  explicitly permits supplying access in the same update dictionary for the
  traditional macOS Keychain. Neither documents atomic preservation of existing
  partition access when data and access change together. This confirms the API
  is supported, but does not overturn the saved partial-commit/access-loss
  counterexamples. The pinned `keychain_add.c:131–155` likewise modifies data
  before setting access. No distinct replacement candidate or new mutation probe
  resulted from this review.
- **Outer transaction (22, 2026-10-27):** pinned Apple
  [Item.cpp:872–905](https://github.com/apple-oss-distributions/Security/blob/db15acbe6a7f257a859ad9a3bb86097bfe0679d9/OSX/libsecurity_keychain/lib/Item.cpp#L872-L905)
  wraps replacement in `CSPDLTransaction`. Its
  [commit implementation](https://github.com/apple-oss-distributions/Security/blob/db15acbe6a7f257a859ad9a3bb86097bfe0679d9/OSX/libsecurity_utilities/lib/CSPDLTransaction.cpp#L39-L73)
  defers commit/rollback when autocommit was already disabled. This supports
  testing public FileDL controls around the high-level modifier, without a
  private encoder or C++ ABI. In a separate disposable Keychain, `security -i`
  created a synthetic item with default access. With UI disabled in the native
  helper, `DB_GET_HANDLE`, disabling autocommit and
  `SecKeychainItemModifyAttributesAndData` returned zero at 2,048 and 8,192 bytes.
  `SecKeychainItemCopyContent` then returned `-25300` (item not found), so new
  bytes were **not verified**. Explicit rollback and autocommit restoration
  returned zero; a fresh `security` process read the exact old bytes after each
  attempt. This establishes old-reader recovery for these attempts, not verified
  replacement, general rollback safety or preserved ACL identity. No commit or
  access-restoration probe ran. The helper exited 1 for both failed reads;
  disposable Keychain deletion returned zero. Commands had 10/15-second bounds,
  captured all credential-bearing output and printed only statuses/comparisons.
  Probe source: Git blob `f8fd3265b3c6a5375cb0cdd0ca86cb5d08cd0c40`.

- **Fresh lookup within transaction (23, 2026-10-27):** the preceding probe
  read only its retained item reference. A follow-up tested whether a fresh lookup
  could verify the new value instead. In a disposable Keychain seeded by
  `security -i` with default access, both 2,048- and 8,192-byte modifications
  returned zero. `SecKeychainItemCopyContent` again returned `-25300`;
  a new `SecKeychainFindGenericPassword` lookup by the same service/account also
  returned `-25300`, while metadata-only reading of the retained item returned
  zero. Thus reacquiring the item through this public lookup does not repair
  the failed verification. The result does not identify the underlying cause or
  establish how committed data would behave. Rollback and autocommit restoration
  returned zero; the existing `security` reader recovered exact old bytes after
  each attempt. No commit or access change was attempted. Cleanup returned zero.
  All subprocesses had 15-second bounds; outputs contained only status, lengths
  and equality results. Probe source: Git blob `0c32548b0168c0e38e08948ca991b2f6905c1334`;
  pre-update plan: `4ba8a12723381e61dec42111cd97561020e9552c`. This is synthetic rollback evidence,
  not a compatible writer or live diagnosis.

Findings 8–20 are source/header review except the read-only installed dispatch
inspection in 18. These exclusions establish neither production acceptance nor
exhaustion of every public interface. Argv credentials, broad trust changes,
delete/recreate, file migration and unprotected post-commit ACL repair violate
the brief (7); provider-owned refresh would select LOO-325.

The local preservation test proves readable old bytes and unchanged access.
It cannot prove server validity after OAuth rotation: `fresh_claude_token`
receives the refresh response before saving, with no transaction joining the
provider and Keychain. Live acceptance must therefore verify actual login use;
neither local rollback nor a successful storage fixture establishes that result.
This distinction does not relax Jack Heart's preservation requirement or select
the excluded concurrency redesign.

## Remaining implementation and acceptance

1. Establish a supported writer's encoding, access and commit behavior before
   another disposable probe. Transport replacement, consumer cutover and deletion
   are one cut. Preserve JSON validation, readback, managed login locking,
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

- Removed `subscription.rs::persist_claude_refresh`; its sole caller performs the
  same check/save directly. Shared race checks and redaction remain.
- Replace the `security -i` spawn, hex script and command-string escaping restrictions
  inside `write_claude_keychain_credential` once a compatible writer is proven.
  Replace its exclusive stand-in in
  `claude_persistence_keychain_diagnostics_distinguish_write_readback_and_mismatch`
  with the surviving writer's diagnostic fixture, retaining stage/secrecy checks.
  No second writer, small-value fallback, size dispatch, dual writes or bypass.
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

Check — 2026-10-27 compression: `git diff --check`, `git diff --cached --check` and `lf context` passed; prose-only reduction, no tests rerun (11 subscription tests previously passed). Native replacement remains unproved; gate owns affected suites/clippy and Infrastructure owns installed refresh acceptance.
