import Foundation
import Loopflow

/// Presentation of shared Session evidence; never Task completion or graph position.
enum SessionProgramStatus {
    case reported(SessionRecord, ProgramStatusReport)
    case available(SessionRecord)
    case unknown
    case unavailable(String)

    init(sessions: [SessionRecord], unavailable: String?) {
        if let unavailable { self = .unavailable(unavailable); return }
        // Preserve report identity: different terminals can use the same record id.
        let ordered = sessions.sorted(by: Self.precedes)
        let reports = ordered.compactMap { session in
            session.programStatus?.summary.map { (session, $0) }
        }
        for state in ProgramStatusState.summaryOrder {
            if let (session, report) = reports.first(where: { $0.1.state == state }) {
                self = .reported(session, report); return
            }
        }
        // Once a reporter clears its records, do not re-infer its old status.
        if let available = ordered.first(where: {
            $0.programStatus?.seen != true && $0.state != .closed && $0.availability == .available
        }) {
            self = .available(available)
        } else { self = .unknown }
    }

    private static func precedes(_ lhs: SessionRecord, _ rhs: SessionRecord) -> Bool {
        if lhs.taskPrimary != rhs.taskPrimary { return lhs.taskPrimary }
        return lhs.id < rhs.id
    }

    var session: SessionRecord? {
        switch self {
        case .reported(let session, _), .available(let session): session
        case .unknown, .unavailable: nil
        }
    }

    /// Freshness changes presentation only, never the underlying execution status.
    func preview(now: Date, fallback: String, sessions: [SessionRecord] = []) -> (text: String, help: String, age: String?) {
        let candidate: SessionRecord?
        switch self {
        case .unavailable: candidate = nil
        case .reported(_, let report) where report.state == .idle || report.state == .done: candidate = nil
        default: candidate = session ?? sessions.min(by: Self.precedes)
        }
        if let session = candidate, session.programStatus?.seen != true || session.programStatus?.summary != nil,
           session.state == .active || session.state == .detached, let excerpt = session.preview,
           !excerpt.providerThread.isEmpty, !excerpt.providerTurn.isEmpty,
           session.agentProcessId != nil {
            let age = now.timeIntervalSince1970 - Double(excerpt.observedAt)
            let available = session.availability == .available
            if age >= 0, available || age <= 30 {
                let safe = ProgramStatusReport.displaySafe(excerpt.text)
                if !safe.isEmpty {
                    let line = safe.split(separator: "\n").last.map(String.init) ?? safe
                    let label = age < 60 ? "\(Int(age))s" : age < 3600 ? "\(Int(age / 60))m" : "\(Int(age / 3600))h"
                    return (line, "\(session.title) (\(session.id)) · \(excerpt.source) · \(Int(age))s ago\n\(safe)", label)
                }
            }
        }
        let summary = fallback.split(separator: "\n").first
            .map { ProgramStatusReport.displaySafe(String($0)) } ?? "No current activity"
        let detail: String
        if case .unavailable(let reason) = self { detail = "Activity unavailable · \(reason)\nSummary · \(summary)" }
        else { detail = "Summary · \(summary)" }
        return ("Summary · \(summary)", detail, nil)
    }

    var label: String? {
        switch self {
        case .reported(_, let report):
            let kind = report.kind.map { " · \($0.rawValue.capitalized)" } ?? ""
            let progress = report.progress.map { " · \($0)%" } ?? ""
            return "\(report.state.rawValue.capitalized)\(kind)\(progress)"
        case .available: return "Available · inferred"
        case .unknown, .unavailable: return nil
        }
    }

    var help: String {
        switch self {
        case .reported(let session, let report):
            return "Program report · \(ProgramStatusReport.displaySafe(session.title)) (\(session.id))\n\(report.displayText)"
        case .available(let session): return "Inferred from shared Session Available · \(session.id)"
        case .unknown: return "No current program report or shared Available."
        case .unavailable(let reason): return "Current program status unavailable: \(reason)"
        }
    }

    var tone: WorkTone? {
        switch self {
        case .reported(_, let report):
            report.state.workTone
        case .available: .available
        case .unknown, .unavailable: nil
        }
    }
}


/// Existing provider and terminal metadata, with no process-name guessing.
struct PaneProgramHeading: Equatable {
    let name: String
    let symbol: String
    let context: String
    let detail: String

    init(provider: String?, title: String, directory: String?, fallback: String) {
        switch provider?.lowercased() {
        case "codex": name = "Codex"; symbol = "terminal"
        case "claude": name = "Claude"; symbol = "sparkle"
        case "pi": name = "Pi"; symbol = "terminal"
        case "opencode": name = "OpenCode"; symbol = "chevron.left.forwardslash.chevron.right"
        case .some(let value) where !value.isEmpty:
            name = ProgramStatusReport.displaySafe(value); symbol = "bubble.left"
        default:
            name = fallback; symbol = fallback == "Shell" ? "terminal" : "rectangle"
        }
        let title = ProgramStatusReport.displaySafe(title)
        let folder = directory.map { URL(fileURLWithPath: $0).lastPathComponent }
        context = [title == name ? nil : title, folder].compactMap { $0 }.filter { !$0.isEmpty }.joined(separator: " · ")
        detail = [title, directory].compactMap { $0 }.joined(separator: "\n")
    }
}
