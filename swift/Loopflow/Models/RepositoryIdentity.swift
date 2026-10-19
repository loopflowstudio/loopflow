import Foundation

/// Retained locators are Machine-local evidence of explicit repository association.
public struct RepositoryIdentity: Codable, Sendable, Equatable {
    public let id: String
    public let locators: [String]

    public init(id: String, locators: [String]) {
        self.id = StructuredIdentity.prefixed(id, "repo_")
        self.locators = locators.map { StructuredIdentity.prefixed($0, "repo_") }
    }
}

extension RepositoryIdentity {
    public init(from decoder: Decoder) throws {
        let values = try decoder.container(keyedBy: CodingKeys.self)
        let raw = try values.decode([String].self, forKey: .locators)
        let canonical = raw.map { StructuredIdentity.prefixed($0, "repo_") }
        guard Set(raw).count == Set(canonical).count else {
            throw DecodingError.dataCorruptedError(forKey: .locators, in: values, debugDescription: "Repository identity collision")
        }
        self.init(id: try values.decode(String.self, forKey: .id), locators: raw)
    }
}
