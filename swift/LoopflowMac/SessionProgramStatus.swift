import Loopflow
import SwiftUI

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
        let reports = ordered.compactMap { session -> (SessionRecord, ProgramStatusReport)? in
            guard let report = session.programStatus?.summary,
                  report.state != .done || session.availability == .available else { return nil }
            return (session, report)
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

    /// What a blocked program is asking, in its own words.
    var ask: String? {
        guard case .reported(_, let report) = self, report.state == .blocked,
              let text = [report.msg, report.title].compactMap({ $0.map(ProgramStatusReport.displaySafe) })
                .first(where: { !$0.isEmpty }) else { return nil }
        return text
    }

    /// The row's status; nil when nothing reports and the row says its lifecycle.
    var row: TaskRowStatus? {
        switch self {
        case .reported(_, let report):
            .program(report.state == .done ? .idle : report.state,
                     kind: report.kind, progress: report.progress, detail: help, ask: ask)
        case .available: .program(.idle, inferred: true, detail: help)
        case .unknown: nil
        case .unavailable: .unread(help)
        }
    }

    var label: String? { row?.word }
    var tone: WorkTone? { row?.tone }

    var help: String {
        switch self {
        case .reported(let session, let report):
            return "Program report · \(ProgramStatusReport.displaySafe(session.title)) (\(session.id))\n\(report.displayText)"
        case .available(let session): return "Inferred from shared Session Available · \(session.id)"
        case .unknown: return "No current program report or shared Available."
        case .unavailable(let reason): return "Current program status unavailable: \(reason)"
        }
    }
}

extension TaskRowStatus {
    init(task: RoadmapTask, status: SessionProgramStatus) {
        self.init(task: task, program: status.row)
    }
}

extension TaskRow where Trailing == EmptyView {
    /// A Portfolio row has planning and no Session readings.
    init(task: RoadmapTask, context: String? = nil) {
        self.init(id: task.id, title: task.task.name, identifier: task.task.identifier, link: task.reference.issueUrl,
                  status: TaskRowStatus(task: task), context: context)
    }
}

/// The ⌘N hint at a row's trailing edge: shown while ⌘ is held, else while the row is hovered.
struct TaskRowSlotHint: View {
    let hint: String?
    let shown: Bool
    let hover: HoverFlag?

    var body: some View {
        if let hint {
            if let hover, !shown {
                HoverRevealed(flag: hover) { Text(hint) }
            } else {
                Text(hint)
            }
        }
    }
}

extension TaskRow where Trailing == TaskRowSlotHint {
    /// The Mac sidebar's row, from what the Work model and its Sessions hold.
    init(id: String, presentation: WorkModel.TaskPresentation, task: RoadmapTask?, status: SessionProgramStatus,
         graphStep: String? = nil, auto: Bool = false, selected: Bool = false, muted: Bool = false,
         slotHint: String? = nil, showsSlotHint: Bool = false, hover: HoverFlag? = nil) {
        let context = [graphStep, presentation.context ?? (task == nil ? "Loading planning…" : nil)]
            .compactMap { $0 }.joined(separator: " · ")
        self.init(id: id, title: presentation.title, identifier: task?.task.identifier ?? presentation.identifier,
                  link: task?.reference.issueUrl,
                  status: task.map { TaskRowStatus(task: $0, status: status) } ?? status.row, auto: auto,
                  context: context.isEmpty ? nil : context,
                  selected: selected, muted: muted) {
            TaskRowSlotHint(hint: slotHint, shown: showsSlotHint, hover: hover)
        }
    }
}


/// Existing provider and terminal metadata, with no process-name guessing.
struct PaneProgramHeading: Equatable {
    let name: String
    let context: String
    let detail: String

    init(provider: String?, title: String, directory: String?, fallback: String) {
        switch provider?.lowercased() {
        case "codex": name = "Codex"
        case "claude": name = "Claude"
        case "pi": name = "Pi"
        case "opencode": name = "OpenCode"
        case "agy": name = "Antigravity"
        case .some(let value) where !value.isEmpty: name = ProgramStatusReport.displaySafe(value)
        default: name = fallback
        }
        let title = ProgramStatusReport.displaySafe(title)
        let folder = directory.map { URL(fileURLWithPath: $0).lastPathComponent }
        context = [title == name ? nil : title, folder].compactMap { $0 }.filter { !$0.isEmpty }.joined(separator: " · ")
        detail = [title, directory].compactMap { $0 }.joined(separator: "\n")
    }
}
