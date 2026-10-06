# Computing dependencies, access and cost

LOO-389 · October 6, 2026 · Draft implementation design

Jack Heart requested a dependency inventory connecting consumers, access and
spending. The Task and Spend goal establish that outcome; the mechanisms below
are kickoff choices, not Jack's approval of an account connection or rotation.
No implementation or live financial evidence exists on this branch yet.

## Experience and demo

Jack can start with a dependency and see its repositories/Waves, accounts,
resources, credential requirements and billed costs—including unknowns. Starting
with a repository/Wave produces a spending breakdown whose amounts lead back to
provider evidence. Shared relationships do not multiply charges.

Proposed CLI (new commands, not currently installed):

```sh
lf spend dependency show build-workers --json
lf spend access show laptop --json
lf spend report --period 2026-09 --repo loopflowstudio/loopflow --wave spend --json
lf spend source import do-main --period 2026-09
```

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
modules, `store/spend.rs`, `store/sqlite/spend.rs` and `lf/commands/spend.rs`;
register the command in the existing CLI. One Task migration draft, created with
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

Inventory mutations use `lf spend inventory import <file>` with validated,
non-secret structured input, stable record IDs and transactional upserts. Reports
use read-only Store access; inspection never polls a provider or fetches secrets.
Local metadata edits confer no provider permissions. DTOs use required or explicitly
optional fields, decimal strings for money, and fixture coverage. No Desktop
consumer is added in this Task; existing usage/app DTOs stay unchanged.

## Provider choice and checked risks

Select **DigitalOcean** as the first implemented compute importer; account
ownership and availability remain unverified. This is a reversible engineering
choice, not a recommendation to buy compute or create an account.

Official documentation checked October 6, 2026:

- [Billing scopes](https://docs.digitalocean.com/reference/api/scopes/billing/read/):
  `billing:read` needs no additional scope. Use a dedicated token rather than
  `api:read` or a provisioning key.
- [Billing API](https://docs.digitalocean.com/platform/billing/reference/api/):
  invoices have UUIDs; paginated items expose amounts, duration units, periods,
  optional resource IDs and project names, but no documented stable item ID.
  Amounts are USD. Summaries include charges, taxes and adjustments. Previews and
  nightly insights are provisional; billing history also contains payments.
  Therefore import complete invoice revisions, retain line locators within each
  revision, and keep estimates/payments outside the invoice total.
- [Doppler service tokens](https://docs.doppler.com/docs/service-tokens): default
  read access is config-scoped, optional write access and expiry exist. Config
  scope is not individual-secret scope. Cached fallback secrets can remain
  available after revocation. Use dedicated minimal configs where a service
  token is necessary; verification must be online without fallback.
- [Doppler rotation](https://docs.doppler.com/docs/secrets-rotation): supported
  integration rotation is provider-specific. Do not infer universal provider-key
  creation/revocation from Doppler storage access.

A generic CSV-only ledger loses provider identity and correction semantics. AWS
exports could provide richer detail but introduce export provisioning before the
first report; that is unnecessary for this selected invoice mechanism. A separate
SaaS inventory would split ownership from Loopflow consumers. These alternatives
are declined, not maintained alongside the selected path.

## Billing ingestion and attribution

Fetch invoice list, full item pagination and summary for the selected period.
Stage normalized financial fields locally, validate page completeness and totals,
then publish one revision atomically. Omit addresses and unrelated personal data;
never retain response headers, auth payloads or raw error bodies. Source locators
and normalized evidence allow tracing without copying arbitrary provider output.

Identity is `(source, account, invoice UUID)`, not month or resource. Compute a
canonical multiset hash of normalized items plus financial summary, preserving
identical-line multiplicity. Same revision is idempotent; changed content creates
an immutable revision and atomically supersedes its predecessor. A missing line
in a complete replacement is a correction, not another charge. No attempt to
match unstable line IDs across revisions. Concurrent imports serialize publication
per invoice; incomplete pages and interrupted imports cannot replace good data.

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

Provide `lf spend access verify <environment>` and
`lf spend access rotate <credential> --replacement <reference>` as explicit
operations, never side effects of report generation. The first supported live
probe is the provider billing read using the selected reference. Keep permission
claims, provider-issued scope evidence and read success as separate fields. A
successful GET alone leaves read-only enforcement unverified; require trusted
provider scope evidence to label it verified. Never test denied writes against
real resources merely to prove read-only access.

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

This Task demonstrates creation, provisioning, verification and retirement with
synthetic test credentials through local fake provider/Doppler endpoints. Real
provider key creation/revocation is not assumed supported; operator receipts or a
provider-supported authorized action are required. Metadata retirement never
claims a key was revoked. Fixture credentials are generated at runtime, never
printed, and absent from persisted reports and logs even on error.

## Complete implementation sequence

One coherent delivery includes the following internal cuts; none replaces Task
acceptance with an inventory-only milestone.

1. **This slice:** Store model, single draft migration, inventory import,
   dependency/access inspections and billing revision/report core. Focused test:
   `cargo test -p loopflow --test spend_tests dependency_cost_round_trip`.
   Seed shared relations, unknowns, exact money and corrected invoices through
   the public CLI against a disposable Store. No live account required.
2. DigitalOcean reader and normalized invoice fixture corpus; page failure,
   correction, credits and effective attribution. Resource usage appears on
   dependency inspection; subscriptions use declared recurrence with an estimate
   label until a source charge exists. AI dependencies expose supported model/key
   evidence from existing usage readers or an explicit unsupported capability;
   this Task does not promise a second live AI billing integration.
3. Extract narrow Doppler resolution; implement access observations, recoverable
   test rotation and isolated report-tool provisioning. Cut existing SSH secret
   resolution over to the shared function and delete its private duplicate.
4. CLI text/JSON, DTO fixtures, README and architecture-reference updates;
   headless acceptance and then authorized live-period import when inputs exist.

No other production deletion is indicated. Preserve native model account routing,
Session attribution and the bank budget script. Forbidden: generic provider
registries, duplicate account auth, secret caches in this Store, dual inventory
writers, inferred equal shares, zero-filled missing evidence, and a shell launcher
advertised as restricted merely because its environment was cleared.

## Done when and checks

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

Review finding: complete invoice revision identity avoids fabricating provider
line identities; isolated tools avoid confusing an environment list with an
access boundary. Both findings are incorporated above.

Check: documentation/source inspection completed; implementation tests are deferred to implement/gate because this change is prose only.
