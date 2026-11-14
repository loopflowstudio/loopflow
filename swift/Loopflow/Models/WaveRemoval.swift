import Foundation

/// `lf wave remove [--preview] --json`: the consequences Desktop shows before
/// removal and the receipt after it. `summary` is rendered verbatim so the CLI
/// and Desktop say the same thing. Optional fields must be present as `null`.
public struct WaveRemoval: Decodable, Sendable, Hashable {
    public let id: String
    public let name: String
    public let preview: Bool
    public let alreadyRemoved: Bool
    public let destination: String?
    public let waves: [String]
    public let moved: [Task]
    public let deleted: [Task]
    public let completedTasks: Int
    public let chapters: Int
    public let sessions: Int
    public let initiatives: [InitiativeArchive]
    public let hasFiles: Bool
    /// Edits under `.lf/wave/<name>` nobody committed; removal deletes them.
    public let uncommitted: [String]
    public let files: AuthoredFiles?
    public let summary: [String]

    /// Whether `summary` ends with lines a person still has to act on: an
    /// Initiative Linear kept live or files left uncommitted.
    public var hasLeftovers: Bool {
        if case .uncommitted = files { return true }
        return initiatives.contains { $0.error != nil }
    }

    public struct Task: Decodable, Sendable, Hashable {
        public let id: String
        public let identifier: String
        public let title: String
        public let started: Bool
    }

    public struct InitiativeArchive: Decodable, Sendable, Hashable {
        public let wave: String
        public let initiative: String
        public let error: String?

        public init(from decoder: Decoder) throws {
            let container = try decoder.container(keyedBy: CodingKeys.self)
            wave = try container.decode(String.self, forKey: .wave)
            initiative = try container.decode(String.self, forKey: .initiative)
            error = try container.decode(String?.self, forKey: .error)
        }

        enum CodingKeys: String, CodingKey { case wave, initiative, error }
    }

    /// What happened to `.lf/wave/<name>/` on the main checkout.
    public enum AuthoredFiles: Decodable, Sendable, Hashable {
        case committed(commit: String)
        case unchanged
        case uncommitted(reason: String)

        enum CodingKeys: String, CodingKey { case state, commit, reason }

        public init(from decoder: Decoder) throws {
            let container = try decoder.container(keyedBy: CodingKeys.self)
            switch try container.decode(String.self, forKey: .state) {
            case "committed": self = .committed(commit: try container.decode(String.self, forKey: .commit))
            case "unchanged": self = .unchanged
            case "uncommitted": self = .uncommitted(reason: try container.decode(String.self, forKey: .reason))
            case let other:
                throw DecodingError.dataCorruptedError(
                    forKey: .state, in: container, debugDescription: "unknown files state \(other)")
            }
        }
    }

    enum CodingKeys: String, CodingKey {
        case id, name, preview, destination, waves, moved, deleted, chapters, sessions
        case initiatives, uncommitted, files, summary
        case alreadyRemoved = "already_removed"
        case completedTasks = "completed_tasks"
        case hasFiles = "has_files"
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        id = try container.decode(String.self, forKey: .id)
        name = try container.decode(String.self, forKey: .name)
        preview = try container.decode(Bool.self, forKey: .preview)
        alreadyRemoved = try container.decode(Bool.self, forKey: .alreadyRemoved)
        destination = try container.decode(String?.self, forKey: .destination)
        waves = try container.decode([String].self, forKey: .waves)
        moved = try container.decode([Task].self, forKey: .moved)
        deleted = try container.decode([Task].self, forKey: .deleted)
        completedTasks = try container.decode(Int.self, forKey: .completedTasks)
        chapters = try container.decode(Int.self, forKey: .chapters)
        sessions = try container.decode(Int.self, forKey: .sessions)
        initiatives = try container.decode([InitiativeArchive].self, forKey: .initiatives)
        hasFiles = try container.decode(Bool.self, forKey: .hasFiles)
        uncommitted = try container.decode([String].self, forKey: .uncommitted)
        files = try container.decode(AuthoredFiles?.self, forKey: .files)
        summary = try container.decode([String].self, forKey: .summary)
    }
}
