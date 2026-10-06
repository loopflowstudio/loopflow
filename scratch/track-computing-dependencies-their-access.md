# Computing dependencies, access and cost

LOO-389 · October 6, 2026 · Approved design; local core implemented, full acceptance outstanding

Jack Heart requested a dependency inventory connecting consumers, access and
spending. He chose `lf auth` and all accounts currently tracked in Doppler,
naming Anthropic, OpenAI, Mercury, AWS, GitHub, Fly.io and Runpod as a
non-exhaustive starting list. This supersedes DigitalOcean-first and `lf spend`.
The [approval record](design-approval.md) authorizes the design and continuation
of the existing Flow; account connections, real rotations and resource changes
still require separate authorization. The local inventory/invoice core now exists;
no live financial evidence exists on this branch.

The October 6 review's findings are incorporated below: linked account costs do
not imply allocations, access evidence is environment/version-specific,
retirement requires consumer cutover, and usage estimates remain separate from
bills. That review inspected local sources without revalidating external provider
documentation; [review provenance](dependency-access-and-billing-review.md)
remains local because `lf commit` excludes scratch files.
The design owns requirements; [questions](questions.md) tracks unresolved inputs.

## Implementation evidence — October 6

The local core introduced in `8216abd54` and reconciled at `93425cdad` remains
intact. Commit `0db558704` adds effective dependency relationship history,
dependency access observations, designated exports and administrative rotation
receipts. One `computing_dependencies.sql` draft owns the schema. These are local
implementation results; full Task acceptance and publication remain outstanding.

The uncommitted compression changes separate current source/account membership
from durable invoice identity. Inventory imports replace a source's current
account links; removed accounts retain their historical invoices. Replaying a
stored revision returns its original evaluation even outside the current scope;
new documents and new corrections must satisfy current membership. The added
regression covers scope removal, retained totals, replay and rejection of a new
document. Its invalid-account import assertion preserves billed evidence but
does not yet prove rollback of every earlier inventory mutation.

`lf auth dependency history` derives half-open effective intervals from existing
inventory snapshots, preserving consumers/account/resource/source/requirement
links and provenance. Same-date corrections use the last import; omitted
records remain effective. It does not yet provide historical account, resource,
credential or environment projections. `dependency show` uses current metadata
and the requested billing period, and now displays only its own access requirements
and observations. Text and JSON use the same Home, requirement revision, exact
Doppler reference and version matching. Recurring estimates appear separately in
text. Current provider permission probes remain unavailable.

`lf auth export` requires a repository and optionally a Wave. Its separate DTO
contains matching evaluated amounts, rule revisions and opaque source references;
it excludes invoice totals, descriptions, unrelated recipients, inventory and
free-form coverage. Administrative `report` filters remain presentation filters.
`scripts/check_spend_isolation.py` successfully consumed a USD 30 fixture export
in Docker with a read-only filesystem/mount, no network or inherited authority,
and no administrative database, credential locations, SSH or Docker sockets.
This proves the fixture boundary, not production agent provisioning.

`lf auth access rotate` records a candidate reference. Rotation receipts represent
explicit provider/operator evidence; importing them executes no provider action.
All required environments must verify the candidate before the original credential
ID's active reference switches transactionally. Separate post-activation cutover
receipts and an explicit complete-consumer-inventory attestation gate retirement.
Failure retains the old reference before activation, and retains retirement-pending
state afterward. Restart preserves receipts; pre-activation cancellation releases
a failed candidate. Concurrent candidates serialize through the Store. Changed
consumer/credential metadata requires reconciliation; automatic recovery of that
inventory drift remains unfinished.

The synthetic HTTP provider/Doppler test creates and provisions runtime keys,
rejects a failed candidate, verifies both required consumers, restarts after
activation, retains the old key until consumer cutover, retries failed retirement,
and proves the candidate succeeds while the retired key is rejected. Assertions
cover secret absence from CLI stdout/stderr, serialized receipts and Store files.
The test harness supplies receipts from its HTTP outcomes; the production CLI has
no provider executor and does not independently authenticate imported receipts.

