import Loopflow
import SwiftUI

struct TaskAutomationView: View {
    let query: RegistryQuery
    let repo: String
    @State private var status: AutomationStatus?
    @State private var error: String?
    @State private var busy = false

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("Background progress").font(.headline)
            if let status {
                HStack {
                    Text(status.coverage.capitalized)
                    Spacer()
                    Button(status.enabled ? "Disable" : "Enable") {
                        perform { try await query.setAutomation(enabled: !status.enabled, cwd: repo) }
                    }
                }
                Text("Checks every minute while this Home is available. Running work continues when disabled.")
                    .font(.caption).foregroundStyle(.secondary)
                if let checked = status.lastSuccessAt {
                    Text("Last successful check: \(Date(timeIntervalSince1970: Double(checked)), style: .relative) ago")
                        .font(.caption)
                }
                if let lastError = status.lastError {
                    Text("Last failed check: \(lastError)").font(.caption).foregroundStyle(.secondary)
                }
                ForEach(status.tasks) { task in
                    VStack(alignment: .leading, spacing: 4) {
                        HStack {
                            Text(task.issue)
                            Spacer()
                            Button(task.enabled == true ? "Hold" : "Enroll") {
                                perform { try await query.automateTask(issue: task.issue, enabled: task.enabled != true, cwd: repo) }
                            }
                        }
                        if let detail = task.detail { Text(detail).font(.caption).foregroundStyle(.secondary) }
                    }
                }
                ForEach(status.deliveries, id: \.prNumber) { delivery in
                    Text("PR #\(delivery.prNumber): \(delivery.detail ?? delivery.state)")
                        .font(.caption)
                }
            } else if error == nil {
                ProgressView()
            }
            if let error { Text(error).foregroundStyle(.red).font(.caption) }
            Button("Refresh") { perform {} }
        }
        .padding(20)
        .frame(width: 400)
        .disabled(busy)
        .task(id: repo) { await refresh() }
        .accessibilityIdentifier("task-automation")
    }

    private func perform(_ operation: @escaping () async throws -> Void) {
        Task {
            busy = true
            defer { busy = false }
            do { try await operation(); await refresh() }
            catch { self.error = error.localizedDescription }
        }
    }

    private func refresh() async {
        do { status = try await query.automationStatus(cwd: repo); error = nil }
        catch { self.error = error.localizedDescription }
    }
}
