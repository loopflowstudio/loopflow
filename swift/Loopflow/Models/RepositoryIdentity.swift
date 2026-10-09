import Foundation

/// Retained locators are Machine-local evidence of explicit repository association.
public struct RepositoryIdentity: Codable, Sendable, Equatable {
    public let id: String
    public let locators: [String]

    public init(id: String, locators: [String]) {
        self.id = id
        self.locators = locators
    }
}
