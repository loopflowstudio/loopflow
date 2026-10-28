# LOO-661: Save refreshed Claude credentials reliably

Status (2026-10-28): **Native writer unresolved; implementation incomplete.** The production
`security -i` transport remains defective; no native acceptance fixture exists.
Findings 35–37 reject the tested access constructions and authentication/group
retention alternatives. A distinct supported mechanism is required before another
mutation probe; public interfaces are not declared exhausted and no scope change
is accepted. Live acceptance separately needs browser reconnection for
jack@loopflow.studio. The historical save failure remains undiagnosed.

Jack Heart accepted bounded repair and autonomous delivery on 2026-10-27 for
[LOO-661](https://linear.app/loopflow/issue/LOO-661/save-refreshed-claude-credentials-reliably-on-macos).
Investigation baseline: `0105aa8688764d08d7cdb4a685bbdaccb3aa91a8`;
prior plan and dependency reconciliation:
`61c6f82a7fd9e9c61d3a9e81dfcfa261708536dc:scratch/loo-661-save-refreshed-claude.md`.

Integrated dependencies (none proves Claude repair or installation):

- PR #1749 selects macOS CI; LOO-657 supplies takeover notices.
- LOO-664 / #1754 (`fefda39493`) repairs source sync. The temporary telemetry
  bridge and archive receipts remain in `scratch/questions.md` until installed recovery.
- LOO-665 / #1756 (`9967df791b`) retains displaced file inodes for Codex recovery;
  that mechanism does not preserve Keychain partition access.
- PR #1759 (`635d99fee0`): v0.15.0 consolidates 0.14.11–0.14.20 and retains the
  0.14.20 store compatibility floor, superseding #1758's release metadata.

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

The target is one native writer shared by refresh and activation. Diagnostics,
a size refusal, successful API status or post-write repair do not prove preservation.

## Demo and planning decision

Demo: after a naturally required refresh on the repaired installation,
`lf account claude --json` accepts the same identity and reports verified capacity;
an existing native Claude Session remains usable. Gate first demonstrates exact
ordinary, 2,048-byte and 8,192-byte create/replace round trips through the production
writer and the independent native reader, with old bytes and access retained after
a failed save. Infrastructure owns installed verification after browser reconnection.

Kickoff decision (2026-10-28): retain this plan and Jack Heart's accepted outcome.
The replacement architecture remains **draft and blocked on a supported writer**;
the existing diagnostics are insufficient for shipment. Finding 37's read-only
check did not identify the missing authorization path. No new mutation probe or
live-account poll was justified. This is not a claim that all public APIs are
exhausted, nor approval to use private interfaces or select LOO-325.

**This slice — blocked:** replace the native transport and its exclusive fixture
together once a candidate meets the preservation conditions below. The focused
test must invoke that production writer, read exact bytes independently, and show
an injected failed save retains the prior readable item and access. Until there
is a viable candidate, no executable implementation sequence or native test command
can honestly be selected. Browser reconnection unblocks live diagnosis only; it
does not resolve this writer dependency.

Success means refresh remains invisible to an ongoing Session, including larger
provider payloads. The critical failure is a reported save error after partial
replacement or access loss. A return code, readback mismatch, or attempted rollback
cannot turn that failure into preservation evidence.

## Surviving implementation

- `provider_auth/mod.rs::read_claude_login` and `write_claude_login` select native
  Keychain storage for native macOS homes and private `.credentials.json` files
  for managed homes. Native reads fall back to the file only when the Keychain
  item is absent; Keychain read errors propagate. Ordinary unit tests use files
  even on macOS.
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
  Input/wait failures reap the child without claiming rollback; waits remain
  unbounded until the replacement implements the required 15-second limit.
- `provider_account/activation.rs::write_login` shares the native writer;
  `lf/commands/account_status.rs::managed_rows` observes and polls native identity.
  Existing diagnostic/recovery fields carry failures. SQLite account evidence is
  derived, not credential authority.

Four diagnostic tests cover hostile output, mismatch, numeric IO errors and
unstructured errors; they prove redaction, not native preservation. This repair
adds no migration, DTO or Swift change.

## Evidence and excluded alternatives

Detailed evidence summarized below, including contradictory preservation results:
`09dfdc6270:scratch/loo-661-save-refreshed-claude.md`, this section.
Complete findings 1–33, citations, probe receipts and earlier archive references:
`c871ddfb0c4956fdb816c079c61d71e68767733f:scratch/loo-661-save-refreshed-claude.md`.
The completed command-boundary probe is preserved at the same commit as
`scratch/claude_command_boundary_probe.py`; it is removed from the working plan
because it reproduces the rejected predecessor, not the required production path.
Its results remain constraints on replacement, not native acceptance.

Native probes used disposable `loopflow-LOO-661-*-probe-<uuid>` services, synthetic
bytes, bounded/reaped children, captured output and successful cleanup. No real
credential was directly read or printed. Apple source conclusions use Security
commit `db15acbe6a7f257a859ad9a3bb86097bfe0679d9`; they do not prove installed binary
equivalence or exhaust public interfaces. The following groups retain the original
finding numbers so the archived evidence stays addressable.

- **Transport (1–2, 30):** `security -i` has a 4,096-byte input buffer. Early
  64/1,242-byte round trips passed; a 2,048-byte failure retained old bytes.
  Later exact-command probes twice contradicted preservation at 2,048/8,192 bytes
  with five- and six-character account names: write exit 1, read exit 0, neither
  old nor intended bytes. They did not invoke compiled Rust. Published parsing
  executes each buffer read, consistent with partial replacement then error;
  installed parsing was not instrumented. Detached `security … -w` returned zero
  but truncated larger values to 128 bytes. PTYs, file/FD indirection and repeated
  `-w`/`-X` cannot append a password across commands.
- **Native access (3–5, 8–9, 11, 21, 27–29, 31, 33):**
  `SecKeychainItemModifyAttributesAndData` returned zero at 2/8 KiB but `security`
  reads exited 128; `ModifyContent`, Apple-signed `osascript`/`SecItemUpdate` and
  copied-access restoration left reads timing out. Writes replaced `apple-tool:`
  with writer `cdhash:`. Published non-null updates create a group and strip
  partition/integrity ACLs; null data leaves bytes unchanged. `security-framework`
  3.7.0 uses that modifier. Partition edits failed with `-25293` under disabled UI;
  initial JXA `-50` was a bridge failure. Signing/trusting an application alone
  supplies no partition authority. Combined data/access updates modify data first
  without encompassing rollback; pre-setting access affects only the old group.
  Public creation rejects duplicates. Private password-based access repair,
  TN3137's file shim and a different data-protection authority supply no accepted
  atomic replacement.
- **Encoding and other interfaces (10, 12–20):** raw `CSSM_DL_DataModify` bypasses
  encryption. `CSSM_EncryptData` still requires private envelope composition;
  `SecKeychainSetBatchMode` is private. Import handles keys/certificates; copy
  creates a group and strips access/integrity ACLs. Read-only macOS 26.0.1 dispatch
  inspection found `KCSetData`/`KCUpdateItem` use staged APIs with published
  new-group paths. Shared web credentials select another authority and pinned
  source returns `errSecUnimplemented`. Other findings were source/header review.
- **Outer transactions (22–26):** FileDL deferred commit around 2,048/8,192-byte
  modifications returning zero. Password reads failed (`-25300`) while metadata
  succeeded; by-ID reads saw new encoded data, queries saw the old snapshot and
  could not find the new decoding group. This prevented pre-commit verification.
  Rollback/autocommit restoration returned zero and independent readers recovered
  exact old bytes, proving neither unchanged ACL identity nor general rollback
  safety. No commit/access-restoration probe ran.
- **Copied-access creation (34, 2026-10-28):**
  `SecKeychainItemCreateFromContent` with copied access failed `-25293` at both
  sizes with interaction disabled. Independent reads retained exact old bytes;
  candidates were absent (44). Promotion/transactions were not tested. This rejects
  that construction, not every explicit-access creation. Full citations and receipts:
  `78530325c1:scratch/loo-661-save-refreshed-claude.md`, finding 34.
- **Live observations (6, 32):** launches reported one write/readback failure
  without status and two expired OAuth errors. Later 0.14.18/0.14.19 polls accepted
  jack@loopflow.studio and jackstah@gmail.com with verified capacity and no recovery;
  the local row remained unavailable. At `1793163454` both managed accounts had
  three verified windows. Such polls expose no refresh occurrence, storage stage,
  payload size or causality. On 2026-10-27 at `1793169073`, supported
  `lf account claude --json` instead rejected jack@loopflow.studio with recovery
  and no windows; at `1793169147`, an allowlisted diagnostic confirmed
  `refresh grant rejected`. This maps to HTTP 400 `invalid_grant` before the writer,
  not the historical save failure's cause or the reason for token invalidation.
  jackstah@gmail.com still had three verified windows; the local row was unavailable.
  No token was directly inspected, forced to expire or passed in argv. Supported
  `lf account connect claude jack@loopflow.studio` is now a live-acceptance dependency;
  no sign-in or route change was attempted. Further polling without recovery adds
  no acceptance evidence.

### 35. Explicit access and partition isolation (2026-10-28)

Eight disposable cases reconstructed access with `SecAccessGetOwnerAndACL` /
`SecAccessCreateFromOwnerAndACL`, or used `SecAccessCreate` trusting only
`/usr/bin/security`, at exactly 2,048 and 8,192 bytes:

| Candidate access | Creation, both sizes | Independent `security` reader |
| --- | --- | --- |
| Original owner/ACLs, omit integrity only | `-25293` | Candidate absent (44) |
| Original owner/ACLs, omit partition only | 0 | 15-second timeout |
| Original owner/ACLs, omit partition and integrity | 0 | 15-second timeout |
| Fresh access, trusted `/usr/bin/security` | 0 | 15-second timeout |

Successful candidates had one partition ACL without the originals' `apple-tool:`.
All eight originals retained exact bytes and semantic ACL fingerprints (owner,
subjects, authorization tags, delegation, entry tags and validity, excluding
allocation addresses/handles). This proves neither storage-group identity nor
production preservation: no original was replaced or assigned new access.
Cleanup succeeded; absent candidates returned 44. Children were bounded to 15
seconds and reaped; output held only statuses, sizes and comparisons. No promotion,
transaction, real credential, route, Session or global trust change followed.

Published-source explanation: `changeAcl` protects the partition tag unless the
operation's credentials authenticate the database. Creation's `Access::Maker`
supplies temporary group credentials; adding the tag is denied. Integrity
installation also checks partition membership. A trusted-application list alone
supplies no partition authority. This is inference, not an installed securityd trace.

The earlier `SecACLRemove` differential returned `-67702` when removing integrity:
published `ACL::remove` leaves an invalid-form tombstone that `addIntegrity` can
find and `setIntegrity` rejects. This does not contradict successful fresh
construction without both entries.
Full citations and initial probe blob IDs:
`1266f97dfeb8bf8b08731868dd57f2917145524b:scratch/loo-661-save-refreshed-claude.md`, finding 35.
Corrected source, runner and sanitized results:
`66ae255389e2800111dfc056c5009d380bab512a:scratch/keychain-access-probe/`.
The rejected harness remains archived, not production acceptance coverage.

### 36. Database authentication and existing-group retention (2026-10-28)

Read-only review of pinned Apple Security source and installed SDK declarations;
no mutation probe, credential read, unlock, authentication or trust change ran.
Full call paths and citations:
`1266f97dfeb8bf8b08731868dd57f2917145524b:scratch/loo-661-save-refreshed-claude.md`, finding 36.
These are source conclusions, not installed securityd traces:

- `KeychainDatabase::checkCredentials` requires a valid database passphrase/key
  in the ACL operation's `KEYCHAIN_LOCK` sample, ignores prompt samples, and
  validates even when unlocked. `CSSM_DL_Authenticate` stores credentials in
  `mCred`, which that check does not consult; `SecKeychainUnlock` does not supply
  credentials to subsequent ACL edits. Creation passes `Access::Maker`'s random
  setup-password sample, and `SecKeychainItemCreateFromContent` accepts no database
  credential argument. Authentication first cannot authorize finding 35's creation.
- `SSDbUniqueRecordImpl::modify` can encode data with the existing `SSGroup`, but
  `ItemImpl::updateSSGroup` reaches it only for null new data; replacement creates
  a group. Public `SSDLSession::DataModify` uses `SSUniqueRecord` inheriting raw
  `DbUniqueRecordImpl`, not that encoder. Retaining an item/record reference cannot
  change dispatch. Calling internal C++ or recreating its envelope crosses the
  private-interface boundary.

**Missing input:** a supported creation API forwarding the required database
credential, or a public replacement API reaching the group-retaining encoder.
Neither was identified. A database password alone is not a demonstrated solution;
do not collect one speculatively. Private interfaces, trust changes or provider-owned
refresh require a scope decision Jack Heart has not made; LOO-325 stays unselected.
No new candidate qualifies for native round-trip and failed-save-preservation tests.

### 37. Local authentication context is not a demonstrated bridge (2026-10-28)

Read-only review tested whether `kSecUseAuthenticationContext` could supply
finding 36's missing authority without collecting another secret. Apple's
[API documentation](https://developer.apple.com/documentation/security/ksecuseauthenticationcontext)
describes reusable `LAContext` authentication. It does not promise preservation
of a file-keychain item's partition ACL. [TN3137](https://developer.apple.com/documentation/technotes/tn3137-on-mac-keychains)
distinguishes file-keychain ACLs from data-protection access controls; SecItem uses
a shim for the former. [kSecAttrAccess](https://developer.apple.com/documentation/security/ksecattraccess)
configures the file-keychain ACL, not the data-protection access-control object.

In pinned Apple source, LAContext externalization occurs in the
[data-protection implementation](https://github.com/apple-oss-distributions/Security/blob/db15acbe6a7f257a859ad9a3bb86097bfe0679d9/OSX/sec/Security/SecItem.m#L1418-L1425).
The separate [file-keychain update](https://github.com/apple-oss-distributions/Security/blob/db15acbe6a7f257a859ad9a3bb86097bfe0679d9/OSX/libsecurity_keychain/lib/SecItem.cpp#L2517-L2526)
still modifies content before setting access; that implementation contains no
handling of `kSecUseAuthenticationContext` or `kSecUseCredentialReference`.
Thus this reviewed path supplies no context-to-database-credential bridge and no
atomic access-preserving replacement. This is a source conclusion, not an installed
securityd trace or a general public-API impossibility proof. No authentication,
Keychain mutation, real credential read, or Session change ran.

Argv credentials, broad trust changes, delete/recreate, file migration and
unprotected post-commit ACL repair remain excluded (7). Local preservation must
prove readable old bytes and unchanged access, but cannot prove server validity
after OAuth rotation: the refresh response precedes saving, without a provider/
Keychain transaction. Neither limitation relaxes Jack Heart's requirements.

## Remaining implementation and acceptance

1. Establish a distinct supported mechanism addressing findings 35–36 before
   another mutation probe. Prove native partition access, exact independent reads
   and preservation of original bytes/access on disposable synthetic items. Then
   replace the production transport and its exclusive fixture together.
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
   Include differing service/account prefix lengths from finding 30: rejected
   oversized input must not silently replace old data with a partial value.
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

- Completed `scratch/keychain-access-probe/{probe.c,run.py,results.json}` is removed;
  finding 35 retains the counterexamples and exact Git reference. Do not restore
  the harness as acceptance coverage or extend these rejected constructions
  without a mechanism addressing partition authority and failure preservation.
- Replace the `security -i` spawn, hex script and command-string escaping restrictions
  inside `write_claude_keychain_credential` once a compatible writer is proven.
  Replace its exclusive stand-in in
  `claude_persistence_keychain_diagnostics_distinguish_write_readback_and_mismatch`
  with the surviving writer's diagnostic fixture, retaining stage/secrecy checks.
  Keep one writer behind `write_claude_login`, shared by refresh and activation;
  no small-value fallback, size dispatch, dual writes or bypass.
- Retain shared readers, service/account parsers and tests, managed file writes,
  explicit profile-capture cleanup and race/rotation tests; they have other consumers.
  The refresh path already retains the original secret and serializes once; its
  redaction boundary needs no additional abstraction. No production deletion is
  ready until the replacement writer is proved.

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

Check — kickoff (2026-10-28): `git diff --check` and `lf context` pass; documentation/source review only, no code or mutation probe rerun. Gate owns affected checks; writer mechanism, native acceptance and installed acceptance remain unresolved.
