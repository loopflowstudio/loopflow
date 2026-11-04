# Trust and automatic host Tailscale setup

Research for Jack Heart · 2026-11-03 · Proposed behavior; no host configuration changed

**Recommendation:** prototype HTTPS/WSS through host Tailscale Serve, with one QR scan issuing a revocable application credential for the specific Loopflow Machine. This offers the most consistent path across ordinary Mac Tailscale installations and Linux. Compare SSH in the same prototype before committing: existing Loopflow command routing makes it a credible alternative, but ordinary SSH and Tailscale SSH have materially different setup requirements.

Host-installed Tailscale, native Swift and no custom relay are accepted direction. HTTPS, the exact pairing protocol and service installation details remain proposals. An optional future hub participates in neither discovery nor authentication for this first connection.

## Four identities, four different jobs

| Identity | Answers | Does not establish |
|---|---|---|
| Tailscale node | Which network device is connected? | Which OS account or Loopflow store owns the work |
| Tailscale user or tags | Which network principal made this connection? | Permission to operate every service on that host |
| Loopflow Machine | Which OS user/data directory owns the records and execution? | Permission for a newly discovered phone |
| Paired application credential | Which client may act on this Machine? | Reachability through tailnet policy |

Current [Machine identity](../docs/architecture/machines.md) already separates addresses from authority. Preserve it across DNS changes; never silently substitute another Machine because its host acquired the old name. Multiple OS users on one host need separate Loopflow service endpoints and credential stores.

Tailscale grants combine network and application permissions; policy remains deny-by-default, although an installed default policy can grant broad access. Reachability is therefore a prerequisite, not a sufficient application permission. Loopflow should neither rewrite an organization's ACLs nor assume a signed-in device can reach every peer. [Grants](https://tailscale.com/docs/features/access-control/grants)

Serve removes incoming Tailscale identity headers and injects the authenticated caller's user information. Tagged callers have no user headers; shared external users can have them. App capability forwarding supports users and tagged devices from Tailscale 1.92. Binding the backend to localhost prevents remote header forgery, but other local processes can still call it. These are proxy assertions, not OS-account isolation. [Serve identity and capability headers](https://tailscale.com/docs/features/tailscale-serve)

**Proposed consequence:** require a Loopflow credential for all privileged remote operations, including requests arriving directly at the loopback backend. Use Tailscale assertions for optional policy and audit context rather than equating a display name with permission. Keep the server in its owning OS account; root and processes with access to that account's secrets remain inside its trust boundary. A tagged server is normal; a tagged client should be represented explicitly, not assigned an invented person.

## One scan, with an honest trust boundary

Proposed sequence:

1. “Enable phone access” starts or locates the owning Machine service and performs host readiness checks.
2. The service creates a random, short-lived, single-use pairing ticket. The QR encodes the HTTPS endpoint, expected Machine ID, protocol version and ticket. It contains no long-lived credential or Tailscale administrative key.
3. Loopflow iOS scans the code, validates HTTPS normally and exchanges the ticket with that exact endpoint. The scan itself is the authorization gesture; avoid a second routine confirmation.
4. The Machine atomically consumes the ticket and issues a separately revocable phone credential. Store its verifier on the host and the credential in the phone's Keychain. Show the paired device in host settings.
5. The first authenticated response checks the expected Machine ID and supported API capabilities, then lists actual Sessions. Save the association and reconnect automatically.

The ticket is a temporary bearer secret: someone who photographs it before use can race the phone. Expiry, atomic one-use consumption and a visible paired-device record bound that risk. Parse QR payloads inside Loopflow; do not put long-lived secrets into browser URLs, logs or shared clipboard history. Explicitly define recovery for a consumed ticket whose response was lost—show a fresh code rather than silently issuing another credential.

