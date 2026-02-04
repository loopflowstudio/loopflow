# Loopflow Auth (Concerto Client)

Swift client authentication for remote lfd access. Sign in via loopflow.studio (GitHub, Google, or Apple), store tokens in Keychain, auto-refresh.

## Problem

Concerto currently only works locally—it connects to lfd on `127.0.0.1`. For Phase 3 mobile access, users need to authenticate to reach their Mac's lfd from their phone.

The server-side auth (loopflow.studio + lfd JWT validation) is designed in `roadmap/rust/05-auth.md`. This doc covers the **client side**: how Concerto authenticates and manages tokens.

## Approach

Use `ASWebAuthenticationSession` to open loopflow.studio's auth flow. loopflow.studio uses WorkOS to handle multiple OAuth providers (GitHub, Google, Apple) behind a uniform API. The client receives a JWT regardless of which provider the user chose.

```
┌─────────────────────┐     ┌────────────────────┐     ┌─────────────────────┐
│   Concerto (Swift)  │     │  loopflow.studio   │     │       WorkOS        │
│                     │     │                    │     │                     │
│  ASWebAuth ─────────┼────▶│  /auth/login       │────▶│  AuthKit hosted UI  │
│                     │     │                    │     │  ┌───────────────┐  │
│                     │     │                    │     │  │ ○ GitHub      │  │
│                     │     │                    │     │  │ ○ Google      │  │
│                     │◀────│  JWT callback      │◀────│  │ ○ Apple       │  │
│                     │     │                    │     │  └───────────────┘  │
│  Store in Keychain  │     │  (normalizes user) │     │                     │
└─────────────────────┘     └────────────────────┘     └─────────────────────┘
         │
         │ Authorization: Bearer <JWT>
         ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│  Remote lfd (user's Mac)                                                    │
│                                                                             │
│  Validates JWT against loopflow.studio JWKS                                 │
│  Checks user in allowed_users config                                        │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

## Why WorkOS

| Approach | Tradeoff | Decision |
|----------|----------|----------|
| Single provider (GitHub only) | Simpler | Excludes non-GitHub devs, fails App Store guideline 4.8 |
| Roll our own multi-provider | Full control | OAuth is tricky, maintenance burden, security risk |
| WorkOS User Management | Adds dependency | Handles GitHub/Google/Apple uniformly, enterprise SSO ready |

WorkOS wins: client stays simple (one code path), App Store compliant (Sign in with Apple), enterprise-ready (SSO supported), and we don't maintain OAuth plumbing.

## Key decisions

### 1. ASWebAuthenticationSession, not WKWebView

Apple's `ASWebAuthenticationSession` is the sanctioned approach for OAuth on macOS/iOS. It:
- Shares cookies with Safari (SSO if user is already logged into their provider)
- Shows the domain to user (trust signal)
- Handles callback URL registration automatically
- Works on both macOS and iOS with same API

WKWebView would require manual cookie handling and doesn't benefit from existing provider sessions.

### 2. Keychain for token storage, not UserDefaults

JWTs are bearer tokens—anyone with the token has access. Keychain provides:
- Hardware-backed encryption on Apple Silicon
- Access control (require user presence, biometrics)
- Secure across app reinstalls (optional)
- Same API on macOS and iOS

UserDefaults is plaintext. File-based storage requires manual encryption.

### 3. JWT with 7-day expiry, silent refresh at 24h remaining

loopflow.studio issues JWTs with 7-day expiry (per `roadmap/rust/05-auth.md`). Client-side:
- Check expiry on every authenticated request
- If <24h remaining, attempt silent refresh in background
- If refresh fails, continue with current token until truly expired
- On expiry, prompt user to re-authenticate

This balances security (tokens expire) with UX (no random logouts).

### 4. AuthState as @Observable, not Combine

SwiftUI's `@Observable` macro (iOS 17+/macOS 14+) is the modern approach. Simpler than Combine publishers, better integration with SwiftUI lifecycle.

```swift
@Observable
final class AuthState {
    var isAuthenticated: Bool { token != nil && !isExpired }
    private(set) var token: String?
    private(set) var user: User?
    private(set) var expiresAt: Date?

    var isExpired: Bool {
        guard let exp = expiresAt else { return true }
        return Date() >= exp
    }
}
```

### 5. TokenProvider injection for WaveService

`RemoteWaveService` (Phase 3) needs auth tokens. Rather than coupling auth into the service, inject a token provider:

```swift
protocol TokenProvider: Sendable {
    func token() async throws -> String
}

// LocalWaveService doesn't need tokens
struct NoAuthProvider: TokenProvider {
    func token() async throws -> String { "" }
}

