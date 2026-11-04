# QR-first Machine connection

Jack Heart selected **QR code as the default** after the automatic-discovery
investigation on November 4, 2026. Entering or pasting the Machine address is the
fallback. Automatic discovery and the announcement experiment are deferred.

## Accepted connection flow

1. Sign into installed Tailscale on the Machine and phone.
2. Enable mobile access in Desktop's Mobile settings; show its HTTPS address and QR.
3. In iOS, make **Scan QR code** the primary action and **Enter address** secondary.
4. Decode the address, show the destination, and connect after confirmation.
5. Remember the Machine for later connections; scanning is initial setup, not a
   requirement on every launch.

The QR contains the HTTPS address only, with no ticket or credential. Tailscale
provides network connectivity and caller identity; the Machine still enforces
which identity is admitted. No extra phone enrollment, embedded node, custom
relay or required hub. The working cutover now implements this flow. It is not delivered yet; the
verification status and installed-host constraints are recorded below.

The earlier investigation is retained below as evidence, not active work.

## Finding

No supported API was found that lets Loopflow read the installed Tailscale iOS
app's peer list directly. That is a research finding, not proof that such an API
cannot exist. The most promising experiment under the chosen constraints reverses
discovery: Machines know their peers and announce their own address to the phone.
**This has not passed a physical-device test and is not an accepted architecture.**

## Paths investigated

