# Connections and native authentication — concept review, 2026-11-16

LOO-913. Proposed design, not implemented. Subscription pooling remains removed.
The main sync is paused mid-merge; do not publish the partially resolved tree.

Jack Heart: “We should:

- review the terms of all the included providers
- generally try to see how we can make it easier for people to log into the systems necessary to use loopflow without breaking anyones terms”

## Decision

A service/non-service split does not decide whether an authentication operation
is appropriate. Distinguish native-client sign-in, a grant issued to Loopflow,
and access for a destination runtime. Offer a single setup experience while
keeping those ownership rules separate. Do not make every supported provider
an onboarding requirement.

Proposed `lf connect` experience (command spelling is open):

    lf connect
    # Coding: Claude ready locally; Codex ready locally
    # Source: GitHub ready for this repository
    # Planning: Linear not connected (optional)
    # Secrets: Doppler ready (optional)

    lf connect linear
    # Browser shows the registered Loopflow app and requested workspace access.
    # Return: Connected workspace X for Task/project reads and writes.

    lf connect codex --machine mini
    # Run native device-code login on mini; approve locally in browser.
    # Return: Codex authentication ready on mini. No model request needed.

Normal: existing login works without another password prompt. Failure: consent
is denied, expires, or an administrator blocks the app; retain existing access,
show the provider result, and offer a provider-supported next action. Recovery:
reauthorize only the affected connection or destination; never rotate identities
because a quota is exhausted. Configuring access and spending inference are
separate operations.

## What exists

The removal branch retains `lf account` service connect/disconnect. GitHub uses
`gh auth login` and extracts its token for lf's own calls. Doppler invokes the
CLI and extracts its CLI token. OpenCode reads its auth file. Linear, Asana and
Notion use registered OAuth with localhost callbacks; client credentials come
from environment or Doppler. `ProviderAuthService` can persist a token even
while checking an existing CLI login. Remote credential installation still
sends GitHub/OpenCode credentials and a fresh Linear access token. The proposed
native-client-only destination requirement removes some convenience which is
not itself prohibited. Incoming main extracts much of this into execution and
Machine modules; inspect HEAD and MERGE_HEAD separately while sync is paused.

These are distinct from the removed account catalog/usage routing system. Do
not retain their old conceptual model just because their commands share a name.

## Provider review

Sources checked 2026-11-16. Public terms and supported flows, not a blanket
legal approval or an audit of every customer contract. Each source below is
paraphrased; no credentials were used or transferred in this review.

### GitHub

