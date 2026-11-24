import Foundation

/// The Wave created or joined by `lf wave create --json`. An unconfigured
/// Wave carries a null Project. Desktop reads its full Projects from the
/// planning stream and shows the receipt's summary verbatim.
public struct WaveAdded: Decodable, Sendable, Hashable {
    public let id: String
    public let name: String
    public let created: Bool
    public let project: Project?
    public let mainCheckout: String
    public let files: WaveRemoval.AuthoredFiles
    public let summary: [String]

    public struct Project: Decodable, Sendable, Hashable {
        public let id: String
        public let name: String
    }

    public init(from decoder: Decoder) throws {
        let fields = try decoder.container(keyedBy: CodingKeys.self)
        id = try fields.decode(String.self, forKey: .id)
        name = try fields.decode(String.self, forKey: .name)
        created = try fields.decode(Bool.self, forKey: .created)
        project = try fields.decode(Project?.self, forKey: .project)
        mainCheckout = try fields.decode(String.self, forKey: .mainCheckout)
        files = try fields.decode(WaveRemoval.AuthoredFiles.self, forKey: .files)
        summary = try fields.decode([String].self, forKey: .summary)
    }

    enum CodingKeys: String, CodingKey {
        case id, name, created, project, files, summary
        case mainCheckout = "main_checkout"
    }
}
