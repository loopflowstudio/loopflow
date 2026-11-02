import Foundation

/// `lf wave add --json`: the Wave's id, whether this call created it, and
/// `summary`, what the add left to finish (its files uncommitted on main),
/// rendered verbatim so the CLI and Desktop say the same thing. The receipt's
/// `project` is the Wave's first Project; Desktop reads Projects from the
/// planning stream, so only its identity is mirrored here. No field is
/// optional, so the synthesized decoder already requires every key.
public struct WaveAdded: Decodable, Sendable, Hashable {
    public let id: String
    public let name: String
    public let created: Bool
    public let project: Project
    public let mainCheckout: String
    public let files: WaveRemoval.AuthoredFiles
    public let summary: [String]

    public struct Project: Decodable, Sendable, Hashable {
        public let id: String
        public let name: String
    }

    enum CodingKeys: String, CodingKey {
        case id, name, created, project, files, summary
        case mainCheckout = "main_checkout"
    }
}
