# Open questions / assumptions

- `lfd install` onboarding currently caps auth polling to 5 minutes per provider (`expires_in` if shorter). If flows should wait the full provider TTL by default, we should adjust timeout behavior.
- Onboarding uses local/session token auth (`LFD_TOKEN`, `LFD_AUTH_TOKEN`, or `~/.lf/session-token`) to call `GET/POST /v0/auth/*`. Container installs without a host-visible token may need a follow-up auth-path tweak.
