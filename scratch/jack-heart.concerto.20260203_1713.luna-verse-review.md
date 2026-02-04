# lfd registration + Concerto auth review

## What was implemented
- Added lfd registration with loopflow.studio: persistent machine ID, JWT loading, registration, heartbeats, and clean deregister on shutdown.
- Added gRPC connection-token validation and auth interceptor gated by registration status.
- Added lfd config loading for auth provider/base URL plus status reporting in `lfd status`.
- Added Concerto auth primitives (AuthService/AuthState/AuthError), token provider, and URL scheme registration.
- Added tests for lfd registration and AuthService callback parsing.

## Key choices
- Use a lightweight urllib-based JSON client for registration and token validation to avoid new runtime deps.
- Gate gRPC auth on registration status so local-only usage remains unchanged.
- Cache connection-token validations with expiry to reduce round-trips.
- Store machine ID in `~/.lf/machine_id` for stable identity without hardware identifiers.

## How it fits together
lfd loads auth config + JWT, registers with loopflow.studio, and starts a heartbeat loop. Registration state is shared for status and for the gRPC interceptor, which enforces connection tokens only when registration is enabled and successful. Concerto uses ASWebAuthenticationSession to obtain a JWT, stores it in Keychain, and exposes a TokenProvider for future remote services.

## Risks and bottlenecks
- Registration/validation endpoints are assumed; contract mismatches will surface as registration failures or denied gRPC connections.
- Registration uses background heartbeat tasks; if the loopflow.studio API is unstable, status may flap until error handling is refined.
- Swift token refresh is stubbed in `AuthState.refreshTokenSilently` and still needs server contract confirmation.

## What's not included
- RemoteWaveService and wiring TokenProvider into a remote client.
- Server-side WorkOS integration and lfd JWT verification (loopflow.studio + lfd side).
- NAT traversal/relay for remote connectivity.
