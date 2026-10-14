import Loopflow
import SwiftUI

/// Presentation of shared Session evidence; never Task completion or Workflow state.
enum TaskRowProgramStatus {
    case reported(SessionRecord, ProgramStatusReport)
    case waiting(SessionRecord)
    case active(SessionRecord)
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
        let nonreporters = ordered.filter { $0.programStatus?.seen != true && $0.state != .closed }
        if let waiting = nonreporters.first(where: { $0.attention == .waiting }) {
            self = .waiting(waiting)
        } else if let active = nonreporters.first(where: { $0.state == .active }) {
            self = .active(active)
        } else { self = .unknown }
    }

    var label: String {
        switch self {
        case .reported(_, let report):
            let kind = report.kind.map { " · \($0.rawValue.capitalized)" } ?? ""
            let progress = report.progress.map { " · \($0)%" } ?? ""
            return "\(report.state.rawValue.capitalized)\(kind)\(progress)"
        case .waiting: return "Waiting · inferred"
        case .active: return "Active · inferred"
        case .unknown: return "Status unknown"
        case .unavailable: return "Status unavailable"
        }
    }

    var help: String {
        switch self {
        case .reported(let session, let report):
            return "Program report · \(ProgramStatusReport.displaySafe(session.title)) (\(session.id))\n\(report.displayText)"
        case .waiting(let session): return "Inferred from shared Session Waiting · \(session.id)"
        case .active(let session): return "Inferred from a live Session client · \(session.id). Activity does not establish program progress."
        case .unknown: return "No current program report or runtime observation."
        case .unavailable(let reason): return "Current program status unavailable: \(reason)"
        }
    }

    var tone: WorkTone {
        switch self {
        case .reported(_, let report):
            switch report.state {
            case .blocked: .human
            case .error: .blocked
            case .working: .running
            case .done: .done
            case .idle, .clear: .neutral
            }
        case .waiting: .human
        case .active: .running
        case .unknown, .unavailable: .neutral
        }
    }
}

extension TaskWorktreeSnapshot {
    func sidebarMachine(localMachineId: String?) -> String? {
        guard let machineId, let localMachineId else { return "Machine unknown" }
        guard machineId != localMachineId else { return nil }
        guard let machineLabel, !machineLabel.isEmpty else { return "Remote · name unavailable" }
        return ProgramStatusReport.displaySafe(machineLabel)
    }
}

struct TaskRowMetadata: View {
    let task: RoadmapTask
    let status: TaskRowProgramStatus
    let localMachineId: String?
    @Environment(\.palette) private var palette

    var body: some View {
        HStack(spacing: 5) {
            Text(verbatim: status.label)
                .foregroundStyle(status.tone.ink)
                .help(Text(verbatim: status.help))
                .accessibilityIdentifier("work-program-status-\(task.id)")
            if let url = task.reference.issueUrl {
                destination(task.task.identifier, url: url, kind: "task")
            }
            if let pr = task.pr?.publication?.github {
                destination("#\(pr.number)", url: pr.url, kind: "pr")
            }
            // Only recorded checkout ownership is shown; assignment is not execution.
            if let workspace = task.reference.workspace {
                if let machine = workspace.sidebarMachine(localMachineId: localMachineId) {
                    Text(verbatim: machine).help(Text(verbatim: "Recorded checkout Machine · \(machine)"))
                        .accessibilityIdentifier("work-machine-\(task.id)")
                }
            } else if task.runtime?.started == true {
                Text("Machine unknown").accessibilityIdentifier("work-machine-\(task.id)")
            }
        }
        .font(Typography.body(10))
        .foregroundStyle(palette.textSecondary)
        .lineLimit(1)
    }

    private func destination(_ title: String, url: URL, kind: String) -> some View {
        // A sibling control to the title button: external links never select Work.
        Link(destination: url) { Text(verbatim: title) }
            .buttonStyle(.plain)
            .foregroundStyle(palette.accentInk)
            .help(url.absoluteString)
            .accessibilityLabel("Open \(kind == "pr" ? "PR" : "Task") \(title)")
            .accessibilityIdentifier("work-\(kind)-link-\(task.id)")
            .fixedSize()
    }
}
