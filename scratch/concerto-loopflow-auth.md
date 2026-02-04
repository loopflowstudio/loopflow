# Loopflow Auth

WorkOS-based authentication for remote access. Local = no auth, remote = Loopflow account.

## Problem

Concerto needs to access lfd remotely (phone connecting to Mac, laptop to server). Phase 1 assumes local-only access via Unix socket. Phase 2 needs authenticated remote access.

Users expect:
- Sign in once with familiar OAuth (Google, GitHub)
- Stay signed in across app restarts
- Seamless token refresh without manual re-auth
- Self-hosters can control who accesses their daemon

## Approach

Use WorkOS AuthKit as the identity provider. loopflow.studio handles OAuth orchestration and issues JWTs. Clients (Concerto, lf CLI) store tokens locally. lfd validates tokens against loopflow.studio's JWKS.

```
┌─────────────┐      ┌───────────────────┐      ┌─────────────────┐
│  Concerto   │─────►│  loopflow.studio  │─────►│  WorkOS AuthKit │
│  (mobile)   │ JWT  │  (issues tokens)  │ OAuth│  (Google/GitHub)│
└─────────────┘      └───────────────────┘      └─────────────────┘
       │
       │ Bearer token in gRPC metadata
       ▼
┌─────────────────────────────────────────────────────────────────┐
│  lfd (self-hosted)                                              │
│                                                                 │
│  1. Extract token from Authorization/grpc-authorization header  │
│  2. Validate JWT signature against cached JWKS                  │
│  3. Check claims (exp, aud, iss)                                │
│  4. Check user in allowed_users (if configured)                 │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| GitHub OAuth directly | Simpler, no WorkOS dependency | No enterprise SSO, limited providers, we'd build user management ourselves |
| Firebase Auth | Good mobile SDKs | Vendor lock-in, harder for self-hosters |
| Auth0 | Feature-rich | More expensive than WorkOS at scale, heavier integration |
| Roll our own | Full control | Significant security surface area, enterprise SSO is hard |

WorkOS wins because: enterprise SSO for free (SAML, OIDC), familiar OAuth providers, clean SDK, straightforward JWT validation, and self-hosters can opt out entirely.

## Key decisions

**WorkOS AuthKit over raw OAuth.** AuthKit handles the OAuth dance, user management, and enterprise SSO. We just redirect and receive JWTs. Per the concerto wave principle of "existing tools > custom solutions."

**JWTs signed by loopflow.studio, validated by lfd.** The service issues tokens, daemons validate them. This separates identity (loopflow.studio) from authorization (lfd config). Self-hosters control `allowed_users` locally.

**Keychain storage on Apple platforms.** Tokens contain sensitive access. Keychain provides hardware-backed encryption and biometric protection. `SecAccessControlCreateFlags.biometryCurrentSet` requires Face ID/Touch ID for token access.

**Device flow for headless scenarios.** Servers and CI need `lf auth login --device` which shows a code instead of opening a browser. Same as GitHub CLI's device flow.

**Graceful degradation to local-only.** If loopflow.studio is unreachable, local connections still work (auth: local in lfd config). Remote connections fail cleanly with "authentication service unavailable."

## Scope

In scope:
- loopflow.studio auth endpoints (/auth/login, /auth/callback, /.well-known/jwks.json)
- Concerto sign-in flow (ASWebAuthenticationSession → token → Keychain)
- lf CLI auth commands (lf auth login, lf auth logout, lf auth status)
- lfd JWT validation middleware (gRPC interceptor)
- Token refresh before expiry
- `allowed_users` config for self-hosters

Out of scope:
- lfd registration with loopflow.studio (separate item: lfd-registration)
- Push notifications (separate item: push-notifications)
- Enterprise SSO configuration UI (future)
- Multi-device session management (future)
- API key fallback (Phase 2.5)

## Implementation

### loopflow.studio

New TypeScript service at loopflow.studio:

```
loopflow-studio/
├── src/
│   ├── auth/
│   │   ├── login.ts         # Redirect to WorkOS
│   │   ├── callback.ts      # Handle WorkOS callback, issue JWT
│   │   ├── jwks.ts          # Serve public keys
│   │   └── device.ts        # Device code flow
│   └── keys/
│       ├── private.pem      # RS256 signing key
│       └── public.pem       # Published via JWKS
```

JWT claims:
```json
{
  "sub": "user_abc123",
  "email": "user@example.com",
  "name": "Jane Developer",
  "iss": "https://loopflow.studio",
  "aud": "loopflow-lfd",
  "exp": 1234567890,
  "iat": 1234567890
}
```

### Concerto (Swift)

New `AuthService` in LoopflowCore:

```swift
public protocol AuthServiceProtocol: Sendable {
    func signIn() async throws -> User
    func signOut() async throws
    func refreshTokenIfNeeded() async throws -> String?
    var currentUser: User? { get async }
    var isSignedIn: Bool { get async }
}
```

Sign-in flow:
1. Present ASWebAuthenticationSession with `https://loopflow.studio/auth/login?redirect_uri=loopflow://auth/callback`
2. Receive callback with token
3. Decode JWT to get user info
4. Store token in Keychain with biometric protection
5. Update UI state

Token storage:
- Key: `com.loopflow.auth.token`
- Access: `.whenUnlockedThisDeviceOnly`
- Protection: `.biometryCurrentSet`

### lf CLI (Rust)

New auth subcommand:

```bash
lf auth login          # Open browser, save token to ~/.lf/credentials.json
lf auth login --device # Show code for headless
lf auth logout         # Clear credentials
lf auth status         # Show current user
```

Credentials file:
```json
{
  "version": 1,
  "tokens": {
    "loopflow.studio": {
      "token": "eyJ...",
      "expires_at": "2024-03-01T00:00:00Z"
    }
  }
}
```

### lfd (Rust)

Config addition to `~/.lf/lfd.yaml`:

```yaml
auth:
  provider: loopflow.studio  # or "local" for no auth
  jwks_url: https://loopflow.studio/.well-known/jwks.json
  audience: loopflow-lfd
  allowed_users:
    - user_abc123
    - user@example.com
```

gRPC interceptor validates Bearer token on all remote requests. Local Unix socket connections skip auth (OS provides identity).

### RemoteWaveService (Swift)

Wraps authenticated requests to remote lfd:

```swift
public struct RemoteWaveService: WaveServiceProtocol {
    private let authService: AuthServiceProtocol
    private let endpoint: URL

    public func listWaves(repo: URL) async throws -> [Wave] {
        let token = try await authService.refreshTokenIfNeeded()
        // Add Authorization header, make request
    }
}
```

## Done when

- [ ] `https://loopflow.studio/auth/login` redirects to WorkOS
- [ ] `https://loopflow.studio/auth/callback` issues JWT
- [ ] `https://loopflow.studio/.well-known/jwks.json` returns public keys
- [ ] Concerto shows "Sign In" button when not authenticated
- [ ] Tapping "Sign In" opens web auth flow
- [ ] Callback stores token in Keychain
- [ ] Token refresh happens automatically before expiry
- [ ] `lf auth login` opens browser and saves token
- [ ] `lf auth login --device` shows verification code
- [ ] lfd rejects requests with invalid/expired tokens
- [ ] lfd `allowed_users` restricts access to listed users
- [ ] Local connections (Unix socket) continue working without auth