[Terms §H](https://docs.github.com/en/site-policy/github-terms/github-terms-of-service#h-api-terms)
cover API use through third-party apps and prohibit sharing tokens to exceed
rate limits. [OAuth/device authorization](https://docs.github.com/en/apps/oauth-apps/building-oauth-apps/authorizing-oauth-apps)
explicitly supports CLI apps. [App installation tokens](https://docs.github.com/en/apps/creating-github-apps/authenticating-with-a-github-app/generating-an-installation-access-token-for-a-github-app)
provide repository/permission-scoped automation.

Design: reuse supported `gh` authentication locally; evaluate an lf-owned App
for direct integration and unattended work. Do not automatically copy a broad
personal token to every rental. Delegating to gh still inherits its token's
scope; it does not by itself make access narrow. Distinguish Git push credentials
from API permissions needed for PR operations.

### Linear

[Terms §2.3](https://linear.app/terms) enforce API limits;
[AUP](https://linear.app/legal/aup) prohibits bypassing authentication/rate
limits and scraping outside published APIs. [OAuth documentation](https://linear.app/developers/oauth-2-0-authentication)
supports PKCE public clients with optional secret, rotating refresh grants, and
client-credentials app access for scheduled automation. [Personal API keys](https://linear.app/developers/graphql)
are documented for personal scripts; OAuth is recommended for apps for others.

Design: local OAuth PKCE can give ordinary users browser setup without a shared
secret. Show workspace, scopes and user/app actor. For remote work use an
explicit destination grant or authorized app mode; keep refresh-token ownership
and rotation coordinated. A short-lived access token forwarded to a Machine is
still a delegated grant, not merely setup metadata.

### Asana

[API terms](https://asana.com/terms/api-terms) require registration, user
consent, adequate security and app privacy disclosures; developer credentials
are confidential and not to be shared with third parties. [OAuth](https://developers.asana.com/docs/oauth)
is the preferred app path and requires protecting a client secret.
[PAT documentation](https://developers.asana.com/docs/personal-access-token)
recommends OAuth when an app acts on behalf of other users.

Design: registered app OAuth with an appropriate confidential component, or an
organization's own registration. Do not distribute Loopflow's app secret in a
CLI or assume each person can read company Doppler configuration. Personal
scripts using PATs are a different distribution case. Disclose that Task content
may be sent to the selected coding provider, beyond OAuth consent to Asana.

### Notion

[Current authorization docs](https://developers.notion.com/guides/authorization/overview)
distinguish internal connections, personal tokens and scoped/public OAuth
integrations. OAuth includes a workspace/page picker and does not require a
Marketplace listing to begin authorization. [Developer Terms indexed by search](https://www.notion.so/Developer-Terms-ba4131408d0844e08330da2cbb225c20)
include restrictions on soliciting end-user/bot credentials, bypassing plan
limits, and training non-personalized models on integration data. The full live
legal page could not be retrieved; the indexed copy is older, so current legal
wording remains unverified. Do not call Notion fully cleared.

Design: official OAuth page selection for a distributed integration, not a
prompt to paste arbitrary account/bot tokens. Honor page access, revocation and
data-use restrictions. Self-owned internal integrations are a separate case;
do not infer their documented existence permits soliciting tokens from others.

### Doppler

[Terms §6](https://www.doppler.com/legal/terms) authorize internal use through
provided interfaces and prohibit defeating authentication. [CLI guide](https://docs.doppler.com/docs/cli)
supports local login; [service tokens](https://docs.doppler.com/docs/service-tokens)
are scoped to config and explicitly recommended instead of CLI/personal tokens
for live environments. [Service accounts](https://docs.doppler.com/docs/service-accounts)
separate automation from a person's identity.

Design: use `doppler login`/`doppler run` locally without maintaining another
copy of the broad CLI token when lf does not need it. Remote provisioning should
use a chosen config/service grant. Doppler is optional secret delivery, not a
prerequisite for every person's OAuth onboarding.

### OpenCode / hosted inference

[OpenCode Models docs](https://opencode.ai/docs/console/models/)
explicitly allow API keys with OpenCode or another coding agent.
[Hosted-service terms](https://opencode.ai/legal/terms-of-service), effective
Nov 8, distinguish local open source from hosted services, prohibit bulk or
multi-account limit evasion, and leave underlying model access/data conditions
applicable. Some broad automation wording is awkward alongside their API docs;
do not claim all unattended use is settled without plan-specific review.

Design: separate OpenCode the local harness, hosted Models/API, Go subscription,
and third-party provider logins. A single OpenCode credential row or generic
copy/export is not a policy decision for all four. Retain native auth for the
harness; explicit supported hosted API grants may belong to lf's integrations.

### Claude / Anthropic

[Current Claude Code legal docs](https://code.claude.com/docs/en/legal-and-compliance)
allow hosting an unmodified binary subject to the Commercial Terms and listed
conditions, including users authenticating with their own credentials and no
usage intermediation. Ordinary individual usage is the stated assumption for
subscription limits. [Authentication docs](https://code.claude.com/docs/en/authentication#generate-a-long-lived-token)
document `claude setup-token` for CI/scripts, with native token environment
support. This is narrower than permission for a generic subscription proxy.

Design: help launch native login on the chosen Machine and support documented
headless setup. No LF account pool, subscription credential harvesting, usage
resale, or automatic cross-account quota recovery. Validate token handling for
our deployment model rather than treating setup-token as blanket permission.
The current legal page differs from wording recorded on Nov 15; use today's
source rather than repeating the earlier broad token-handling claim.

### Codex / OpenAI

[Official auth documentation](https://learn.chatgpt.com/docs/auth)
recommends native device-code login for headless runtimes and documents copying
auth.json over SSH as fallback. [SIWC terms §3.1](https://openai.com/policies/sign-in-with-chatgpt-terms/),
effective Nov 4, prohibit quota-evading account rotation/pooling and permit only
specified token handling; remote runtime use must be under that user's control.
SIWC and launching native Codex are distinct integration surfaces; do not apply
one provision as proof for the other.

Design: prefer native destination login, browser/device assistance and explicitly
authorized background work. Removing catalog-driven transfer need not forbid
helping users through documented remote setup. API-key deployments stay distinct
from ChatGPT subscription access. Do not recreate pooling through a new command.

## Proposed model and API consequences

Person-facing concept: connection, with provider, purpose, identity/workspace,
permissions, destination and observed readiness. Show only what the official
client/provider can establish; signed-in is not a promise of remaining quota.

Native-client sign-in remains provider-owned. A grant issued to the Loopflow
integration is lf-owned and may legitimately require encrypted token storage,
renewal and revocation. A destination grant records where it was authorized to
run, its scope/lifetime and who renews it. Prefer provider-issued restricted,
short-lived grants over broad credential copying, where supported. No universal
transport protocol can manufacture restrictions a provider does not offer.

Proposed operations: list observed connection state, begin authorization for a
purpose/destination, check completion, disconnect the selected LF grant. Current
`account.list/connect/disconnect` may migrate behind this experience; names are
not decided. Disconnecting lf's use must not silently log a person out of a
native tool used independently. Scope/session work carries grant references,
not token values. Browser profile preference is optional presentation; it never
substitutes for provider consent or expected workspace selection.

Do not add an account manager with ranking/rotation under the name connection.
Do not require login to all eight providers: one coding client is sufficient for
local use; source/planning/secrets connect when the feature needs them.

## Open choices

1. Local-first with optional hosted OAuth helper, entirely local with organization
registrations where needed, or hosted OAuth by default? Asked in conversation;
not decided. Linear PKCE can remain local; Asana/Notion confidential-client
requirements need their own supported architecture.
2. In this removal PR: replace blanket remote-login prohibition with accurate
provider-native guidance; defer the unified connection UX and destination-grant
model to follow-up, or expand this PR? Proposed: corrected guidance now, broader
UX later. No code rewrite authorized by concept-review alone.
3. Notion live Developer Terms and OpenCode hosted-plan automation conditions
need confirmation before claiming policy coverage. No provider contacted and no
private agreements reviewed.
