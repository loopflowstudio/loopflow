import Foundation

public struct ConnectionProfile: Codable, Identifiable, Sendable, Hashable {
    public let id: UUID
    public var name: String
    public var connection: ServerConnection
    public var lastConnectedAt: Date?

    public init(
        id: UUID = UUID(),
        name: String,
        connection: ServerConnection,
        lastConnectedAt: Date? = nil
    ) {
        self.id = id
        self.name = name
        self.connection = connection
        self.lastConnectedAt = lastConnectedAt
    }
}
