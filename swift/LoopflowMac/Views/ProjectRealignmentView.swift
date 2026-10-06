import SwiftUI
import Loopflow

/// Explicit repository operation; opening a Wave never invokes rotation.
struct ProjectRealignmentView: View {
    let repo: String
    let onApplied: () -> Void
    @Environment(\.dismiss) private var dismiss
    @State private var name = ""
    @State private var plan = ""
    @State private var preview: (input: String, report: ProjectRotationPreview)?
    @State private var errorMessage: String?
    @State private var busy = false
    @State private var applied = false

    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            Text("Realign Projects").font(.title2)
            Text("Review the retained chapter plan before moving started work. Unreviewed backlog stays in its original Project.")
            TextField("Chapter name", text: $name)
                .disabled(busy)
            TextField("Retained plan file path", text: $plan)
                .disabled(busy)
            if let preview {
                ScrollView {
                    VStack(alignment: .leading, spacing: 12) {
                        ForEach(preview.report.waves, id: \.wave) { wave in
                            Text(wave.wave).font(.headline)
                            Text("\(wave.predecessor?.name ?? "No predecessor") → \(wave.successor?.name ?? "New Project")")
                            Text(wave.successor_id).font(.caption).textSelection(.enabled)
                            ForEach(wave.tasks, id: \.task.id) { task in
                                Text("\(task.task.identifier) · \(task.task.name): \(task.disposition) — \(task.reason)")
                            }
                        }
                    }.frame(maxWidth: .infinity, alignment: .leading)
                }
            }
            if let errorMessage { Text(errorMessage).foregroundStyle(.red).textSelection(.enabled) }
            if applied { Text("Projects realigned.") }
            HStack {
                Button("Close") { dismiss() }.disabled(busy)
                Spacer()
                if busy { ProgressView().controlSize(.small) }
                Button(errorMessage != nil && preview == nil ? "Retry preview" : "Preview") { perform(previewOnly: true) }
                    .disabled(busy || name.isEmpty || plan.isEmpty)
                Button(errorMessage != nil && preview != nil ? "Retry apply" : "Apply") { perform(previewOnly: false) }
                    .disabled(busy || preview == nil || applied)
            }
        }
        .padding(24)
        .frame(width: 640, height: 520)
        .onChange(of: name) { preview = nil; applied = false }
        .onChange(of: plan) { preview = nil; applied = false }
    }

    private func perform(previewOnly: Bool) {
        busy = true
        errorMessage = nil
        Task {
            defer { busy = false }
            do {
                let input: String
                if previewOnly {
                    let path = (plan as NSString).expandingTildeInPath
                    input = try await Task.detached {
                        try String(contentsOfFile: path, encoding: .utf8)
                    }.value
                } else {
                    guard let preview else { return }
                    input = preview.input
                }
                let report = try await RegistryQueryLocal.shared.realignProjects(
                    name: name, plan: input, preview: previewOnly, cwd: repo
                )
                preview = (input, report)
                applied = !previewOnly
                if applied { onApplied() }
            } catch {
                // An uncertain apply retries the reviewed bytes, even if the file changed.
                if previewOnly { preview = nil }
                errorMessage = error.localizedDescription
            }
        }
    }
}
