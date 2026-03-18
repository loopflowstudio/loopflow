import Foundation

public struct SuppliedKey: Codable, Sendable, Identifiable {
    public let envName: String
    public let provider: String
    public let present: Bool

    public var id: String { envName }

    enum CodingKeys: String, CodingKey {
        case envName = "env_name"
        case provider
        case present
    }

    public init(envName: String, provider: String, present: Bool) {
        self.envName = envName
        self.provider = provider
        self.present = present
    }
}

public struct SecretsProviderStatus: Codable, Sendable {
    public let provider: String
    public let connected: Bool
    public let project: String?
    public let config: String?
    public let keys: [SuppliedKey]

    public init(
        provider: String,
        connected: Bool,
        project: String? = nil,
        config: String? = nil,
        keys: [SuppliedKey] = []
    ) {
        self.provider = provider
        self.connected = connected
        self.project = project
        self.config = config
        self.keys = keys
    }

    public static var disconnected: SecretsProviderStatus {
        SecretsProviderStatus(
            provider: "",
            connected: false,
            keys: [
                SuppliedKey(envName: "ANTHROPIC_API_KEY", provider: "claude", present: false),
                SuppliedKey(envName: "OPENAI_API_KEY", provider: "codex", present: false),
            ]
        )
    }

    public var presentKeys: [SuppliedKey] { keys.filter(\.present) }
    public var missingKeys: [SuppliedKey] { keys.filter { !$0.present } }
}
