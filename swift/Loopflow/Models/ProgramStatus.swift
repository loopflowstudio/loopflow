import Foundation

public enum ProgramStatusState: String, Codable, Sendable, Hashable {
    case idle, working, done, blocked, error, clear
}

public enum ProgramStatusKind: String, Codable, Sendable, Hashable {
    case permission, question, auth
}

/// Validated program claims. These values confer no process or completion authority.
public struct ProgramStatusReport: Codable, Sendable, Hashable {
    public let state: ProgramStatusState
    public let id: String?
    public let kind: ProgramStatusKind?
    public let progress: UInt8?
    public let app: String?
    public let title: String?
    public let msg: String?

    public init(state: ProgramStatusState, id: String?, kind: ProgramStatusKind?, progress: UInt8?,
                app: String?, title: String?, msg: String?) {
        self.state = state
        self.id = id
        self.kind = kind
        self.progress = progress
        self.app = app
        self.title = title
        self.msg = msg
    }

    public var displayText: String {
        var parts = [state.rawValue.capitalized]
        if state == .blocked, let kind { parts.append(kind.rawValue.capitalized) }
        if let progress { parts.append("\(progress)%") }
        if let msg, !msg.isEmpty { parts.append(Self.displaySafe(msg)) }
        return parts.joined(separator: " · ")
    }

    /// Strip invisible formatting only at presentation; preserve the reported bytes.
    public static func displaySafe(_ text: String) -> String {
        String(text.unicodeScalars.filter {
            $0.properties.generalCategory != .format && !CharacterSet.controlCharacters.contains($0)
        })
    }

    public func encode(to encoder: Encoder) throws {
        var c = encoder.container(keyedBy: CodingKeys.self)
        try c.encode(state, forKey: .state)
        try c.encode(id, forKey: .id)
        try c.encode(kind, forKey: .kind)
        try c.encode(progress, forKey: .progress)
        try c.encode(app, forKey: .app)
        try c.encode(title, forKey: .title)
        try c.encode(msg, forKey: .msg)
    }
}

public struct ProgramStatusRecords: Codable, Sendable, Hashable {
    public let seen: Bool
    /// Oldest update first, newest last, matching the terminal reducer.
    public let records: [ProgramStatusReport]

    public init(seen: Bool, records: [ProgramStatusReport]) {
        self.seen = seen
        self.records = records
    }

    public var summary: ProgramStatusReport? {
        let priorities: [ProgramStatusState] = [.blocked, .error, .working, .done, .idle]
        for state in priorities {
            if let report = records.last(where: { $0.state == state }) { return report }
        }
        return nil
    }

    public func app(for report: ProgramStatusReport) -> String? {
        if let app = report.app { return app }
        var path = (report.id ?? "").split(separator: "/").map(String.init)
        while !path.isEmpty {
            path.removeLast()
            let parent = path.joined(separator: "/")
            if let app = records.last(where: { ($0.id ?? "") == parent })?.app { return app }
        }
        return nil
    }
}

/// Events copied out of Ghostty's callback before crossing to the main actor.
public enum ProgramStatusEvent: Sendable, Equatable {
    case report(ProgramStatusReport)
    case prompt, reset, exit, interaction
}

/// A terminal endpoint reducer; Session Waiting is always supplied by Rust.
public struct ProgramStatusReducer: Sendable {
    public private(set) var snapshot = ProgramStatusRecords(seen: false, records: [])

    public init() {}

    public mutating func apply(_ event: ProgramStatusEvent) {
        var records = snapshot.records
        var seen = snapshot.seen
        switch event {
        case .report(let report):
            seen = true
            let id = report.id ?? ""
            if report.state == .clear {
                records.removeAll { id.isEmpty || ($0.id ?? "") == id || ($0.id ?? "").hasPrefix(id + "/") }
            } else {
                records.removeAll { $0.id == report.id }
                records.append(report)
                if records.count > 64 { records.removeFirst() }
            }
        case .prompt, .exit:
            records.removeAll { $0.state != .done && $0.state != .error }
        case .reset:
            records.removeAll()
        case .interaction:
            records.removeAll { $0.state == .done || $0.state == .error }
        }
        snapshot = ProgramStatusRecords(seen: seen, records: records)
    }
}
