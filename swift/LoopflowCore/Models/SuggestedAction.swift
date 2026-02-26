import Foundation

public struct SuggestedAction: Sendable, Hashable, Identifiable {
    public let id: UUID
    public let label: String
    public let description: String?

    public init(id: UUID = UUID(), label: String, description: String? = nil) {
        self.id = id
        self.label = label
        self.description = description
    }
}

public struct SuggestedActionPayload: Sendable, Hashable {
    public let label: String
    public let description: String?

    public init(label: String, description: String? = nil) {
        self.label = label
        self.description = description
    }
}
