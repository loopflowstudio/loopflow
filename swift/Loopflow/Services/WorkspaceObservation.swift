import Foundation

/// One foreground reader per window: `lf monitor workspace --watch --json`.
/// Each part arrives again only when a commit changed what it shows.
public struct WorkspaceObservation: Sendable {
    public let frames: AsyncThrowingStream<WorkspaceFrame, any Error>
    /// Requests are delivered in the order they are made.
    public let request: @Sendable (WorkspaceRequest) -> Void
    /// Returns only after the owned reader has exited.
    public let cancel: @Sendable () async -> Void

    public init(
        frames: AsyncThrowingStream<WorkspaceFrame, any Error>,
        request: @escaping @Sendable (WorkspaceRequest) -> Void,
        cancel: @escaping @Sendable () async -> Void
    ) {
        self.frames = frames
        self.request = request
        self.cancel = cancel
    }
}

public enum WorkspaceObservationError: Error, Sendable {
    /// Replace the reader only after draining its previous launch authority.
    case configurationChanged
}

/// The Work whose activity is shown. Mirrors Rust `WorkActivityScope`.
public struct WorkActivityScope: Decodable, Equatable, Sendable {
    public let wave: String?
    public let project: String?
    public let task: String?

    public init(wave: String?, project: String?, task: String?) {
        self.wave = wave
        self.project = project
        self.task = task
    }
}

/// What a window shows. Mirrors the Rust reader's `scope` request.
public struct WorkspaceScope: Equatable, Sendable {
    public let repo: String?
    public let headless: Bool
    /// Task identifier whose work is shown.
    public let task: String?
    /// Wave id whose detail is shown.
    public let wave: String?
    public let activity: WorkActivityScope?

    public init(repo: String?, headless: Bool, task: String?, wave: String?, activity: WorkActivityScope?) {
        self.repo = repo
        self.headless = headless
        self.task = task
        self.wave = wave
        self.activity = activity
    }
}

/// Every request carries an id; a frame's `answers` names the newest one
/// handled before that frame was read.
public enum WorkspaceRequest: Equatable, Sendable {
    case scope(id: Int, WorkspaceScope)
    case refresh(id: Int)

    public var id: Int {
        switch self {
        case .scope(let id, _), .refresh(let id): id
        }
    }

    /// One newline-terminated JSON line. Absent values are written as `null`.
    public var line: Data {
        var object: [String: Any] = ["id": id]
        switch self {
        case .scope(_, let scope):
            object["action"] = "scope"
            object["repo"] = scope.repo ?? NSNull()
            object["headless"] = scope.headless
            object["task"] = scope.task ?? NSNull()
            object["wave"] = scope.wave ?? NSNull()
            object["activity"] = scope.activity.map { activity -> Any in
                ["wave": activity.wave ?? NSNull(), "project": activity.project ?? NSNull(),
                 "task": activity.task ?? NSNull()] as [String: Any]
            } ?? NSNull()
        case .refresh: object["action"] = "refresh"
        }
        // The values above are all JSON types.
        var data = (try? JSONSerialization.data(withJSONObject: object)) ?? Data()
        data.append(10)
        return data
    }
}

/// Store revisions a frame was read at. Mirrors Rust `StoreRevisions`.
public struct StoreRevisions: Decodable, Equatable, Sendable {
    public let planning: Int64
    public let sessions: Int64
    public let flows: Int64
    public let execs: Int64
    public let usage: Int64
}

/// One line of the stream. Mirrors Rust `WorkspaceFrame`: the body is the same
/// wire type the one-shot `--json` read prints.
public struct WorkspaceFrame: Decodable, Sendable {
    public struct Planning: Decodable, Sendable {
        public let roadmap: RoadmapSnapshot
        public let waves: [WaveSnapshot]
    }

    public struct Sessions: Decodable, Sendable {
        public let repo: String
        public let includesHeadless: Bool
        public let entries: [SessionRecord]

        enum CodingKeys: String, CodingKey {
            case repo, entries
            case includesHeadless = "includes_headless"
        }
    }

    /// A Task's work and each of its Flow runs, in the order of `work.flows`.
    public struct TaskPart: Decodable, Sendable {
        public let task: String
        public let work: TaskWork
        public let flowRuns: [FlowDetail]

        enum CodingKeys: String, CodingKey {
            case task, work
            case flowRuns = "flow_runs"
        }
    }

