import Foundation

/// `lf wave create --json`: the Wave's id, whether this call created it, and
/// `summary`, what the add left to finish (its files uncommitted on main),
/// rendered verbatim so the CLI and Desktop say the same thing. No field is
/// optional, so the synthesized decoder already requires every key.
public struct WaveAdded: Decodable, Sendable, Hashable {
    public let id: String
    public let name: String
    public let created: Bool
    public let mainCheckout: String
    public let files: WaveRemoval.AuthoredFiles
    public let summary: [String]

    enum CodingKeys: String, CodingKey {
        case id, name, created, files, summary
        case mainCheckout = "main_checkout"
    }
}
