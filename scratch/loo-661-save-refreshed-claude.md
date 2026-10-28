# LOO-661: Save refreshed Claude credentials reliably

Status (2026-10-28): **Implementation stopped at the writer design boundary;
native acceptance fails.**
The reader now decodes Unicode readback exactly. All eight mocked-refresh/native
storage cases execute and clean up: ordinary ASCII, Unicode and invalid-input
preservation pass; four large replacements still destroy old data. The defective
`security -i` writer remains, with no eligible replacement established. The reader
fix does not complete this repair or permit delivery.

`ed6c82b2b` closes finding 40 with finding 41's Unicode proof. Findings 42–43
exclude two examined public CSSM bridges, not all public APIs. Review at
`c98606d8703498f44f35843dae1868a2f5c4003a` confirms the writer and fixture still
match these findings. Neither native synthetic proof nor release evidence
establishes installed Claude recovery.

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

Two independent gaps remain: the [writer decision](#writer-design-decision-needed)
must address partition authority and failed-save preservation before another
mutation probe; [live diagnosis](#live-diagnosis) needs browser reconnection before
the historical save failure can be investigated further. Local rollback cannot
undo server-side OAuth rotation: refresh and saving share no transaction.

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
- The explicit native fixture mocks HTTP and opts one disposable home per isolated
  child into real storage, active-home selection and account reporting. Results and
  remaining coverage are recorded below; ordinary tests stay on files.

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

**Public key enumeration (43, October 28; source review only):** higher-level
searches filter group keys; lower-level symmetric-key queries return `CSSM_KEY`
references but do not associate them with the target password. FileDL cursors
read the pre-modification snapshot, so a before/after query cannot find the new
group key before commit; content update returns no key ID. By-ID reads see modified
records, but guessing IDs or parsing the private password envelope is excluded.
Committing to discover the key puts partition repair outside the preservation
boundary. Database authentication and full group/ACL rollback also remain unproved.
No Keychain enumeration or mutation ran. Exact Apple source citations and download
checks remain at `21f257c337584cefe24c068bcb5c8de2c4f79a64:scratch/loo-661-save-refreshed-claude.md`,
“Public key enumeration (43)”; this establishes no installed-binary equivalence or
public-API exhaustion.

## Live diagnosis

**Findings 6, 32:** the original save/readback failure had no status.
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

Further implementation needs that supported mechanism, then the native fixture's
large-value and injected-failure proof.

**Unselected proposal (October 28):** Jack Heart and Infrastructure must decide
whether to obtain Apple guidance for this public-interface path, or commission a
separately reviewed provider-owned refresh design under LOO-325 if none can be
established. Neither action is selected here. Provider-owned refresh remains an
investigation choice, not a proven fix or waiver of the accepted requirements.
Private APIs and trust changes remain excluded. Finding 43 justifies no production
change or native mutation; the reader repair alone cannot advance delivery.

### Containment alternative — draft, not selected (October 28)

The direct conversation preserved in `questions.md` proposes stopping Loopflow's
native refresh before the OAuth request. Source inspection confirms one boundary
in `fresh_claude_token`: after the unexpired-token return and before refresh-token
lookup/POST. Both expiry and the usage request's 401 retry pass through it;
identity reads also share this function. Native credentials remain authoritative,
and each subsequent poll rereads them. Managed file-backed accounts would keep
refreshing. No extra storage, background Claude launch or account-route change is
needed for that narrower behavior.

Its demo would be an expired native login reporting sanitized `unavailable`, zero
refresh requests and unchanged bytes, followed by accepted identity/usage after
an external credential replacement. It sacrifices automatic capacity recovery
until Claude refreshes or Jack reconnects. It does not repair activation writes,
arbitrary-size persistence or the already rejected grant. Selecting containment
therefore requires an explicit product-scope decision; Jack Heart's request for
help and existing shipping authorization do not select it. LOO-661's complete
acceptance remains binding.

Read-only upstream review confirms reports of the same transport defect in
[#28541](https://github.com/anthropics/claude-code/issues/28541) and
[#30337](https://github.com/anthropics/claude-code/issues/30337), plus reader-access
loss after provider refresh in
[#94491](https://github.com/anthropics/claude-code/issues/94491).
These are reporter observations, not provider guarantees or evidence about the
installed account. Suggested argv transport violates credential secrecy; splitting
or shrinking stored JSON changes provider storage semantics. None supplies the
missing preservation proof. Provider-owned refresh alone is not established as a
complete solution.

## Remaining implementation and acceptance

1. **This slice — writer capability proof and replacement (blocked).** Select and
   prove an eligible writer mechanism before replacing the production
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
- Native fixture guidance lives in `rust/loopflow/tests/README.md`; removed the
  duplicate global `AGENTS.md` rule. Keep containment's behavior and tradeoff only
  in this design; `questions.md` retains attribution, selection and sync recovery.

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

Check (October 28 compress; prose only) — `git diff --check` and `lf context`: PASS; no code changes or test rerun. Prior `ed6c82b2b` decoder: PASS (1), native acceptance: FAIL (8 executed, Unicode passes, 4 large-value failures, all cleaned), format/Clippy: PASS. Gate retains affected suites and complete acceptance; Infrastructure retains installed verification.
