import Foundation

public enum ProviderRole: String, Codable, Sendable {
    case agent
    case sourceControl
    case projectManagement
    case secrets

    public var displayName: String {
        switch self {
        case .agent: "Agents"
        case .sourceControl: "Source Control"
        case .projectManagement: "Project Management"
        case .secrets: "Secrets"
        }
    }

    public static var displayOrder: [ProviderRole] {
        [.agent, .sourceControl, .projectManagement, .secrets]
    }

    /// Roles configurable per-repo (enable/disable agents, PM tools, secrets config).
    /// Source control is global-only — GitHub auto-detects.
    public static var repoConfigurable: [ProviderRole] {
        [.agent, .projectManagement, .secrets]
    }
}

public enum AuthProvider: String, Codable, Sendable, CaseIterable {
    case github
    case claude
    case codex
    case opencode = "opencodezen"
    case asana
    case linear
    case doppler

    public var displayName: String {
        switch self {
        case .github: "GitHub"
        case .claude: "Claude"
        case .codex: "Codex"
        case .opencode: "OpenCode"
        case .asana: "Asana"
        case .linear: "Linear"
        case .doppler: "Doppler"
        }
    }

    public var icon: String {
        switch self {
        case .github: "terminal"
        case .claude: "brain.head.profile"
        case .codex: "cpu"
        case .opencode: "chevron.left.forwardslash.chevron.right"
        case .asana: "list.bullet.rectangle"
        case .linear: "square.stack.3d.up"
        case .doppler: "key.fill"
        }
    }

    public var role: ProviderRole {
        switch self {
        case .claude, .codex, .opencode: .agent
        case .github: .sourceControl
        case .asana, .linear: .projectManagement
        case .doppler: .secrets
        }
    }

    public static func providers(for role: ProviderRole) -> [AuthProvider] {
        allCases.filter { $0.role == role }
    }
}

public enum ProviderAuthStatus: String, Codable, Sendable {
    case active
    case pending
    case none
    case expired
}

public enum CredentialType: String, Codable, Sendable {
    case oauth
    case apikey
}

public struct AuthProviderStatus: Codable, Sendable, Identifiable {
    public let provider: AuthProvider
    public let status: ProviderAuthStatus
    public let login: String?
    public let credentialType: CredentialType?

    public var id: AuthProvider { provider }

    enum CodingKeys: String, CodingKey {
        case provider
        case status
        case login
        case credentialType = "credential_type"
    }

    public init(
        provider: AuthProvider,
        status: ProviderAuthStatus,
        login: String? = nil,
        credentialType: CredentialType? = nil
    ) {
        self.provider = provider
        self.status = status
        self.login = login
        self.credentialType = credentialType
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
