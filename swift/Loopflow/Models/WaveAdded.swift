import Foundation

/// `lf wave add --json`: the Wave's id, whether this call created it, and
/// `summary`, what the add left to finish (its files uncommitted on main),
/// rendered verbatim so the CLI and Desktop say the same thing. The receipt's
/// `project` is the Wave's first Project; Desktop reads Projects from the
/// planning stream, so only its identity is mirrored here.
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

        public init(from decoder: Decoder) throws {
            let container = try decoder.container(keyedBy: CodingKeys.self)
            id = try container.decode(String.self, forKey: .id)
            name = try container.decode(String.self, forKey: .name)
        }

        enum CodingKeys: String, CodingKey { case id, name }
    }

    enum CodingKeys: String, CodingKey {
        case id, name, created, project, files, summary
        case mainCheckout = "main_checkout"
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        id = try container.decode(String.self, forKey: .id)
        name = try container.decode(String.self, forKey: .name)
        created = try container.decode(Bool.self, forKey: .created)
        project = try container.decode(Project.self, forKey: .project)
        mainCheckout = try container.decode(String.self, forKey: .mainCheckout)
        files = try container.decode(WaveRemoval.AuthoredFiles.self, forKey: .files)
        summary = try container.decode([String].self, forKey: .summary)
    }
}
