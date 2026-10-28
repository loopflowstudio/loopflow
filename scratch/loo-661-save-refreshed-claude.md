# LOO-661: Save refreshed Claude credentials reliably

Status (2026-10-28): **Implementation incomplete; supported native writer unresolved.**
The defective `security -i` writer remains, with no native acceptance fixture.
The historical save failure is undiagnosed; live acceptance separately needs browser
reconnection for jack@loopflow.studio. No public-API exhaustion or scope change is assumed.

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

**Blocked at mechanism selection.** Before another mutation probe, identify a
distinct supported writer addressing findings 35–37: creation with the necessary
partition authority, or replacement retaining the existing encoding group/access.
A password alone is unproved; do not collect one speculatively or repeat rejected
probes. Private interfaces, trust changes and provider-owned refresh require a
scope decision Jack Heart has not made. Diagnostics alone cannot ship this repair.

The October 28 kickoff rechecked `SecItemUpdate` with `kSecAttrAccess` and the
password requirement for partition edits; neither established a new candidate.
Its documentation citations remain at
`157cafb2a7b912a1573f839c2c2359e53088ec9f:scratch/loo-661-save-refreshed-claude.md`,
“Required outcome and next decision.” This is not public-API exhaustion.

Browser reconnection unblocks live diagnosis only: `invalid_grant` returns before
`write_claude_login`. Local rollback cannot establish server token validity after
OAuth rotation because refresh precedes saving without a shared transaction.
Neither dependency relaxes the preservation requirement or proves a native writer.

Planning disposition (2026-10-28): retain this design and its accepted outcome.
Inspection at `56541b779b94500348267372d85aee3cd97c5549` confirms the writer still
uses `security -i`, ordinary login tests still select files, and `invalid_grant`
returns before saving. No new mechanism or changed authorization was supplied.
The next implementation slice therefore remains blocked on compatible native
persistence; browser reconnection addresses only the independent live-diagnosis
dependency. Repeating rejected probes or publishing diagnostics cannot satisfy
either missing proof. No production edit or credential operation follows from
this planning pass.

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

The refresh and redaction paths need no additional abstraction. Keep their behavior while
replacing the transport; do not polish code already selected for deletion.

## Evidence and excluded alternatives

Complete pre-compression plan, citations, dependency history and archive chain:
`055c8a3005aed51af7551794fc3f34d7a7827cca:scratch/loo-661-save-refreshed-claude.md`.
Findings 1–33 and the retired command-boundary probe:
`c871ddfb0c4956fdb816c079c61d71e68767733f:scratch/`.
Findings retain their original numbers below for lookup. The evidence constrains
replacement; it is not production acceptance.

Native probes used disposable `loopflow-LOO-661-*-probe-<uuid>` services, synthetic
bytes, bounded/reaped children, captured output and successful cleanup. They neither
read real credentials directly nor changed routes, Sessions or global trust. Source
conclusions use Apple Security `db15acbe6a7f257a859ad9a3bb86097bfe0679d9`; they do not
establish installed binary equivalence or exhaust public interfaces.

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
  reads exited 128; `ModifyContent`, Apple-signed `osascript`/`SecItemUpdate` and
  copied-access restoration left reads timing out. Writes replaced `apple-tool:`
  with writer `cdhash:`. Published non-null updates create a group and strip
  partition/integrity ACLs; null data leaves bytes unchanged. `security-framework`
  3.7.0 uses that modifier. Partition edits failed `-25293` with UI disabled;
  initial JXA `-50` was a bridge failure. Signing/trusting an application alone
  supplies no partition authority. Combined data/access updates modify data first
  without encompassing rollback; pre-setting access affects only the old group.
  Public creation rejects duplicates. Private password-based repair, TN3137's file
  shim and another data-protection authority supply no accepted atomic replacement.
- **Other interfaces (10, 12–20):** raw `CSSM_DL_DataModify` bypasses encryption;
  `CSSM_EncryptData` needs private envelope composition; `SecKeychainSetBatchMode`
  is private. Import handles keys/certificates; copy strips access/integrity ACLs
  when creating a group. Read-only macOS 26.0.1 dispatch inspection found
  `KCSetData`/`KCUpdateItem` use staged APIs with published new-group paths. Shared
  web credentials select another authority and pinned source returns
  `errSecUnimplemented`. Other findings were source/header review.
- **Outer transactions (22–26):** FileDL deferred commit around successful
  2,048/8,192-byte modifications. Password reads failed `-25300` while metadata
  succeeded; by-ID reads saw new encoded data, queries saw the old snapshot and
  could not find its decoding group. This prevented pre-commit verification.
  Rollback/autocommit restoration returned zero and independent readers recovered
  old bytes, proving neither unchanged ACL identity nor general rollback safety.
  No commit/access-restoration probe ran.
