import Foundation

public struct ChatMemoryBlock: Identifiable, Sendable, Hashable {
    public var id: String { name }
    public var name: String
    public var content: String
    public var position: Int
    public var updatedAt: Date?

    public init(
        name: String,
        content: String,
        position: Int,
        updatedAt: Date? = nil
    ) {
        self.name = name
        self.content = content
        self.position = position
        self.updatedAt = updatedAt
    }
}
