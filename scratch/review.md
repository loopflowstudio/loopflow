# Simulated operational review

## Findings fixed

- Equivalent local and forwarded subscription identities could make a
  target-side prefix ambiguous or let an origin route beat the local copy.
  Selection now deduplicates local-first while retaining forwarded provenance
  for explicit origin preferences and origin-owned session pins.
- Target-side selection was validated against one provider at a time, so a
  Codex-qualified preference could break a Claude launch. It now resolves
  against the complete merged catalog before applying provider-local order.
- A preferred target-local account with a known missing credential did not fall
  through to the healthy route. Missing credentials now fail over regardless of
  preference; explicit-only and cooldown policy retain their existing explicit
  semantics.
- Home identity checks happened after Clap parsing, so built-in `--version`
  could exit before proving a HomeId-addressed hop. The SSH preamble now compares
  `lf home id` with the expected identity before every requested command.
- A separate Home resident created a second boot authority and could make
  `lfd` startup a no-op. `lfd` now owns an in-process `WaveHost` and one
  reconciliation loop.
- One broken Wave could stop later siblings from starting. `WaveHost` now
  attempts every requested Wave and returns the collected failures afterward.
- The WaveHost reconciliation task could race graceful shutdown. Shutdown now
  aborts and joins that task before stopping listener children.
- The old `LF_LFD_AUTH_TOKEN` name implied request authentication even though it
  only permitted a non-loopback bind. It is now the exact policy knob
  `LF_LFD_ALLOW_NON_LOOPBACK=1`; exposed health/status routes still require a
  real network boundary.
- The new Wave control routes initially shared that exposed listener without
  authentication. They now require a generated per-process capability stored
  only in the owner-readable local endpoint record, created as `0600` before
  its contents are written.

## Evidence matrix

| Claim | Planned behavior | Implemented behavior | Proof | Result |
|---|---|---|---|---|
| Remote `lf` syntax | Target is the argument boundary; inner `lf` is implicit; nested SSH fails | Parser inserts only its internal boundary and transport always prefixes `lf` | 14 SSH unit tests; 3 SSH binary-parser tests | pass |
| Merged account authority | Target chooses local and forwarded identities with lazy origin credentials, health fallback, and authority-owned pins | Full two-provider selection catalog; local-first duplicate handling; origin broker resolves only chosen credentials | 8 account-lease tests, including merged target selection and lazy fallback | pass |
| Durable authority | Foreground authority cannot survive a detached Loopflow spawn | tmux command and environment remove lease handles, singleton tokens, named secrets, SSH context, and lfd ingress config | 9 process tests | pass |
| Machine-local Home behavior | `start` runs here; bare startup filters policy and placement; HomeId SSH proves the target | Start claims local placement, `WaveHost` filters `owner`/`home`, SSH preflight compares HomeIds | 3 machine tests; 2 WaveHost tests; Home preamble test | pass |
| One Home server | lfd directly hosts and reconciles Waves without a second resident | One Home lock, endpoint, `WaveHost`, 30-second loop, explicit stop suppression | 15 lfd unit tests; real lfd subprocess integration | pass |
| Local control security | Public probes stay read-only; Wave mutation needs local capability | Random control token in a `0600` endpoint record; non-loopback exposure requires exact opt-in | Unauthorized/authorized Wave-start test; endpoint mode integration assertion | pass |
| User documentation | Security explains trust; subscriptions explain identity and choice | Separate Security and Subscription Management pages, synchronized into the website | docs/accessibility suite and prompt golden | pass |
| Live SSH host | Run branch-built `lf ssh <target> --version` without provider use | Bounded SSH transport attempted; target was unreachable before remote execution | `cargo run -q -p loopflow --bin lf -- ssh mini-heart --version` | gap |

## Deliberate bounds

- `owner` and `home` are automatic-start policy, not authorization.
- HomeId is the stable preferred value. Loopback, hostnames, local interface
  addresses, and the current SSH destination are accepted when Loopflow can
  prove them. A NAT-only public address is not guessed from inside the guest.
- The WaveHost reconciles every 30 seconds, so newly registered eligible Waves
  and exited eligible listeners are picked up without restarting `lfd`.
- The tmux fallback started by `lf start` is a scrubbed development server. It
  hosts Waves but does not retain webhook secrets from the invoking shell;
  production webhook ingress belongs to the installed lfd service.
