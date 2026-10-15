import Foundation
import Loopflow

/// Presentation of shared Session evidence; never Task completion or Workflow state.
enum SessionProgramStatus {
    case reported(SessionRecord, ProgramStatusReport)
    case waiting(SessionRecord)
    case unknown
    case unavailable(String)

    init(sessions: [SessionRecord], unavailable: String?) {
        if let unavailable { self = .unavailable(unavailable); return }
        // Preserve report identity: different terminals can use the same record id.
        let ordered = sessions.sorted {
            if $0.taskPrimary != $1.taskPrimary { return $0.taskPrimary }
            return $0.id < $1.id
        }
        let reports = ordered.compactMap { session in
            session.programStatus?.summary.map { (session, $0) }
        }
        for state in ProgramStatusState.summaryOrder {
            if let (session, report) = reports.first(where: { $0.1.state == state }) {
                self = .reported(session, report); return
            }
        }
        // Once a reporter clears its records, do not re-infer its old status.
        if let waiting = ordered.first(where: {
            $0.programStatus?.seen != true && $0.state != .closed && $0.attention == .waiting
        }) {
            self = .waiting(waiting)
        } else { self = .unknown }
    }

    var label: String? {
        switch self {
        case .reported(_, let report):
            let kind = report.kind.map { " · \($0.rawValue.capitalized)" } ?? ""
            let progress = report.progress.map { " · \($0)%" } ?? ""
            return "\(report.state.rawValue.capitalized)\(kind)\(progress)"
        case .waiting: return "Waiting · inferred"
        case .unknown, .unavailable: return nil
        }
    }

    var help: String {
        switch self {
        case .reported(let session, let report):
            return "Program report · \(ProgramStatusReport.displaySafe(session.title)) (\(session.id))\n\(report.displayText)"
        case .waiting(let session): return "Inferred from shared Session Waiting · \(session.id)"
        case .unknown: return "No current program report or shared Waiting."
        case .unavailable(let reason): return "Current program status unavailable: \(reason)"
        }
    }

    var tone: WorkTone? {
        switch self {
        case .reported(_, let report):
            report.state.workTone
        case .waiting: .waiting
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
