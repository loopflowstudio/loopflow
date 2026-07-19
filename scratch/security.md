# SSH authority and Home composition

> "I don't want the smallest change, I want the simplest overall system that
> meets all the requirements."

> "We should fundamentally think of lf as checking both its forwarded and its
> local accounts when thinking about the accounts it has access to."

## Intent

Make remote `lf` feel like local `lf`. SSH adds origin authority to the target
machine for one foreground invocation; it does not replace authority already
installed there. Home-aware orchestration remains a layer above machine-local
commands.

```bash
lf ssh my-company task pursue
lf ssh --account jack@personal my-company task pursue
lf ssh my-company --account jack@company task pursue

lf start shipper                    # start it on this machine
lf ssh <shipper-home> start shipper # explicitly start it there
lf home start shipper               # future Home layer: route to its listed Home
```

`lf ssh` runs only a remote `lf`, so the target is the argument boundary:
SSH options and origin account preferences precede it; everything after it is
ordinary remote `lf` syntax. The remote executable and `--` separator are
implicit. Nested `lf ssh` is rejected.

## Authority model

GitHub, Linear, and OpenCode each contribute one credential to an invocation.
Foreground `lf ssh` forwards them automatically. Doppler remains explicit by
secret name, and SSH-agent forwarding remains explicit.

Claude and Codex subscription accounts are different: each machine can have
several, routing chooses among them, and provider sessions stay pinned to the
account that created them. A remote `lf` therefore selects from the union of:

1. accounts installed on the target machine; and
2. accounts made available by the origin's foreground broker.

The forwarded view must preserve two distinct facts:

- **available:** which origin accounts the target may ask the broker to use;
- **selection:** the origin repository route plus preferences or restrictions
  stated on the outer invocation.

No account is chosen merely because SSH starts. The target `lf` chooses when it
launches Claude or Codex. A local candidate launches through its provider home;
a forwarded candidate asks the broker for one access token. Account homes and
refresh credentials never cross SSH.

The broker advertises account facts eagerly but resolves credentials lazily.
Forwarding every viable account must not refresh every OAuth token during SSH
setup. Only the account selected for a provider launch is refreshed and served;
if it is unavailable, selection continues to the next candidate.

```rust
struct ForwardedAccounts {
    available: Vec<ForwardedAccount>,
    selection: AccountSelection,
}

struct AccountCandidate {
    provider: Provider,
    account_id: ProviderAccountId,
    login: EmailAddress,
    authority: AccountAuthority,
}

enum AccountAuthority {
    Local { home: PathBuf },
    Forwarded { client: AccountLeaseClient },
}
```

Selection, health, session pinning, and diagnostics operate over
`Vec<AccountCandidate>`. Effects return to the owning authority: local health
and pins go to the target store; forwarded health and pins go through the
broker to the origin store.

`lf auth accounts`, `lf route show`, and `lf usage` show the merged view with a
`local` or `forwarded` provenance label. Authentication and account mutation
remain local operations; forwarded catalog entries are read-only capabilities.

## Lifetime model

Borrowed authority belongs to the foreground SSH process tree. Same-user code
on the target can use or retain singleton environment credentials while they
are present. Forwarded subscription access ends when the broker closes.

Processes that outlive the invocation cannot inherit borrowed authority. The
durable spawn boundary—not an SSH flag—must scrub forwarded singleton values
and account handles, then use credentials installed on that machine. This is
what lets an explicit remote command work without a public `--remote-native`:

```bash
lf ssh <shipper-home> start shipper
```

The controlling command is foreground; the resident it starts is native to
that machine.

## Home layer

The base system is single-machine:

- `lf start shipper` starts `shipper` here;
- `lf status shipper` reads here;
- `lf ssh <target> start shipper` runs that same local primitive there.

Home-aware commands compose those primitives. A future `lf home start shipper`
can read placement, resolve the Home's SSH route, verify its `HomeId`, and invoke
the remote machine's ordinary `lf start shipper`. The Home layer does not alter
the semantics of `start` itself.

## Required failures

- Reject nested `lf ssh` for every SSH invocation, not only when a provider
  lease happens to exist.
- Reject arbitrary remote programs; ordinary `ssh` owns them.
- Reject combinations whose outer and inner account restrictions cannot be
  satisfied together.
- Reject any durable spawn that would retain borrowed authority.
- Fail closed when the account broker, Home identity proof, or remote `lf`
  compatibility probe fails.

## Demo

On a credential-free SSH target, run an agent with a subscription account that
exists only on the origin:

```bash
lf ssh --only-account jack@personal my-company : "report the current branch"
```

The remote `lf` lists the forwarded account as available, launches the provider
through it, and leaves no account home or refresh credential on the target.
Add a different account on the target and show that an inner selector can pick
it while the forwarded account remains visible.