Focused coverage additionally proves the USD 170→160 correction, USD 110/30/20
attribution, duplicate lines, unknown/zero costs, preserved rule evaluations,
and a later correction evaluated under a changed rule. Output DTO fixtures and a
released-frontier migration test are present. Shared account links never allocate
charges. No Session usage reader changed.

`provider_auth/doppler.rs` remains the narrow bounded resolver extracted from SSH;
no real secret was fetched. `discovered-inventory.json` contains the seven starting
services, 13 exact metadata hints and explicit gaps, without invented accounts or
consumers. It has not been imported into Jack Heart's administrative Store.

Review fixed two evidence hazards: reference changes now invalidate old access
observations even if version strings coincide; internal rotation inventory events
retain discovery gaps instead of accidentally clearing coverage. Rotation receipt
selection uses observation time, and cutover receipts cannot predate activation.

### Provider contract finding

The October 6 public [Runpod OpenAPI contract](https://api.runpod.io/v2/openapi.json)
exposes aggregate and resource-specific time buckets with echoed query windows,
record counts and totals. It does not document invoice IDs, invoice finality or
credit/refund reconciliation on those billing endpoints. The official
[scoped-key announcement](https://www.runpod.io/blog/scoped-api-keys-runpod)
describes read/no-access endpoint scopes; it does not establish the permissions
of the discovered key. [Fly billing docs](https://fly.io/docs/about/billing/)
describe downloadable invoices through its billing portal; no machine-readable
invoice contract was established in this research.

This invalidates treating an arbitrary provider billing response as a complete
invoice. Keep the implemented invoice core for actual normalized documents.
Before the provider-reader cut, choose an invoice/export source or extend the
records with distinct provider usage-ledger evidence, without passing bucket sums
off as settled bills. The October 6 implementation re-fetched the public contract and confirmed the
same distinction. No invoice reader is selected; adding provider usage-ledger
evidence would require an explicit design revision and cannot replace billed
acceptance. It does not block local metadata, test rotation or isolation work.

## Experience and demo

Jack can start with a dependency and see its repositories/Waves, accounts,
resources, credential requirements and billed costs—including unknowns. Starting
with a repository/Wave produces a spending breakdown whose amounts lead back to
provider evidence. Shared relationships do not multiply charges.

Implemented inspection CLI (provider import still requires `--file`):

```sh
lf auth dependency show build-workers --period 2026-09 --json
lf auth access show laptop --json
lf auth report --period 2026-09 --repo loopflowstudio/loopflow --json
lf auth source import <source> --period 2026-09 --file invoice.json
```

Dependency inspection separates its attributed charges from linked account costs.
For example, two dependencies sharing a USD 100 account may each show that account
and its USD 100 invoice as linked evidence. Neither owns USD 100 merely because
of the link. Without a dependency mapping, each shows unknown attributed cost,
and the account's USD 100 remains visible once in the report. Dependency totals
are not added to repository/Wave totals; they are separate views of the same
charge evidence. Consumer membership alone never creates an allocation.

Access inspection lists each requirement's purpose, intended permissions,
credential/tool reference, last verification environment and time, and missing
evidence. A laptop that has never been probed says unverified even when the
importer can use the same credential. Failure retains the previous observation
as dated history, alongside the current failure and next required action.

The headless demo seeds two repositories with same-named Waves, two dependencies
sharing one account and credential, a second account, a subscription and an
unknown-cost dependency. It imports a synthetic provider invoice twice, corrects
it, and shows unchanged historical attribution after a mapping change. An access
probe and failed/successful test rotation show exactly which environments are
ready. The live demo later uses an explicitly authorized account and period;
fixture success never substitutes for that Task acceptance.

Success is answering “what uses this key, and where did this bill go?” in those
three inspections. Failure is an attractive total assembled from duplicated
account links, stale credentials, or estimates passed off as settled bills.

## Current architecture and chosen placement

Rust owns persisted records in the existing SQLite Store. Add `spend/` domain
modules, `store/spend.rs` and `store/sqlite/spend.rs` for financial records;
expose the dependency, access and cost commands through `lf auth`, integrating
with the existing authentication command structure. Internal financial ownership
does not require a separate public `lf spend` command. One Task migration draft, created with
`uv run python scripts/new_migration.py computing_dependencies`, extends this
Store. No second database, daemon, Python billing authority or manifest mirror.
An import document is input to a transaction, not a second live inventory.

Existing sources to reuse:

- `repository.rs`: `CanonicalRepo` collapses worktrees; `RepoId` represents the
  provider repository. Resolve local consumers to the existing canonical repo
  and Wave ID, retain provider repo identity when known, and never key by basename
  or Wave name alone. Unregistered external repositories may be declared by
  provider identity; unresolved local identity is explicit, not guessed.
- `session_record.rs`: `SessionHistory`, `SessionUsage`, recorded Task/Wave IDs,
  finality and evidence gaps; `lf/commands/session_history.rs` owns the reader.
  Reference this evidence for drill-down without copying or reallocating it.
  `performance/{context-cost,context-ablation,bind-attribution}.md` are historical
  instruments, not billing importers. Keep prospective usage attribution.
  Existing `SessionUsage.cost_usd` is optional floating-point usage evidence;
  display it separately with its provenance and gaps. It must not become exact
  invoice money or be added to a bill covering the same usage. Model evidence
  does not establish which provider key was used; missing key linkage stays unknown.
- `store/mod.rs::ProviderAccount` and `provider_auth/` own model login identity,
  selection and routing. A cloud billing account is a different entity; link an
  existing managed account when relevant, never duplicate its credentials or
  credential state into a competing authentication authority.
- `lf/commands/ssh.rs` resolves named Doppler secrets locally but also assembles
  GitHub, PM and model access. Reuse the narrow resolver after extracting it into
  `provider_auth/doppler.rs`; do not use the broad SSH bundle for reporting.
  Preserve existing SSH behavior and tests at that cutover.
- `scripts/check_monthly_spend.py` reads bank exports for automation budgets.
  It remains a separate payments consumer; do not feed it invoices as payments.

Infrastructure retains secret resolution, process isolation and authentication
mechanics; Spend owns dependency metadata, permission evidence and financial
interpretation. Coordination here is an explicit code boundary based on its
GOAL and managed-account memory, not a claim of review by another Work.

## Authoritative records

Use typed IDs, explicit foreign keys and ordinary relation tables, not a generic
graph engine. All relations have provenance and effective-from/to dates where
changes affect attribution or access.

| Record | Authority and relations |
|---|---|
| Dependency | Named software/service capability; remains present with no account, cost or known access. Many consumers, accounts, resources, sources and credential requirements. |
| Service account | Provider + provider-native account identity; many dependencies and billing sources. Optional links to existing managed ProviderAccount IDs. No login/routing writes. |
| Resource | Account + provider resource ID; optional discovered project/tags and dated observation. Many dependency links. Deleted resources retain historical identity. |
| Billing source | Provider account/export scope, supported capabilities, credential reference and coverage. May span multiple accounts/dependencies. |
| Credential reference | Doppler project/config/secret name and non-secret provider key ID where available; purpose, declared permissions, observed scope evidence, expiry/rotation evidence. Never a value or secret-derived digest. |
| Access environment and requirement | Laptop, agent environment or principal; link existing Home ID where available. Required credential/tool, purpose and scope. Required access and observed possession are distinct. |
| Access observation | Reference/environment/version, operation, observation time, success/denied/unavailable, scope evidence source. Successful reads do not prove absence of write authority. |
| Invoice revision and charge | Source account, provider invoice ID, complete revision hash, period, currency, fetched time and source-generated time if present. Exact decimal amounts, resource/usage detail, kind and source locator. |
| Attribution rule and result | Effective interval, resource/project/explicit selection, direct assignment or allocation weights, reason. Immutable evaluated results link charge revision and rule revision. |
| Rotation | Old/candidate references, required environments, verification receipts, activation and retirement receipts; durable recoverable state with no secret material. |

Inventory mutations use `lf auth inventory import <file>` with validated,
non-secret structured input, stable record IDs and transactional upserts. Reports
use read-only Store access; inspection never polls a provider or fetches secrets.
Local metadata edits confer no provider permissions. DTOs use required or explicitly
optional fields, decimal strings for money, and fixture coverage. No Desktop
consumer is added in this Task; existing usage/app DTOs stay unchanged.

## Doppler inventory and integration depth

Start with all accounts Jack currently tracks in Doppler. Inspect project/config
and secret-name metadata only; never retrieve values to discover accounts. Record
which scopes were inspected and which were unavailable. The seven named services
are a starting checklist, not a hardcoded provider allowlist or one account each.
Secret names are discovery hints, not proof of account identity, permissions or
ownership. Keep unresolved references visible; reconcile duplicate references
using non-secret evidence rather than comparing secret values.

| Starting service | Intended inspection depth |
|---|---|
| Anthropic, OpenAI | Accounts and scoped credential references; reuse recorded model/usage evidence; expose billing and key-linkage coverage separately. |
| AWS, Fly.io, Runpod | Accounts, resources, credential requirements and granular usage/cost evidence where supported. |
| GitHub | Account/organization access and subscription or metered charges where supported. |
| Mercury | Account and credential/access inventory; bank transactions and connectivity remain separate work. |

For every discovered service, show supported, unavailable, unverified and missing
capabilities explicitly. Inventory coverage spans all discovered accounts even
when a billing reader does not yet exist. Do not label an unimplemented capability
as unsupported by its provider. Match service depth to the Task's acceptance;
a names-only inventory does not complete this Task.

Choose the first live compute billing import from the discovered existing accounts
after checking available billing evidence, narrow read scopes and an authorized
period/export. DigitalOcean is no longer an implementation target. Research the
selected provider's current official API/export contract before implementing its
reader; do not carry over DigitalOcean-specific invoice UUID, currency, pagination
or scope assumptions. Full revision replacement remains the proposed approach
where a source supplies complete invoices without stable line identities.

### Discovery evidence — October 6, 2026

Metadata-only commands (`doppler projects --json`, `doppler configs -p <project>
--json`, and `doppler secrets --only-names -p <project> -c <config> --json`)
reached six visible projects and all 25 listed configs without a failed scope.
Each paginated project/config list contained fewer than its default 100 results.
The visible projects are cadenza, etude, hootro, kata, loopflow and nonna; each
has dev, dev_personal, stg and prd, and loopflow also has dev_billing. This is
coverage of visible metadata, not proof that every organizational account is
visible or that a credential works. No secret values were requested.

| Service hint | Observed reference names and scope | Remaining evidence |
|---|---|---|
| Anthropic | `ANTHROPIC_API_KEY` in hootro/prd, loopflow/dev_personal and nonna/dev_personal; `ANTHROPIC_ADMIN_KEY` in loopflow/dev_billing | Account links, key scopes, billing authorization |
| OpenAI | No explicitly OpenAI-named reference found | Account/reference discovery gap; OpenCode is not evidence of an OpenAI account |
| Mercury | `MERCURY_API_TOKEN` in loopflow/dev_billing | Account and permissions unknown; bank connection excluded |
| AWS | `AWS_ACCESS_KEY_ID` and `AWS_SECRET_ACCESS_KEY` in cadenza/prd and kata/prd | Could address S3-compatible services; names do not establish AWS billing accounts |
| GitHub | `DEPENDABOT_MERGE_TOKEN` in loopflow/prd is a candidate access reference | Provider/account and billing scope unconfirmed |
| Fly.io | `FLY_API_TOKEN` in loopflow/dev_billing and loopflow/prd | Account identity, shared reference relationship and billing scope unknown |
| Runpod | `RUNPOD_API_KEY` in etude/prd | Account identity and billing scope unknown |

Other names suggest Cloudflare/R2, Apple/App Store, Google, Twilio, Letta, Asana,
WorkOS, Discord, Linear, Notion, Cargo registry and CubeCobra access, plus database
and application signing credentials. These remain discovery hints, not asserted
service accounts, confirmed consumers or a complete dependency inventory.

Doppler remains the credential authority. Inventory metadata grants no permission
to fetch values, connect providers or rotate keys. Permission verification must
separate successful reads from read-only enforcement and distinguish current
online evidence from cached access. Credential creation/revocation and version
support require provider-specific evidence rather than assumptions.

## Billing ingestion and attribution

Fetch the selected provider’s complete billing evidence for the selected period,
including full pagination and reconciliation totals where provided.
Stage normalized financial fields locally, validate page completeness and totals,
then publish one revision atomically. Omit addresses and unrelated personal data;
never retain response headers, auth payloads or raw error bodies. Source locators
and normalized evidence allow tracing without copying arbitrary provider output.

Identity is `(source, account, provider billing-document ID)`, not month or resource. Compute a
canonical multiset hash of normalized items plus financial summary, preserving
identical-line multiplicity. Same revision is idempotent; changed content creates
an immutable revision and atomically supersedes its predecessor. A missing line
in a complete replacement is a correction, not another charge. No attempt to
match unstable line IDs across revisions. Concurrent imports serialize publication
per invoice; incomplete pages and interrupted imports cannot replace good data.

The source ID names a stable billing authority, not a credential or transport.
Credential rotation and an export of the same provider invoice reuse that source.
Overlapping sources must be reconciled to one invoice identity before contributing
to totals; unresolved overlap is a visible coverage gap, not two expenses.

Use decimal arithmetic with source precision; never float sums. Group currencies
separately, no exchange-rate conversion. Signed credits reduce charges only when
source semantics establish the sign. Taxes, overages and adjustments are included
exactly once, with the summary used as a reconciliation control—not appended on
top of itemized charges. Any unexplained difference is a visible reconciliation
gap. Payment/refund-of-payment events are separate evidence; never net them into
billed expense a second time. Unsupported or ambiguous adjustment semantics stay
unreconciled rather than being guessed from descriptions.

Resource identity gives direct assignment only through an explicit effective
mapping. Project/tag mappings require dated observations, never today's tags
retroactively applied to last month. A charge crossing a rule boundary is split
only with measured subperiod evidence; time proration is an explicit allocation.
Multiple dependency links never imply equal sharing. Allocations carry weights,
reason and deterministic residual rounding; weights cannot exceed 100%. Unmapped
amounts remain unassigned, deliberately common amounts remain shared. Conflicting
direct mappings are unresolved evidence. Preserve old rule evaluations and expose
restatements explicitly when correcting a mapping.

For each currency and invoice revision:
`direct + allocated + shared + unassigned = normalized billed charges`.
Also show the difference to the provider invoice total and reconciliation status.
Unknown-cost dependencies have null amounts and coverage status, not zero. A
verified complete zero invoice can show zero. Missing months, absent scopes,
late invoices and failures retain last-good evidence with its age and gap status.
A report names period, source revisions, fetched/generated times and coverage.

Use bounded HTTP calls (30 seconds/request, three attempts for transient reads,
5 minutes/import); follow pagination only on the configured provider origin,
respect rate-limit responses within that deadline. Reports never wait on network.
Partial staging is discardable; a retry starts from the source, not a presumed
successful last page. Provider errors become bounded typed outcomes.

## Access and rotation

Provide `lf auth access verify <environment>` and
`lf auth access rotate <credential> --replacement <reference>` as explicit
operations, never side effects of report generation. The first supported live
probe is the provider billing read using the selected reference. Keep permission
claims, provider-issued scope evidence and read success as separate fields. A
successful GET alone leaves read-only enforcement unverified; require trusted
provider scope evidence to label it verified. Never test denied writes against
real resources merely to prove read-only access.

Verification runs within the named environment through its designated credential
or tool path. A local invocation may verify the current environment; it cannot
certify a remote laptop by attaching that laptop's name to a local result. An
unreachable environment stays unavailable. Tool-only consumers verify report-tool
access without receiving a provider key. Observations bind the environment,
requirement revision and non-secret credential version; replacing a value under
the same Doppler secret name invalidates prior verification for the new version.
Where no trustworthy version evidence exists, report that uncertainty and require
fresh verification for a controlled replacement.

Prefer designated tools for agents: an isolated reporting process receives only
read access to normalized report data, with no Doppler or provider credentials.
A trusted importer resolves the designated secret through Doppler and performs
fixed read operations; callers cannot supply arbitrary URLs or commands. Agent
provisioning exposes only the report commands/data, not the administrative Store.
An unrestricted shell under the administrator's OS identity is not an isolation
boundary: environment scrubbing alone cannot hide credential files or sockets.
The headless provisioning demo uses an isolated container with no host home,
Doppler config, admin database, Docker socket or SSH agent mounts; only a read-only
report export. It proves tool access and absence of inherited authority. Raw
credential consumption, when needed by a laptop/service, is resolved inside the
trusted process and never printed or given to an agent transcript.

Extract the existing Doppler resolver into Infrastructure's module and add explicit
project/config/name inputs, sanitized failures, bounded subprocess output and
online verification. Avoid broad `doppler run` config injection into agents.
Doppler references remain secret authority; SQLite holds metadata only.

Rotation state: candidate recorded → candidate verified in each required
environment → active reference switched transactionally → old reference retirement
pending → retired. Verification failure leaves the old reference active. Failed
retirement leaves the candidate active and the old key explicitly still valid;
retry retirement without reactivation or a second candidate. Restart resumes from
receipts. Prevent concurrent activation through the Store transaction. Shared
credentials require verification for every known dependent environment; unknown
consumers block automatic retirement.

The Store's active-reference switch is not proof that consumers have switched.
Before retirement, each required environment must acknowledge using the candidate
through its normal consumption path, including reload/restart where necessary.
Offline or failed consumers keep retirement pending and the old credential valid.
The test provider must prove the candidate works after retirement and the old key
is rejected; a metadata state transition alone cannot satisfy the rotation demo.

This Task demonstrates creation, provisioning, verification and retirement with
synthetic test credentials through local fake provider/Doppler endpoints. Real
provider key creation/revocation is not assumed supported; operator receipts or a
provider-supported authorized action are required. Metadata retirement never
claims a key was revoked. Fixture credentials are generated at runtime, never
printed, and absent from persisted reports and logs even on error.

## Remaining implementation

One coherent delivery retains the full acceptance below; the local core is not
an inventory-only replacement milestone and is not ready to publish.

1. Expand inventory/access/report coverage: reconcile discovery relationships from
   non-secret evidence; retain last-good source evidence and dated failed-import
   observations; extend output DTO fixtures to observed-access and rotation outcomes and add
   edge tests for concurrency, mixed currencies, rule boundaries and invalid
   inventory rollback. Add dated project/tag observations and mapping resolution;
   current rules select account/resource only. Link existing Session usage evidence without converting it to invoice money.
   Extend effective dependency-link history to historical account/resource/access
   projections and dated project/tag attribution. No existing usage reader has changed.
2. Resolve the provider contract distinction above, select the initial compute
   reader and implement its bounded fetch/export normalization and fixture corpus.
   Do not claim a provider-unsupported capability merely because a reader is absent.
   Unknown account identity does not authorize inventing an account or using a key.
3. Implement provider access probes and production provisioning. Administrative
   rotation persistence and synthetic endpoint/isolated-export demonstrations now
   exist; production provider execution and authenticated receipt collection do not.
   Complete recovery when required consumers or credential metadata change during
   an active rotation. Expand tests for concurrent candidates, unknown consumers,
   stale receipts and cancelled-candidate restart. The Docker script proves only
   the designated fixture export boundary; deploy no general shell as a restricted
   reporting agent. Keep all provider/administrative authority outside consumers.
4. Complete documentation and headless acceptance, then perform the authorized
   live-period import only when the required account/reference/period or export
   inputs exist. None has been supplied. Mercury connectivity stays excluded.

### Delete — do not maintain

`lf/commands/ssh.rs::resolve_doppler_secret` has been removed; SSH now uses
the narrow shared resolver. Preserve SSH forwarding behavior and its tests.
The working tree has removed the invoice foreign key to mutable source/account
membership, serialized credential equality and nested access-command rematching.
Durable invoice references, typed equality and a shared repository/Wave predicate
retain those responsibilities without duplicate representations.
No other production deletion is indicated. Preserve native model account routing,
Session attribution and the bank budget script. Forbidden: generic provider
registries, duplicate account auth, secret caches in this Store, dual inventory
writers, inferred equal shares, zero-filled missing evidence, and a shell launcher
advertised as restricted merely because its environment was cleared.

## Done when and checks

Acceptance also demonstrates coverage of all discovered Doppler accounts, with
all seven named services represented or an explicit discovery gap. Mercury
appears in access inspection without requiring bank transaction ingestion.

Gate's new `cargo test -p loopflow --test spend_tests` suite crosses public CLI,
SQLite and both output consumers. Its main scenario starts with USD 100 direct,
USD 60 shared (explicit 50/50 allocation), USD 20 unassigned and USD -10 credit
mapped to the direct owner: USD 170 total, USD 120/30 attributed and USD 20
unassigned. A corrected direct charge of USD 90 yields USD 160 total and USD
110/30/20; repeat/reordered-page imports change neither amounts nor multiplicity.
Same-named Waves in separate repositories remain separate. A mapping update
cannot rewrite the earlier revision's report. Include duplicate identical lines,
partial pages, stale/missing periods, summary mismatch, unknown access and cost,
missing resource IDs, mixed currencies and explicit zero-source evidence.

Access tests prove successful read vs verified scope distinction, failed candidate
preservation, interrupted activation, failed retirement retry and all-consumer
verification for a shared key. Capture stdout/stderr, report serialization and
Store contents to assert runtime test secrets never appear.
Include a local successful probe that cannot verify a remote environment, a
consumer still using the old key after activation, and a tool-only consumer.
Billing checks also cover shared dependency links without invented allocations
and the same invoice arriving through API and export without double counting.

Add `uv run python scripts/check_spend_isolation.py` for the container-based
headless report consumer proof: report succeeds, admin data and inherited
credential locations are unavailable. Container availability is a capable CI
requirement; it must not be replaced by a weaker env-only assertion. Gate also
runs `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, the focused
SSH/auth suites affected by resolver extraction and existing documented-command
checks once. Test migration from the released frontier to the finished draft.

Live acceptance remains open until an authorized account, Doppler reference and
period/export are supplied: import it, trace one resource and a shared/unassigned
amount to the source, and record reconciliation/coverage without secret output.
No live connection, rotation or resource change occurred during kickoff.

Exclusions: bank connectivity, runway forecasting, detailed Session performance
analysis, general security policy, fleet-wide sandbox design, Desktop UI,
automatic provider purchases and live resource mutations. Chapter metric targets
are empty; no new KR is represented as accepted.

Check: recorded `cargo test -p loopflow --test spend_tests --test spend_rotation_tests` (10), released-frontier migration test (1), `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings` passed; realign's prose-only reconciliation passed `git diff --check`, without rerunning those checks; SSH and broader acceptance remain with gate, authorized live evidence missing.
