# Open questions — remote-lfd-connection

`review-design` ran headless: no human to reshape the kickoff doc live. The
design's code claims were verified against the codebase (almost all CONFIRMED —
the kickoff researched honestly). Reshaping narrowed over-built parts and made
reversible executive calls. The genuine human-judgment forks, with the default
taken so implementation isn't blocked:

1. **Token TTL: fixed 90-day (taken) vs sliding.** Sliding = a DB write per
   connected phone per minute (60s WS revalidation loop, `ws.rs:82-104`;
   `validate()` never bumps `expires_at` today). Fixed long TTL already meets
   the "connected tomorrow" bar. Overrule only if "never re-pair, ever" is the
   real intent — and then make the bump cheap (only when remaining life < half
   TTL), don't write every tick.

2. **`token_kind` audit column: deferred (taken).** Revoke-by-prefix and
   `--all` already cover pairing tokens correctly. The column only buys audit
   *visibility* (studio-pool vs phone-pairing) at the cost of a real schema
   migration. Add only if audit distinctness is a stated need.

3. **`lf op pair` host resolution: refuse, no LAN fallback (taken).** Order:
   `--host` → `tailscale ip -4` (must be in `100.64.0.0/10`) → hard fail with
   an actionable message. A LAN address that dies when you leave the network
   is the exact failure the wave exists to prevent. Overrule only if a
   loud-warned local-only LAN mode is explicitly wanted.

4. **Reverse-proxy cert fingerprint: operator supplies (taken).** lfd serves
   no TLS; `lf op pair` runs on the lfd host and can't introspect a separate
   proxy. Default: `--fingerprint <sha256>`, with `--tls-url <url>` as a
   fetch-and-hash convenience. Absent → `fp` omitted → phone TOFU.

5. **Deep-link scheme: `loopflow://pair` (taken).** Reuses the already-
   registered `loopflow` scheme (`Concerto/Info.plist:21-30`); no second
   scheme to register. No `onOpenURL` handler exists for any scheme yet, so
   the handler is new work regardless. Flagged because kickoff assumed
   `lfd://`.

None of these block implementation. Defaults are encoded in
`scratch/Mobile-remote-lfd-connection.md` (§ Open questions, Key decisions,
Scope). A later interactive session can overrule any of them cheaply.
