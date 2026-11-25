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
    public let summary: [String]

    enum CodingKeys: String, CodingKey {
        case id, from, to, renamed, files, summary
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        id = try container.decode(String.self, forKey: .id)
        from = try container.decode(String.self, forKey: .from)
        to = try container.decode(String.self, forKey: .to)
        renamed = try container.decode(Bool.self, forKey: .renamed)
        files = try container.decode(WaveRemoval.AuthoredFiles.self, forKey: .files)
        summary = try container.decode([String].self, forKey: .summary)
    }
}
