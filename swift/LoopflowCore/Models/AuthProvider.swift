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
    public let verificationURI: String
    public let verificationURIComplete: String?
    public let userCode: String?
    public let expiresIn: UInt64?

    enum CodingKeys: String, CodingKey {
        case provider
        case verificationURI = "verification_uri"
        case verificationURIComplete = "verification_uri_complete"
        case userCode = "user_code"
        case expiresIn = "expires_in"
    }

    public init(
        provider: AuthProvider,
        verificationURI: String,
        verificationURIComplete: String? = nil,
        userCode: String? = nil,
        expiresIn: UInt64? = nil
    ) {
        self.provider = provider
        self.verificationURI = verificationURI
        self.verificationURIComplete = verificationURIComplete
        self.userCode = userCode
        self.expiresIn = expiresIn
    }
}

public struct AuthProviderListResponse: Codable, Sendable {
    public let providers: [AuthProviderStatus]

    public init(providers: [AuthProviderStatus]) {
        self.providers = providers
    }
}
