import Foundation

public enum AuthProvider: String, Codable, Sendable, CaseIterable {
    case github
    case claude
    case codex

    public var displayName: String {
        switch self {
        case .github:
            "GitHub"
        case .claude:
            "Claude"
        case .codex:
            "Codex"
        }
    }

    public var icon: String {
        switch self {
        case .github:
            "terminal"
        case .claude:
            "brain.head.profile"
        case .codex:
            "cpu"
        }
    }
}

public enum ProviderAuthStatus: String, Codable, Sendable {
    case active
    case pending
    case none
    case expired
}

public struct AuthProviderStatus: Codable, Sendable, Identifiable {
    public let provider: AuthProvider
    public let status: ProviderAuthStatus
    public let login: String?

    public var id: AuthProvider { provider }

    public init(provider: AuthProvider, status: ProviderAuthStatus, login: String? = nil) {
        self.provider = provider
        self.status = status
        self.login = login
    }
}

public struct AuthFlow: Codable, Sendable {
    public let provider: AuthProvider
    public let verification_uri: String
    public let verification_uri_complete: String?
    public let user_code: String?
    public let expires_in: UInt64?

    public init(
        provider: AuthProvider,
        verification_uri: String,
        verification_uri_complete: String? = nil,
        user_code: String? = nil,
        expires_in: UInt64? = nil
    ) {
        self.provider = provider
        self.verification_uri = verification_uri
        self.verification_uri_complete = verification_uri_complete
        self.user_code = user_code
        self.expires_in = expires_in
    }
}

public struct AuthProviderListResponse: Codable, Sendable {
    public let providers: [AuthProviderStatus]

    public init(providers: [AuthProviderStatus]) {
        self.providers = providers
    }
}