    public struct WavePart: Decodable, Sendable {
        public let wave: String
        public let detail: WaveDetailSnapshot
    }

    public struct WorkActivityPart: Decodable, Sendable {
        public let scope: WorkActivityScope
        public let snapshot: WorkActivitySnapshot
    }

    public struct Heartbeat: Decodable, Sendable {
        /// Readings per part since the reader started, sent or not.
        public let projections: [String: Int]
    }

    /// A `nil` body is a reading that failed; `unavailable` says why.
    public enum Content: Sendable {
        case planning(Planning?)
        case sessions(Sessions?)
        case task(TaskPart?)
        case wave(WavePart?)
        case workActivity(WorkActivityPart?)
        case activity(ActivitySnapshot?)
        case heartbeat(Heartbeat)

        public var part: String {
            switch self {
            case .planning: "planning"
            case .sessions: "sessions"
            case .task: "task"
            case .wave: "wave"
            case .workActivity: "work_activity"
            case .activity: "activity"
            case .heartbeat: "heartbeat"
            }
        }
    }

    public let sequence: Int
    public let answers: Int?
    public let home: String
    public let revisions: StoreRevisions?
    public let unavailable: String?
    public let content: Content
    /// Wire text of the parts a saved workspace keeps, exactly as a one-shot
    /// read would have printed them. Present only on frames decoded from a line.
    public let wire: Wire?

    public struct Wire: Sendable {
        public let roadmap: String?
        public let waves: String?
        public let sessionPage: String?
    }

    enum CodingKeys: String, CodingKey {
        case sequence, answers, home, revisions, unavailable, part, body
    }

    public init(sequence: Int, answers: Int?, home: String, revisions: StoreRevisions?,
                unavailable: String?, content: Content, wire: Wire?) {
        self.sequence = sequence
        self.answers = answers
        self.home = home
        self.revisions = revisions
        self.unavailable = unavailable
        self.content = content
        self.wire = wire
    }

    public init(from decoder: any Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        sequence = try container.decode(Int.self, forKey: .sequence)
        answers = try container.decode(Int?.self, forKey: .answers)
        home = try container.decode(String.self, forKey: .home)
        revisions = try container.decode(StoreRevisions?.self, forKey: .revisions)
        unavailable = try container.decode(String?.self, forKey: .unavailable)
        wire = nil
        let part = try container.decode(String.self, forKey: .part)
        switch part {
        case "planning": content = .planning(try container.decode(Planning?.self, forKey: .body))
        case "sessions": content = .sessions(try container.decode(Sessions?.self, forKey: .body))
        case "task": content = .task(try container.decode(TaskPart?.self, forKey: .body))
        case "wave": content = .wave(try container.decode(WavePart?.self, forKey: .body))
        case "work_activity": content = .workActivity(try container.decode(WorkActivityPart?.self, forKey: .body))
        case "activity": content = .activity(try container.decode(ActivitySnapshot?.self, forKey: .body))
        case "heartbeat": content = .heartbeat(try container.decode(Heartbeat.self, forKey: .body))
        default:
            throw DecodingError.dataCorruptedError(forKey: .part, in: container,
                                                   debugDescription: "unknown workspace part \(part)")
        }
    }

    /// Decode one stream line, keeping the wire text a saved workspace needs.
    public static func decode(line: Data) throws -> WorkspaceFrame {
        let frame = try JSONDecoder().decode(WorkspaceFrame.self, from: line)
        let wire: Wire?
        switch frame.content {
        case .planning(.some), .sessions(.some):
            let body = (try JSONSerialization.jsonObject(with: line) as? [String: Any])?["body"] as? [String: Any]
            func text(_ value: Any?) -> String? {
                guard let value, let data = try? JSONSerialization.data(withJSONObject: value) else { return nil }
                return String(data: data, encoding: .utf8)
            }
            wire = Wire(
                roadmap: text(body?["roadmap"]),
                waves: text(body?["waves"]),
                sessionPage: body?["entries"].flatMap { text(["entries": $0, "next": NSNull()] as [String: Any]) })
        default:
            wire = nil
        }
        return WorkspaceFrame(sequence: frame.sequence, answers: frame.answers, home: frame.home,
                              revisions: frame.revisions, unavailable: frame.unavailable,
                              content: frame.content, wire: wire)
    }
}
