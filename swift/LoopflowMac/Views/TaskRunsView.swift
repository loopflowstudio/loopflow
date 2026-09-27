import Foundation
import Loopflow
import SwiftUI

/// Recent Runs for one Task, disclosed on demand. The list comes from the
/// shared `lf runs --task` reader for the Task's exact identifier; nothing is
/// read until the human expands it. A Run's recorded outcome is shown as-is:
/// history never claims a Run is live.
struct TaskRunsView: View {
    let model: PodiumModel
    let task: RoadmapTask
    let wave: WaveSnapshot
    @Environment(\.palette) private var palette

    private var reading: PodiumReading<[RunSnapshot]> { model.recentRuns[task.id] }
    private var runs: [RunSnapshot]? { reading.value }
    private var expanded: Bool { model.navigation.expandedRuns.contains(task.id) }
    private var inFlight: Bool { model.recentRuns.inFlight.contains(task.id) }

    var body: some View {
        VStack(alignment: .leading, spacing: Spacing.sm) {
            WorkspaceDisclosureHeading(
                title: "Recent runs",
                count: expanded ? runs?.count : nil,
                expanded: expanded,
                inFlight: inFlight,
                failure: expanded && reading.errorMessage != nil ? runs != nil : nil,
                identifier: "task-runs",
                accessibilityLabel: "Recent runs, \(expanded ? "expanded" : "collapsed")",
                help: "Newest 50 Runs recorded for \(task.task.identifier) in the last 7 days",
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
        Text(reading.errorMessage.map { "Runs could not be read: \($0)" } ?? "Reading runs…")
            .font(Typography.body(12))
            .foregroundStyle(palette.textSecondary)
            .textSelection(.enabled)
            .accessibilityIdentifier("task-runs-status")
    }

    private func list(_ runs: [RunSnapshot]) -> some View {
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
                Text("No Runs recorded for this Task in the last 7 days.")
                    .font(Typography.body(12))
                    .foregroundStyle(palette.textSecondary)
                    .accessibilityIdentifier("task-runs-empty")
            }
            if runs.count == 50 {
                Text("Showing the newest 50 Runs from the last 7 days.")
                    .font(Typography.meta)
                    .foregroundStyle(palette.textTertiary)
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
                    Text(Self.started(run.started))
                        .font(Typography.caption(11))
                        .foregroundStyle(palette.textTertiary)
                    Text(run.outcome ?? "No outcome recorded")
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

    static func agent(_ run: RunSnapshot) -> String {
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