- **Copied-access creation (34):** `SecKeychainItemCreateFromContent` with copied
  access failed `-25293` at both sizes with UI disabled. Old bytes remained exact;
  candidates were absent (44). Promotion/transactions were not tested. This rejects
  that construction, not every explicit-access creation. Receipts:
  `78530325c1:scratch/loo-661-save-refreshed-claude.md`, finding 34.
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
  an invalid-form tombstone, not a contradiction of fresh construction. Full citations:
  `1266f97dfeb8bf8b08731868dd57f2917145524b:scratch/loo-661-save-refreshed-claude.md`.
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
  reconstruction crosses the private-interface boundary. Citations: same
  `1266f97dfeb8bf8b08731868dd57f2917145524b` plan, finding 36.
- **Local authentication context (37):** read-only documentation/source review
  found LAContext externalization in the data-protection implementation. The
  separate file-keychain update modifies content before access, without handling
  `kSecUseAuthenticationContext` or `kSecUseCredentialReference`. This identifies
  no context-to-database-credential bridge or atomic preservation path; it is not
  an installed securityd trace or public-API impossibility proof. Findings 36–37
  performed no authentication, unlock, mutation or real credential read. Full
  citations: pre-compression plan above, finding 37.
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

## Remaining implementation and acceptance

1. Prove a distinct supported mechanism addressing findings 35–37 on disposable
   synthetic items before replacing the production transport and exclusive fixture.
   Preserve JSON validation, readback, managed login locking, replacement checks,
   omitted refresh tokens, unknown fields and the 15-second HTTP timeout. No extra
   OAuth request or automatic retry. Bound new subprocesses to 15 seconds, close
   stdin and reap on every exit. Report uncertain commits and re-read before another
   write; storage failure alone never requires sign-in or deletion.
2. Prove production create/replace at ordinary, exactly 2,048 and exactly 8,192
   serialized bytes, including Unicode/escaping and unrelated fields. Use disposable
   items created by `security` with default access; candidate and existing readers
   must return exact bytes without intervention. Verify scoped/ambient selection,
   account/access preservation and varied service/account prefix lengths. Invalid
   JSON, injected pre-commit failure and denied/locked storage must leave old data
   readable; cover partition and partial-commit failures. Compare booleans/lengths,
   never values in output. Do not apply an unproved candidate to real credentials.
3. Compile and explicitly invoke the production native writer while ordinary unit
   tests remain file-backed. Require nonzero native cases; names/filters alone prove
   nothing. Use the shared environment lock and a child process for process-global
   Keychain UI settings, without a production test-only backend. Mocked OAuth refresh
   must reach the writer through `fresh_claude_token` and identity/usage/account-report
   consumers. Failed saves must report sanitized actionable `unavailable`, not
   `rejected`, with old data readable. Preserve managed-file, replacement-race and
   native-activation coverage. Register a dedicated native test binary if needed.
   Unavailable Keychain infrastructure is unavailable, not green; Linux cannot
   replace macOS proof.
4. Diagnose recurrence through supported installed account operations, recording
   sanitized stage/code, storage kind and whether refresh occurred. Never force
   expiry, revoke tokens, switch routes or damage a real item. Keep recovery docs
   aligned; report browser dependency without bypassing it. Infrastructure's demo
   must show a naturally required refresh on the repaired installation leaving
   `lf account claude --json` accepting the same identity/capacity and native Claude
   usable without reauthentication. Unexpired polls and synthetic success cannot
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

## Verification and delivery

Gate owns affected suites, formatting and `cargo clippy --all-targets -- -D warnings`.
The macOS CI classifier selects `macos-process-test` for `provider_auth/mod.rs`,
which does not supply the missing native fixture. With inherited `LF_*` unset
and stdin `/dev/null`, run:

```sh
uv run python scripts/test.py --reuse-passing --rust-filter 'test(provider_auth::) | test(subscription::) | test(provider_account::) | test(lf::commands::account_status::) | test(lf::commands::account::)'
```

Record the native acceptance command when implemented; require nonzero cases
meeting steps 2–3. Publish/ship only the complete repair through the selected Graph,
without another release publisher or repeated checks merely for a lifecycle change.

Check — `git diff --check` and `lf context`: pass (2026-10-28 kickoff); plan-only clarification, no code change or test rerun. Both context budgets hold; native writer, preservation proof and live diagnosis remain open; Infrastructure owns installed acceptance.