Use normal hostname validation and system trust for Serve certificates. Do not pin the short-lived leaf certificate and turn renewal into a pairing failure. HTTPS certificate names appear in public Certificate Transparency logs, even when the service itself is private; this belongs in first-time HTTPS setup information. [Tailscale HTTPS](https://tailscale.com/docs/how-to/set-up-https-certificates)

Revoking a phone should invalidate its credentials and close its active streams. Forgetting a host on the phone only deletes local access material; it is not remote revocation. Removing a Tailscale device is a separate network-level response. Credential rotation, expiration and backup behavior need explicit tests; a hidden 90-day expiry copied from old lfd would recreate surprise disconnects. The old pairing implementation is historical evidence, not a ready current subsystem. [Prior pairing attempt](lfd-history.md)

## What “automatic” can accomplish

The host can detect an installation, query backend status, identify its DNS name, configure its own proxy endpoint, launch the service and report progress. It cannot silently grant VPN consent, complete SSO, obtain missing organization permissions, wake a sleeping computer or prove that a phone on another network can reach it.

Use the existing executable or the Mac application bundle's CLI, with `TAILSCALE_BE_CLI=1` when needed. `status --json` is useful but explicitly has a changeable format. Inspect only necessary fields through a version-tested adapter. Linux may need a privileged setup step or an existing operator account to configure tailscaled; a non-root operator grant is broader than merely reading status. iOS has no Tailscale CLI. [CLI reference](https://tailscale.com/docs/reference/tailscale-cli?tab=macos)

The Mac App Store, Standalone and open-source variants all offer a CLI, but have different lifecycles. Tailscale's current table says neither GUI variant runs before login; open-source tailscaled does. A Machine intended to return after reboot before anyone logs in therefore needs a distinct unattended installation choice, plus an appropriate Loopflow process/credential lifecycle. Do not automatically replace the person's existing Tailscale variant. [macOS variants](https://tailscale.com/docs/concepts/macos-variants)

For Serve, read existing configuration first. Prefer a dedicated, unused HTTPS port for this Machine, persist it, and verify that it points to the intended backend. Background Serve configuration survives Tailscale restart. Disable only the recorded route if it still matches Loopflow's installation; never call global reset. JSON status currently differs from text status for Services, so a future implementation using Tailscale Services must inspect their configuration separately. [Serve CLI](https://tailscale.com/docs/reference/tailscale-cli/serve)

The dedicated port matters: Serve/Funnel privacy is determined for the entire port by its most recent configuration. Adding `/loopflow` to an arbitrary existing port 443 can interfere with a public service; a later Funnel change can expose the whole listener. Select a private listener without changing unrelated routes, then verify it. A non-Funnel-supported port is useful today, but is not a permanent security contract. [Funnel port behavior](https://tailscale.com/docs/features/tailscale-funnel)

When HTTPS needs consent, follow Tailscale's supplied setup URL and resume after completion. Port forwarding is supported by the Mac app variants; their sandbox restrictions concern file serving. Do not require open-source tailscaled merely to proxy a local HTTP service. [Serve prerequisites and limitations](https://tailscale.com/docs/features/tailscale-serve)

Expose readiness as distinct states: Tailscale unavailable, sign-in/consent required, host connected, service ready, phone authenticated. An old client should receive a concrete update requirement; an unsupported API should negotiate or explain incompatibility before terminal attachment. A timeout alone cannot distinguish ACL rejection, offline VPN, sleeping host or a routing failure.

## Phone discovery and SSH comparison

Tailscale publishes iOS Shortcuts actions for Find Devices, Ping Devices, Get Status and Connect. That disproves a blanket claim that iOS has no discovery integration. It does not establish a supported in-process API allowing Loopflow to enumerate another app's peers without a Shortcut. Treat Shortcuts as an optional setup accelerator to test; the QR remains a reliable endpoint handoff. Peer presence also does not advertise a Loopflow service or grant pairing. [Tailscale Shortcuts](https://tailscale.com/docs/features/mac-ios-shortcuts)

| Transport | Strongest advantage | Setup and product cost |
|---|---|---|
| Ordinary SSH over host Tailscale | Existing Loopflow routing; OS-account authentication; no new HTTP listener required | SSH server, keys and host-key verification still need setup; mobile must implement command and stream transport |
| Tailscale SSH | Uses tailnet identity and policy without separate client SSH keys | Server supported on Linux and open-source macOS tailscaled only; SSH policy may require administrator changes |
| HTTPS/WSS through Serve | Uniform app networking, automatic certificates, typed subscriptions and clear per-phone pairing | New API frontend and application credential lifecycle; shared host proxy configuration requires care |

Tailscale SSH requires both network access to port 22 and SSH authorization to the destination account. Check mode can require browser reauthentication. Its host-key distribution does not prove a third-party iOS SSH library automatically consumes those keys; verify or use the QR to carry the expected host key. [Tailscale SSH](https://tailscale.com/docs/features/tailscale-ssh)

**Inference:** HTTPS offers the smoother general first-run experience for existing Mac app users. SSH could still win for an initially technical audience with working SSH already. Measure the same Session list/attach/reconnect scenario; neither transport solves command idempotency, process lifetime or mobile terminal input by itself.

## Smallest proofs before promising zero configuration

Use clean Mac App Store, Standalone, Linux and unattended-Mac installations. Test: signed out versus signed in; HTTPS disabled; restricted ACL; existing Serve/Funnel routes; two OS accounts; tagged/shared callers; expired and replayed QR; forged localhost headers without a phone credential; active-connection revocation; certificate renewal; API mismatch; logout/reboot and VPN account switch. Confirm an unrelated route is unchanged after enable/disable.

Finally scan from a real iPhone on cellular, not just the host itself. Repeat after foreground return and network change. **Unverified:** all these end-to-end behaviors, Shortcut-to-Loopflow ergonomics and third-party SSH host-key integration. No installation, network mutation or device trial was performed for this report.
