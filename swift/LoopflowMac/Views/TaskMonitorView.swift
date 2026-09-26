import Loopflow
import SwiftUI

/// Snapshot presentation; Podium owns the shared read for every Monitor pane.
struct TaskMonitorView: View {
    let taskId: String
    let model: PodiumModel

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack {
                Text(model.task(id: taskId)?.task.task.name ?? "Task unavailable")
                    .font(.headline)
                Spacer()
                Button(model.activeRunsNeedsRetry ? "Retry" : "Refresh") { Task { await model.refreshActiveRuns() } }
                    .accessibilityIdentifier("monitor-refresh-\(taskId)")
            }
            if let snapshot = model.activeRuns.value {
                Text("Observed \(Date(timeIntervalSince1970: TimeInterval(snapshot.observedAt)).formatted(date: .omitted, time: .standard)) · \(model.activeRunsNeedsRetry ? "Updates paused" : "Updates automatically")")
                    .font(.caption).foregroundStyle(.secondary)
            }
            if model.isRefreshingActiveRuns || model.activeRuns.isLoading {
                ProgressView("Reading active Runs…")
            }
            if let error = model.activeRuns.errorMessage {
                Text("Active Runs unavailable — \(error)")
                    .foregroundStyle(Color.statusWarning)
                if model.activeRuns.value != nil {
                    Text("Showing the last successful observation.").font(.caption)
                }
            }
            if let error = model.roadmap.errorMessage {
                Text("Task planning unavailable — \(error)").foregroundStyle(Color.statusWarning)
            }
            if let snapshot = model.activeRuns.value {
                if snapshot.discovery != .ready {
                    Text(snapshot.discovery == .scanning ? "Discovering active Runs…" : "Active Run discovery unavailable")
                        .foregroundStyle(Color.statusWarning)
                }
                if !snapshot.gaps.isEmpty {
                    DisclosureGroup("Some activity unavailable") {
                        ForEach(snapshot.gaps, id: \.self) { Text($0).font(.caption) }
                    }
                    .foregroundStyle(Color.statusWarning)
                }
                if let task = model.task(id: taskId)?.task {
                    let work = task.runtime.map { WorkReference.task(id: $0.workId) }
                    let runs = snapshot.runs.filter { work != nil && $0.work == work }
                    if runs.isEmpty {
                        if snapshot.discovery == .ready && snapshot.gaps.isEmpty && model.activeRuns.errorMessage == nil
                            && model.roadmap.errorMessage == nil {
                            Text("No active Runs in this observation")
                                .accessibilityIdentifier("monitor-empty-\(taskId)")
                        } else {
                            Text("No matching Runs in the available evidence")
                        }
                    }
                    ScrollView {
                        LazyVStack(alignment: .leading, spacing: 14) {
                            ForEach(runs) { run in
                                VStack(alignment: .leading, spacing: 4) {
                                    Text(run.label).font(.body.weight(.medium))
                                    Text(run.id).font(.caption.monospaced()).textSelection(.enabled)
                                    ForEach(run.processes, id: \.pid) { process in
                                        Text("\(process.provider) · \(process.state.rawValue.capitalized) · PID \(process.pid)")
                                            .font(.caption).foregroundStyle(.secondary)
                                    }
                                }
                                .accessibilityIdentifier("monitor-run-\(run.id)")
                            }
                        }
                        .frame(maxWidth: .infinity, alignment: .leading)
                    }
                } else {
                    Text("Task planning is unavailable; Run attribution cannot be shown.")
                }
            }
            Spacer(minLength: 0)
        }
        .padding(16)
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        .foregroundStyle(.white)
        .accessibilityIdentifier("task-monitor-\(taskId)")
    }
}
