import Foundation
import Loopflow
import SwiftUI

struct TaskHistoryFilter: Equatable {
    var showCompleted = false
    var allTime = false
    var daysText = "7"
    private(set) var days = 7

    var validationMessage: String? {
        guard let value = Int(daysText), value > 0 else { return "Enter a positive number of days." }
        return nil
    }

    mutating func editDays(_ text: String) {
        daysText = text
        if let value = Int(text), value > 0 { days = value }
    }

    static func hasUnresolvedExecution(runtime: TaskRuntimeSnapshot?, condition: TaskConditionSnapshot, flow: TaskFlowSnapshot) -> Bool {
        guard let runtime else { return false }
        if runtime.status == .ready { return true }
        if case .pinned(let pinned) = flow.record, pinned.execution != .idle { return true }
        // Removed historical checkouts do not reopen settled work. Only an
        // observation of existing unsettled files/commits keeps it current.
        return condition.localProgress.state == .observed && condition.localProgress.unsettled == true
    }

    func includes(_ task: TaskPlanningSnapshot, runtime: TaskRuntimeSnapshot?, condition: TaskConditionSnapshot, flow: TaskFlowSnapshot, now: Date) -> Bool {
        if !task.isTerminal || Self.hasUnresolvedExecution(runtime: runtime, condition: condition, flow: flow) { return true }
        guard task.isSuccessful, showCompleted else { return false }
        if allTime { return true }
        guard let timestamp = task.completedAt, let date = Self.completionDate(timestamp) else { return false }
        return date <= now && date >= now.addingTimeInterval(-Double(days) * 86_400)
    }

    private static func completionDate(_ timestamp: String) -> Date? {
        if let date = try? Date.ISO8601FormatStyle(includingFractionalSeconds: true).parse(timestamp) { return date }
        return try? Date.ISO8601FormatStyle().parse(timestamp)
    }
}

struct TaskHistoryControls: View {
    @Binding var filter: TaskHistoryFilter

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            Toggle("Show completed", isOn: $filter.showCompleted)
                .accessibilityIdentifier("task-history-show")
            if filter.showCompleted {
                HStack {
                    Toggle("All time", isOn: $filter.allTime)
                        .accessibilityIdentifier("task-history-all")
                    if !filter.allTime {
                        Text("Last")
                        TextField("Days", text: Binding(get: { filter.daysText }, set: { filter.editDays($0) }))
                            .frame(width: 65)
                            .accessibilityIdentifier("task-history-days")
                        Text("days")
                    }
                }
                if !filter.allTime, let message = filter.validationMessage {
                    Text(message).foregroundStyle(Color.statusWarning)
                }
            }
        }
    }
}