// RemoteWaveService uses real tokens
final class KeychainTokenProvider: TokenProvider {
    func token() async throws -> String {
        // Read from Keychain, refresh if needed
    }
}
```

This keeps `LocalWaveService` unchanged and enables testing with mock providers.

## Scope

**In scope:**
- `AuthService` with sign-in/sign-out methods
- `ASWebAuthenticationSession` OAuth flow (provider-agnostic)
- Keychain storage for JWT
- Token refresh logic
- `AuthState` observable for UI
- `TokenProvider` protocol for service injection
- URL scheme registration for callback

**Out of scope:**
- loopflow.studio WorkOS integration (server-side, see `roadmap/rust/05-auth.md`)
- lfd JWT validation (see `roadmap/rust/05-auth.md`)
- lf CLI auth commands (see `roadmap/rust/05-auth.md`)
- Device code flow (defer until watchOS/tvOS needed)
- Multiple account support (single user for v1)
- Offline mode (requires tokens, which require network)

## Current status (2026-02-03)

**Implemented:**
- Swift auth client primitives (AuthService/AuthState/AuthError) with Keychain storage and ASWebAuthenticationSession flow.
- WaveService/EventService protocol split with LocalWaveService/LocalEventService wiring.
- HTTP API usage for waves, wave runs, and worktrees (plus parsing helpers and LFD base URL constant).
- URL scheme registration for `loopflow://` callbacks.
- Standardized lfd HTTP port via `http_server.DEFAULT_PORT` (2486).

**Not implemented yet:**
- RemoteWaveService and wiring TokenProvider into a remote client.
- Server-side WorkOS integration and lfd JWT validation (see `roadmap/rust/05-auth.md`).
- iOS target/platform support updates (package still macOS-focused).

**Risks / bottlenecks:**
- Token refresh contract assumed (`/auth/refresh` returning `token`, `jwt`, or raw string). If server differs, silent refresh fails.
- Port change to 2486 requires external clients/scripts to follow DEFAULT_PORT.
- AuthState refresh loop runs continuously; if refresh endpoint is flaky it may error quietly unless token expires.

## Implementation

### AuthService

```swift
// swift/LoopflowCore/Services/AuthService.swift

import AuthenticationServices

public final class AuthService: NSObject, Sendable {
    private let keychainService = "studio.loopflow.auth"
    private let baseURL = URL(string: "https://loopflow.studio")!

    /// Sign in via loopflow.studio (GitHub, Google, or Apple). Returns JWT on success.
    @MainActor
    public func signIn() async throws -> String {
        let callbackScheme = "loopflow"
        // WorkOS AuthKit shows provider picker—no need to specify provider client-side
        let authURL = baseURL.appendingPathComponent("auth/login")
            .appending(queryItems: [
                URLQueryItem(name: "redirect_uri", value: "\(callbackScheme)://auth/callback")
            ])

        let callbackURL = try await withCheckedThrowingContinuation { continuation in
            let session = ASWebAuthenticationSession(
                url: authURL,
                callbackURLScheme: callbackScheme
            ) { url, error in
                if let error { continuation.resume(throwing: error) }
                else if let url { continuation.resume(returning: url) }
                else { continuation.resume(throwing: AuthError.noCallback) }
            }
            session.presentationContextProvider = self
            session.prefersEphemeralWebBrowserSession = false // Share Safari cookies
            session.start()
        }

        // Extract token from callback URL
        guard let components = URLComponents(url: callbackURL, resolvingAgainstBaseURL: false),
              let token = components.queryItems?.first(where: { $0.name == "token" })?.value
        else {
            throw AuthError.invalidCallback
        }

        // Store in Keychain
        try saveToken(token)

        return token
    }

    public func signOut() throws {
        try deleteToken()
    }

    public func currentToken() -> String? {
        loadToken()
    }

    public func tokenExpiresAt() -> Date? {
        guard let token = loadToken() else { return nil }
        return decodeExpiry(token)
    }
}

// MARK: - Keychain

extension AuthService {
    private func saveToken(_ token: String) throws {
        let data = Data(token.utf8)
        let query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: keychainService,
            kSecAttrAccount as String: "jwt",
            kSecValueData as String: data,
            kSecAttrAccessible as String: kSecAttrAccessibleAfterFirstUnlock
        ]

        SecItemDelete(query as CFDictionary) // Remove existing
        let status = SecItemAdd(query as CFDictionary, nil)
        guard status == errSecSuccess else {
            throw AuthError.keychainWrite(status)
        }
    }

    private func loadToken() -> String? {
        let query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: keychainService,
            kSecAttrAccount as String: "jwt",
            kSecReturnData as String: true
        ]

        var result: AnyObject?
        let status = SecItemCopyMatching(query as CFDictionary, &result)
        guard status == errSecSuccess, let data = result as? Data else { return nil }
        return String(data: data, encoding: .utf8)
    }

    private func deleteToken() throws {
        let query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: keychainService,
            kSecAttrAccount as String: "jwt"
        ]
        let status = SecItemDelete(query as CFDictionary)
        guard status == errSecSuccess || status == errSecItemNotFound else {
            throw AuthError.keychainDelete(status)
        }
    }

    private func decodeExpiry(_ token: String) -> Date? {
        // JWT is base64url(header).base64url(payload).signature
        let parts = token.split(separator: ".")
        guard parts.count == 3,
              let payloadData = base64UrlDecode(String(parts[1])),
              let json = try? JSONSerialization.jsonObject(with: payloadData) as? [String: Any],
              let exp = json["exp"] as? TimeInterval
        else { return nil }
        return Date(timeIntervalSince1970: exp)
    }

    private func base64UrlDecode(_ string: String) -> Data? {
        var base64 = string
            .replacingOccurrences(of: "-", with: "+")
            .replacingOccurrences(of: "_", with: "/")
        while base64.count % 4 != 0 { base64.append("=") }
        return Data(base64Encoded: base64)
    }
}

extension AuthService: ASWebAuthenticationPresentationContextProviding {
    public func presentationAnchor(for session: ASWebAuthenticationSession) -> ASPresentationAnchor {
        #if os(macOS)
        NSApp.keyWindow ?? NSApp.windows.first!
        #else
        UIApplication.shared.connectedScenes
            .compactMap { $0 as? UIWindowScene }
            .flatMap { $0.windows }
            .first { $0.isKeyWindow }!
        #endif
    }
}
```

