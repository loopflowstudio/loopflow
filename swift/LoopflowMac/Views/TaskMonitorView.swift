import Loopflow
import SwiftUI

/// Snapshot presentation; Podium owns the shared read for every Monitor pane.
/// Monitor sits in the dark pane surface beside terminals, so it uses the
/// terminal inks rather than the page palette.
struct TaskMonitorView: View {
    let taskId: String
    let model: PodiumModel

    private enum Ink {
        static let text = Color(hex: 0xEDE7DF)
        static let secondary = Color(hex: 0xB9B1A7)
        static let tertiary = Color(hex: 0x8D949C)
        static let rule = Color(hex: 0x434A52)
        static let card = Color(hex: 0x333940)
        static let warning = Color(hex: 0xE3B866)
    }

    var body: some View {
        VStack(alignment: .leading, spacing: Spacing.md) {
            HStack(alignment: .firstTextBaseline, spacing: Spacing.sm) {
                VStack(alignment: .leading, spacing: 2) {
                    Text("Monitor")
                        .textCase(.uppercase)
                        .font(Typography.caption(11).weight(.bold))
                        .tracking(0.66)
                        .foregroundStyle(Ink.tertiary)
                    Text(model.task(id: taskId)?.task.task.name ?? "Task unavailable")
                        .font(Typography.sectionTitle(20))
                        .foregroundStyle(Ink.text)
                        .lineLimit(2)
                }
                Spacer()
                Button(model.activeRunsNeedsRetry ? "Retry" : "Refresh") { Task { await model.refreshActiveRuns() } }
                    .buttonStyle(.plain)
                    .font(Typography.body(12))
                    .foregroundStyle(Ink.text)
                    .padding(.horizontal, 10)
                    .padding(.vertical, 3)
                    .overlay(RoundedRectangle(cornerRadius: 6).strokeBorder(Ink.rule))
                    .accessibilityIdentifier("monitor-refresh-\(taskId)")
            }
            if let snapshot = model.activeRuns.value {
                Text("Observed \(Date(timeIntervalSince1970: TimeInterval(snapshot.observedAt)).formatted(date: .omitted, time: .standard)) · \(model.activeRunsNeedsRetry ? "Updates paused" : "Updates automatically")")
                    .font(Typography.caption(11.5))
                    .foregroundStyle(Ink.tertiary)
            }
            if model.isRefreshingActiveRuns || model.activeRuns.isLoading {
                ProgressView("Reading active Runs…")
                    .controlSize(.small)
                    .foregroundStyle(Ink.secondary)
            }
            if let error = model.activeRuns.errorMessage {
                note("Active Runs unavailable — \(error)")
                if model.activeRuns.value != nil {
                    Text("Showing the last successful observation.")
                        .font(Typography.caption(11.5))
                        .foregroundStyle(Ink.secondary)
                }
            }
            if let error = model.roadmap.errorMessage {
                note("Task planning unavailable — \(error)")
            }
            if let snapshot = model.activeRuns.value {
                if snapshot.discovery != .ready {
                    note(snapshot.discovery == .scanning ? "Discovering active Runs…" : "Active Run discovery unavailable")
                }
                if !snapshot.gaps.isEmpty {
                    DisclosureGroup("Some activity unavailable") {
                        ForEach(snapshot.gaps, id: \.self) {
                            Text($0).font(Typography.caption(11.5)).foregroundStyle(Ink.secondary)
                        }
                    }
                    .font(Typography.body(12.5))
                    .foregroundStyle(Ink.warning)
                }
                if let task = model.task(id: taskId)?.task {
                    let work = task.runtime.map { WorkReference.task(id: $0.workId) }
                    let runs = snapshot.runs.filter { work != nil && $0.work == work }
                    if runs.isEmpty {
                        if snapshot.discovery == .ready && snapshot.gaps.isEmpty && model.activeRuns.errorMessage == nil
                            && model.roadmap.errorMessage == nil {
                            Text("No active Runs in this observation")
                                .font(Typography.body(13))
                                .foregroundStyle(Ink.secondary)
                                .accessibilityIdentifier("monitor-empty-\(taskId)")
                        } else {
                            Text("No matching Runs in the available evidence")
                                .font(Typography.body(13))
                                .foregroundStyle(Ink.secondary)
                        }
                    }
                    ScrollView {
                        LazyVStack(alignment: .leading, spacing: Spacing.sm) {
                            ForEach(runs) { run in
                                runRow(run)
                            }
                        }
                        .frame(maxWidth: .infinity, alignment: .leading)
                    }
                } else {
                    Text("Task planning is unavailable; Run attribution cannot be shown.")
                        .font(Typography.body(13))
                        .foregroundStyle(Ink.secondary)
                }
            }
            Spacer(minLength: 0)
        }
        .padding(16)
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        .foregroundStyle(Ink.text)
        .accessibilityIdentifier("task-monitor-\(taskId)")
    }

    private func note(_ text: String) -> some View {
        Text(text)
            .font(Typography.body(12.5))
            .foregroundStyle(Ink.warning)
            .padding(.leading, Spacing.sm)
            .overlay(alignment: .leading) { Rectangle().fill(Ink.warning).frame(width: 2) }
    }

    private func runRow(_ run: ActiveRun) -> some View {
        VStack(alignment: .leading, spacing: 4) {
            Text(run.label)
                .font(Typography.body(13.5).weight(.bold))
                .foregroundStyle(Ink.text)
            Text(run.id)
                .font(Typography.code(11))
                .foregroundStyle(Ink.tertiary)
                .textSelection(.enabled)
            ForEach(run.processes, id: \.pid) { process in
                HStack(spacing: Spacing.sm) {
                    Text(process.state.rawValue)
                        .font(Typography.caption(10.5).weight(.bold))
                        .foregroundStyle(Color(hex: 0xC9D4E0))
                        .padding(.horizontal, 7)
                        .padding(.vertical, 1)
                        .background(Color(hex: 0x2F6BC0).opacity(0.35), in: RoundedRectangle(cornerRadius: 4))
                    Text("\(process.provider) · PID \(process.pid)")
                        .font(Typography.code(11))
                        .foregroundStyle(Ink.secondary)
                }
            }
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 10)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(Ink.card, in: RoundedRectangle(cornerRadius: 8))
        .accessibilityIdentifier("monitor-run-\(run.id)")
    }
}