| Path | Evidence and consequence |
| --- | --- |
| Installed iOS client API | Apple's [NETunnelProviderManager](https://developer.apple.com/documentation/networkextension/netunnelprovidermanager) exposes VPN configurations created by the calling app. It does not give Loopflow access to Tailscale's extension. No separate supported peer-list API was established. |
| Shortcuts | Tailscale's [Find Devices](https://tailscale.com/docs/features/mac-ios-shortcuts) action returns devices with name and online filters. This is a supported Shortcut route, not evidence for an automatic in-app picker. |
| MagicDNS | [DNS documentation](https://tailscale.com/docs/reference/dns-in-tailscale) describes automatic names but disallows arbitrary records. Source inspection found known-name resolution, not a general service directory. Guessing hostnames is not discovery. |
| Bonjour | [Multicast support remains requested](https://github.com/tailscale/tailscale/issues/1013). Same-LAN discovery cannot establish remote tailnet discovery. Wide-area DNS-SD requires additional DNS setup. |
| Cloud device API | [OAuth clients](https://tailscale.com/docs/features/oauth-clients) require configured credentials. New [OAuth apps](https://tailscale.com/docs/features/oauth-apps) support user authorization, but require admin registration and restrict users to the app's tailnet. Neither simply inherits the phone's existing sign-in. |
| Tailscale Services | A [named service](https://tailscale.com/docs/features/tailscale-services) could provide a predictable entry point, but needs service configuration and host approval or an auto-approval policy. Multiple hosts behind one service are routing alternatives, not automatically a Machine picker. Worth revisiting for managed installations; not zero setup for arbitrary personal tailnets. |
| Quad100 / Taildrive | Source inspection found no general peer-directory endpoint suitable here. Taildrive's remote list depends on sharing capability and access. Do not repurpose it as Loopflow discovery. |
| Embedded Tailscale | Supplies our own node and its network map, but changes the accepted host-Tailscale architecture. Not proposed for this experiment. |

Source inspection used official tailscale/tailscale commit
`f851ca27e42dcd1bc32cf48231837b7f92d2775b`:
[DNS resolver](https://github.com/tailscale/tailscale/blob/f851ca27e42dcd1bc32cf48231837b7f92d2775b/net/dns/resolver/tsdns.go),
[Taildrive](https://github.com/tailscale/tailscale/blob/f851ca27e42dcd1bc32cf48231837b7f92d2775b/ipn/ipnlocal/drive.go),
[network stack](https://github.com/tailscale/tailscale/blob/f851ca27e42dcd1bc32cf48231837b7f92d2775b/wgengine/netstack/netstack.go),
[PeerAPI listeners](https://github.com/tailscale/tailscale/blob/f851ca27e42dcd1bc32cf48231837b7f92d2775b/ipn/ipnlocal/peerapi.go).
The source distinguishes iOS from platforms with netstack fallback; netstack
behavior alone cannot prove the installed iOS app's behavior.

## Candidate: Machine announcements

```mermaid
sequenceDiagram
    participant H as Machine service
    participant T as Host Tailscale
    participant P as Foreground iOS app
    H->>T: Read online same-owner peers
    H-->>P: Unicast address offer over tailnet
    Note over P: Candidate only; no authorization
    P->>H: Verify HTTPS and Loopflow protocol
    Note over P: Show reachable Machine for selection
```

The scratch experiment sends bounded UDP offers to port 47471. It sends no
credentials. The receiver displays only syntactically valid HTTPS `.ts.net`
addresses and never connects to them. It listens while foregrounded and stops
when the app leaves the foreground. The sender selects online, same-owner,
untagged iOS peers through the installed host CLI.

Before production:

- Prove packets reach an ordinary foreground app through installed iOS Tailscale
  on Wi-Fi and cellular, including VPN reconnect and app resume.
- [Incoming connections and directional ACLs](https://tailscale.com/docs/features/client/manage-preferences)
  must permit Machine-to-phone traffic. Phone-to-Machine access alone is insufficient.
- Measure discovery latency and battery cost. The host cannot currently know when
  Loopflow is foregrounded; frequent advertisements could keep tunnels active.
- Verify server origin and identity before admission. A `.ts.net` suffix or an
  advertisement is not proof of ownership. The phone lacks the host's peer identity
  API. Bound connections, deduplicate, expire candidates, reject redirects and
  consider source-address matching before automatic probing.
- Test multiple Machines, tagged servers, different owners, tailnet changes,
  IPv6 and offline peers. The current sender is intentionally IPv4/same-owner only.
- Decide whether these costs justify the automatic path. Do not add this protocol
  merely because it is technically possible.

## Evidence from this checkout, 2026-11-04

- Native `NWListener` probe compiled for macOS, the iOS Simulator SDK and the
  physical iOS SDK. These are compile checks, not installed-app tests.
- macOS loopback delivered a valid address offer to the native listener.
- Sending to this Mac's own Tailscale IPv4 address timed out. Cause unresolved;
  this is neither a successful tailnet test nor proof that remote iOS delivery fails.
- No online same-owner iOS peers were available for the cross-device test.
- The reachable iPad had Developer Mode enabled, but no mounted developer disk
  image and no matching device entry in the available wildcard development profile.
  Xcode's component update failed because the OS installer required authorization.
- No physical-device acceptance, automatic Machine selection, HTTPS verification
  or battery measurement has passed. Simulator networking would not substitute
  for that evidence.

If automatic discovery is revisited, the next decisive experiment is a provisioned physical iPhone/iPad signed into
Tailscale, running the probe in the foreground, with announcements from a separate
Machine. Repeat with a second Machine and a different network before integration.
Run instructions live in [tailscale-discovery/README.md](tailscale-discovery/README.md).

## Authorization implementation

Rex documents [address-only QR](https://www.superlogical.com/rex/docs/reference/cli#rex-server-tailscale-pair)
and [direct tailnet listeners](https://www.superlogical.com/rex/docs/linux/remote-access).
The working implementation replaces phone enrollment with host `whois --json`
admission of distinct, untagged, same-owner devices. It keeps HTTPS/WSS, retrieves
the host certificate into memory, and refreshes it periodically. The approved
node/user persist across service restart. Logout closes remote streams; restoring
the same account retries the listener. A different account requires explicit
re-enablement. Local Desktop/CLI retain their private Unix admission.

The earlier private Serve-socket patch was applied, tested, then replaced after
its live setup failed. It is not a second supported implementation. Current
source is the work to review; historical patch contents are not instructions.

## Installed-host compatibility finding — November 4

The address-only Swift flow and private Serve-socket cutover are working changes,
not delivered functionality. Unit and lifecycle checks passed; live setup failed:
the installed macOS Tailscale 1.94.1 rejects `unix:` targets even though its help
advertises them. No existing Serve route was changed. QR and manual address UI
remain the accepted product flow.

The direct listener has replaced that carrier in the working tree.
The installed client successfully resolves caller identity with `whois --json`,
binds its IPv4/IPv6 tailnet addresses, and returns its HTTPS certificate in memory.
Writing the certificate from the sandboxed CLI into an arbitrary temporary path
fails; capturing its output privately avoids that filesystem dependency. A local
TCP round trip worked over IPv4; same-host IPv6 timed out. This is not a remote
phone test. Rex documents direct tailnet listeners, but uses HTTP over WireGuard;
Loopflow's accepted HTTPS address requires its own TLS endpoint and renewal.

Required before this cutover is complete: preserve owner admission across service
restart and account changes; authenticate from the actual socket peer, never
caller-supplied headers; prove native connection and retained Session behavior;
remove the superseded Serve implementation instead of keeping two carrier modes.

Current proof: native QR-first, camera fallback/address validation and Keychain
checks pass; seven focused Rust unit checks and the real-TLS lifecycle test pass.
The production listener also served its actual host certificate successfully to
a validating HTTPS client and refused same-host identity with HTTP 403. No
existing Serve routes or installed Tailscale settings changed. Full native Session acceptance now passes with an explicit phone-identity fixture:
32 MiB output, 512 edited acknowledgements, passive restart recovery and Task
creation. Cross-device/physical network proof remains separate.
