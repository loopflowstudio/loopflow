# LOO-661: Save refreshed Claude credentials reliably

Status (2026-10-28): **Writer unresolved; diagnostics only; not implementation-ready.**
No candidate meets transport, native reader-access and failed-save preservation
requirements. The checked-in `security -i` writer still has the destructive size
failure below. Public interfaces are not exhausted; revisit rejected routes only
with a mechanism addressing their counterexamples. In particular,
[`kSecAttrAccess`](https://developer.apple.com/documentation/security/ksecattraccess)
support does not establish atomic preservation (findings 31/33). Creating a
separate item with copied access also fails before promotion (finding 34).
The implementation sequence is conditional on that missing mechanism; Jack Heart's
accepted outcome and boundaries remain unchanged. Supported reconnection for
jack@loopflow.studio is a separate live-acceptance dependency, proving neither
the writer nor the historical save-failure cause.

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

**Demo:** after a naturally required refresh on the repaired installation,
`lf account claude --json` accepts the same account with verified capacity, and
native Claude continues using that login without reauthentication. The current
rejected grant must first be recovered through the supported sign-in flow;
that recovery alone will not demonstrate the repair.

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

- **Transport (1–2, 30):** `security -i` has a 4,096-byte input buffer; hex reaches
  it at 2 KiB, and raw JSON cannot fit 8 KiB. Early probes round-tripped 64/1,242
  bytes and retained old bytes after a 2,048-byte failure. Later exact-command
  probes contradicted preservation: both five- and six-character account names
  at 2,048/8,192 bytes produced write exit 1 and read exit 0, returning neither old
  nor intended bytes. Read-output lengths were 3,990/1,994, not raw encoding sizes.
  All six items were deleted; the result reproduced twice with 15-second child
  deadlines. These probes did not invoke compiled Rust. Published interactive-loop
  source executes each buffer read and replaces final status per command, consistent
  with partial replacement then error; installed parsing was not instrumented.
  Detached `security … -w` returned zero but truncated larger values to 128 bytes.
  PTYs, file/FD indirection and repeated password options do not solve this:
  `-w`/`-X` replace, not append; separate commands replace separately.
- **Native access (3–5, 8–9, 11, 21, 27–29, 31, 33):**
  `SecKeychainItemModifyAttributesAndData` returned zero at 2/8 KiB but existing
  `security` readers exited 128; `ModifyContent` left reads timing out. Native
  writes replaced `apple-tool:` with writer `cdhash:`. Published non-null updates
  create a new group and copy access after stripping partition/integrity ACLs;
  null data leaves bytes unchanged. Preparing old access first therefore does not
  preserve it. `security-framework` 3.7.0 uses that modifier. Apple-signed
  `osascript`/`SecItemUpdate` and copied-access restoration also returned zero while
  reads timed out. Edited partition access failed with `-25293` under disabled UI;
  initial JXA `-50` results were bridge failures. Apple-anchored `com.apple.security`
  gets `apple-tool:`, other Apple code `apple:`; signing alone proves no access.
  Public creation permits explicit access but rejects valid duplicates. SetAccess
  edits the current group, not the next replacement. Although `kSecAttrAccess`
  permits a combined update, published `_UpdateKeychainItem` modifies data before
  setting access; failure does not restore data, and `SecItemUpdate_osx` adds no
  encompassing transaction. `SecKeychainItemSetAccessWithPassword` is private,
  requires a Keychain passphrase and changes only access. TN3137's default file
  shim retains these limits; data protection selects another storage authority.
  No compatible atomic data/access replacement is established.
- **Encoding and other interfaces (10, 12–20):** raw `CSSM_DL_DataModify` bypasses
  encryption; JSON would overwrite encoded storage. No public password encoder
  was found; `CSSM_EncryptData` still needs private group/label/IV/ciphertext
  composition. Private envelope reconstruction/C++ ABI remain excluded.
  `SecKeychainSetBatchMode` is private; public FileDL controls have the limitation
  below. Import handles keys/certificates; copy strips access/integrity ACLs and
  creates a group. Read-only macOS 26.0.1 (25A362) dispatch inspection resolved
  `KCSetData`/`KCUpdateItem` to staged APIs with published new-group paths.
  Shared web credentials use another authority; pinned source returns
  `errSecUnimplemented`. Other findings in 8–20 were source/header review, not
  new native mutation proof.
- **Outer transactions (22–26):** FileDL deferred commit around 2,048/8,192-byte
  modifications returning zero. Retained/fresh password reads returned `-25300`
  while metadata succeeded. By-ID reads saw new encoded bytes (2,084/8,228), but
  group-key queries returned end-of-data (`-2147413715`); fresh password queries
  still saw the original 52-byte encoded record/key. Published source reads
  modified records by ID but queries the pre-write snapshot, consistent with the
  unavailable decoding group. Another lookup/wrapper does not supply pre-commit
  verification. Rollback/autocommit restoration returned zero and fresh `security`
  readers recovered exact old bytes. This proves those recoveries only, not
  unchanged ACL identity or general rollback safety. No commit/access-restoration
  probe ran; committing first abandons verification and leaves access unresolved.
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

Argv credentials, broad trust changes, delete/recreate, file migration and
unprotected post-commit ACL repair violate the brief (7); provider-owned refresh
would select LOO-325. Local preservation must prove readable old bytes and unchanged
access, but cannot prove server validity after OAuth rotation: `fresh_claude_token`
receives the refresh response before saving, with no provider/Keychain transaction.
Neither limitation relaxes Jack Heart's requirements.

### Copied-access creation counterexample (34, 2026-10-28)

Apple documents `SecKeychainItemCreateFromContent` accepting an access object
copied from another item via `SecKeychainItemCopyAccess`
([API contract](https://developer.apple.com/documentation/security/seckeychainitemcreatefromcontent(_:_:_:_:_:_:_:))).
This suggests staging a readable candidate before changing the original, avoiding
the uncommitted new-group lookup in findings 22–26. It does not promise an atomic
promotion or preservation of partition access.

A new disposable probe created a synthetic original with `security -i`, copied
its access object without reading its password through the API, then attempted
native creation under a second unique service with exactly 2,048/8,192 bytes.
With interaction disabled, lookup and access-copy returned zero; both creations
returned `-25293`. Independent `security` reads returned exact original bytes;
the new items were absent (exit 44). Both original deletions succeeded; cleanup
of absent candidates also returned 44. Every child had a 15-second timeout and
was reaped. Output held only statuses, sizes and equality booleans. No real
credential, routing, Session or global trust setting changed.

This rejects copied-access staging as an established escape from the replacement
counterexamples. No promotion or transaction experiment followed; creation itself
failed. It does not establish that every explicit-access creation is impossible.
The supported writer remains unresolved; no product choice has been relaxed.
Reproducible evidence is preserved in Git blobs: C source
`1059c9ed006003509adf938405870afc3b0cc4a9`, Python runner
`b80bb3edc52b65f7fd2d1585a249d40b744bad9d`, JSON results
`23c5b26bbcbea7af1c2334eb89b4571dd6202938`. Restore them as `probe.c`, `run.py`
and `results.json` in one disposable directory; compile with
`clang -Wno-deprecated-declarations probe.c -framework Security -framework CoreFoundation -o probe`,
then run `uv run python run.py`.

## Remaining implementation and acceptance

1. **This slice:** prove a supported writer's encoding, access and failure preservation, then
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

Check — kickoff (2026-10-28): copied-access creation probe executed both native sizes and returned `-25293`, preserving both originals; `git diff --check` and `lf context` pass. No production change or broad test rerun; writer proof, affected gate checks and Infrastructure's installed acceptance after supported reconnection remain pending.
