import Foundation
import Loopflow
import SwiftUI

/// Complete Task-attributed Session history, disclosed on demand. The list comes from the
/// shared `lf runs --task` reader; loading begins on expansion. Provider outcomes
/// remain separate from recorded input completion and command exit.
struct TaskRunsView: View {
    let model: PodiumModel
    let task: RoadmapTask
    let wave: WaveSnapshot
    @Environment(\.palette) private var palette

    private var reading: PodiumReading<[SessionHistory]> { model.recentRuns[task.id] }
    private var runs: [SessionHistory]? { reading.value }
    private var expanded: Bool { model.navigation.expandedRuns.contains(task.id) }
    private var inFlight: Bool { model.recentRuns.inFlight.contains(task.id) }

    var body: some View {
        VStack(alignment: .leading, spacing: Spacing.sm) {
            WorkspaceDisclosureHeading(
                title: "Session history",
                count: expanded ? runs?.count : nil,
                expanded: expanded,
                inFlight: inFlight,
                failure: expanded && reading.errorMessage != nil ? runs != nil : nil,
                identifier: "task-runs",
                accessibilityLabel: "Recent runs, \(expanded ? "expanded" : "collapsed")",
                help: "Complete recorded Session history for \(task.task.identifier)",
                toggle: {
                    let navigation = model.navigation
                    if navigation.expandedRuns.remove(task.id) == nil {
                        navigation.expandedRuns.insert(task.id)
                        Task { await model.loadRecentRuns(task: task, wave: wave) }
                    }
                },
                retry: { Task { await model.loadRecentRuns(task: task, wave: wave) } }
            )
            if expanded {
                if let runs { list(runs) } else { statusText }
            }
        }
    }


    @ViewBuilder
    private var statusText: some View {
        Text(reading.errorMessage.map { "History could not be read: \($0)" } ?? "Reading history…")
            .font(Typography.body(12))
            .foregroundStyle(palette.textSecondary)
            .textSelection(.enabled)
            .accessibilityIdentifier("task-runs-status")
    }

    private func list(_ runs: [SessionHistory]) -> some View {
        VStack(alignment: .leading, spacing: 0) {
            if let reason = reading.errorMessage {
                Text("Latest read failed: \(reason)")
                    .font(Typography.body(12))
                    .foregroundStyle(palette.textSecondary)
                    .textSelection(.enabled)
                    .padding(.bottom, Spacing.xs)
                    .accessibilityIdentifier("task-runs-status")
            }
            if runs.isEmpty {
                Text("No Session history recorded for this Task.")
                    .font(Typography.body(12))
                    .foregroundStyle(palette.textSecondary)
                    .accessibilityIdentifier("task-runs-empty")
            }
            ForEach(runs) { run in
                HStack(alignment: .firstTextBaseline, spacing: Spacing.sm) {
                    Text(run.skill ?? run.harness)
                        .font(Typography.code(11.5))
                        .foregroundStyle(palette.text)
                    Text(Self.agent(run))
                        .font(Typography.caption(11))
                        .foregroundStyle(palette.textSecondary)
                    Spacer(minLength: Spacing.sm)
                    Text(Self.started(run.observedAt))
                        .font(Typography.caption(11))
                        .foregroundStyle(palette.textTertiary)
                    Text(run.status)
                        .font(Typography.caption(11))
                        .foregroundStyle(palette.textSecondary)
                        .frame(minWidth: 72, alignment: .trailing)
                    Text(Self.shortId(run.id))
                        .font(Typography.code(10.5))
                        .foregroundStyle(palette.textTertiary)
                        .textSelection(.enabled)
                        .help(run.id)
                }
                .padding(.vertical, 5)
                .accessibilityElement(children: .combine)
                .accessibilityIdentifier("task-run-\(run.id)")
            }
        }
        .accessibilityIdentifier("task-runs-list")
    }

    static func agent(_ run: SessionHistory) -> String {
        run.model.map { "\(run.harness):\($0)" } ?? run.harness
    }

    static func started(_ unix: Int) -> String {
        Date(timeIntervalSince1970: TimeInterval(unix)).formatted(date: .abbreviated, time: .shortened)
    }

    static func shortId(_ id: String) -> String {
        let body = id.hasPrefix("run_") ? String(id.dropFirst(4)) : id
        return String(body.prefix(8))
    }
}
