import Foundation

/// `lf wave rename --json`: the Wave keeps its id; `summary` holds what the rename
/// left to finish, rendered verbatim so the CLI and Desktop say the same thing.
/// Optional fields must be present as `null`.
public struct WaveRenamed: Decodable, Sendable, Hashable {
    public let id: String
    public let from: String
    public let to: String
    public let renamed: Bool
    public let files: WaveRemoval.AuthoredFiles
    public let initiative: InitiativeRename?
    public let summary: [String]

    public struct InitiativeRename: Decodable, Sendable, Hashable {
        public let initiative: String
        public let title: String
        public let error: String?

        public init(from decoder: Decoder) throws {
            let container = try decoder.container(keyedBy: CodingKeys.self)
            initiative = try container.decode(String.self, forKey: .initiative)
            title = try container.decode(String.self, forKey: .title)
            error = try container.decode(String?.self, forKey: .error)
        }

        enum CodingKeys: String, CodingKey { case initiative, title, error }
    }

    enum CodingKeys: String, CodingKey {
        case id, from, to, renamed, files, initiative, summary
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        id = try container.decode(String.self, forKey: .id)
        from = try container.decode(String.self, forKey: .from)
        to = try container.decode(String.self, forKey: .to)
        renamed = try container.decode(Bool.self, forKey: .renamed)
        files = try container.decode(WaveRemoval.AuthoredFiles.self, forKey: .files)
        initiative = try container.decode(InitiativeRename?.self, forKey: .initiative)
        summary = try container.decode([String].self, forKey: .summary)
    }
}