### AuthState

```swift
// swift/LoopflowCore/Services/AuthState.swift

import Foundation

@Observable
public final class AuthState {
    private let authService: AuthService
    private var refreshTask: Task<Void, Never>?

    public private(set) var token: String?
    public private(set) var isLoading = false
    public private(set) var error: AuthError?

    public var isAuthenticated: Bool { token != nil && !isExpired }

    public var isExpired: Bool {
        guard let exp = authService.tokenExpiresAt() else { return true }
        return Date() >= exp
    }

    public var needsRefresh: Bool {
        guard let exp = authService.tokenExpiresAt() else { return false }
        return Date().addingTimeInterval(24 * 3600) >= exp
    }

    public init(authService: AuthService = AuthService()) {
        self.authService = authService
        self.token = authService.currentToken()
        startRefreshMonitor()
    }

    @MainActor
    public func signIn() async {
        isLoading = true
        error = nil
        do {
            token = try await authService.signIn()
        } catch let e as AuthError {
            error = e
        } catch {
            self.error = .unknown(error)
        }
        isLoading = false
    }

    public func signOut() {
        try? authService.signOut()
        token = nil
        error = nil
    }

    private func startRefreshMonitor() {
        refreshTask = Task {
            while !Task.isCancelled {
                try? await Task.sleep(for: .seconds(3600)) // Check hourly
                if needsRefresh && !isExpired {
                    await refreshTokenSilently()
                }
            }
        }
    }

    private func refreshTokenSilently() async {
        // Silent refresh: hit loopflow.studio/auth/refresh with current token
        // If it fails, continue with current token until truly expired
    }
}
```

### TokenProvider

```swift
// swift/LoopflowCore/Services/TokenProvider.swift

public protocol TokenProvider: Sendable {
    func token() async throws -> String
}

public struct NoAuthProvider: TokenProvider {
    public init() {}
    public func token() async throws -> String { "" }
}

public final class KeychainTokenProvider: TokenProvider, Sendable {
    private let authService: AuthService

    public init(authService: AuthService = AuthService()) {
        self.authService = authService
    }

    public func token() async throws -> String {
        guard let token = authService.currentToken() else {
            throw AuthError.notAuthenticated
        }

        if let exp = authService.tokenExpiresAt(), Date() >= exp {
            throw AuthError.tokenExpired
        }

        return token
    }
}
```

### AuthError

```swift
// swift/LoopflowCore/Services/AuthError.swift

public enum AuthError: Error, Sendable {
    case noCallback
    case invalidCallback
    case notAuthenticated
    case tokenExpired
    case keychainWrite(OSStatus)
    case keychainDelete(OSStatus)
    case unknown(Error)
}

extension AuthError: LocalizedError {
    public var errorDescription: String? {
        switch self {
        case .noCallback: "Authentication was cancelled"
        case .invalidCallback: "Invalid authentication response"
        case .notAuthenticated: "Not signed in"
        case .tokenExpired: "Session expired, please sign in again"
        case .keychainWrite(let status): "Failed to save credentials (\(status))"
        case .keychainDelete(let status): "Failed to clear credentials (\(status))"
        case .unknown(let error): error.localizedDescription
        }
    }
}
```

### URL Scheme Registration

Add to Concerto's `Info.plist`:

```xml
<key>CFBundleURLTypes</key>
<array>
    <dict>
        <key>CFBundleURLSchemes</key>
        <array>
            <string>loopflow</string>
        </array>
        <key>CFBundleURLName</key>
        <string>studio.loopflow.auth</string>
    </dict>
</array>
```

## Done when

- [ ] `AuthService.signIn()` opens loopflow.studio auth (provider picker via WorkOS)
- [ ] JWT stored in Keychain after successful auth
- [ ] `AuthState.isAuthenticated` reflects current state
- [ ] Token expiry detected, user prompted to re-authenticate
- [ ] `TokenProvider` injectable into `RemoteWaveService`
- [ ] `signOut()` clears Keychain
- [ ] URL scheme `loopflow://` registered and handles callback
- [ ] Works on macOS 14+ and iOS 17+
