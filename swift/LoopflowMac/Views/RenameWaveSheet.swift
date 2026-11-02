// Rename Wave: `lf wave rename <id> <new-name>`. The Wave keeps its id, Tasks and
// chapters, so selection stays on it; a failure keeps the sheet open with the error.

import SwiftUI
import Loopflow

struct RenameWaveSheet: View {
    let model: WorkModel
    let id: String
    let currentName: String
    let repo: String
    let onDone: () -> Void
    @Environment(\.palette) private var palette
    @State private var newName: String
    @State private var renaming = false
    @State private var error: String?

    init(model: WorkModel, id: String, currentName: String, repo: String, onDone: @escaping () -> Void) {
        self.model = model
        self.id = id
        self.currentName = currentName
        self.repo = repo
        self.onDone = onDone
        _newName = State(initialValue: currentName)
    }

    private var trimmed: String { newName.trimmingCharacters(in: .whitespacesAndNewlines) }

    var body: some View {
        VStack(alignment: .leading, spacing: Spacing.lg) {
            Text("Rename \(currentName)")
                .font(Typography.sectionTitle(20))
                .foregroundStyle(palette.text)
            TextField("Wave name", text: $newName)
                .textFieldStyle(.roundedBorder)
                .onSubmit(rename)
                .disabled(renaming)
                .accessibilityIdentifier("rename-wave-name")
            if let error {
                Text(error)
                    .font(Typography.caption())
                    .foregroundStyle(palette.failure)
                    .accessibilityIdentifier("rename-wave-error")
            }
            HStack(spacing: Spacing.md) {
                Spacer()
                Button("Cancel", action: onDone).buttonStyle(.plain)
                    .foregroundStyle(palette.textSecondary)
                Button("Rename", action: rename)
                    .keyboardShortcut(.defaultAction)
                    .disabled(renaming || trimmed.isEmpty || trimmed == currentName)
                    .accessibilityIdentifier("rename-wave-confirm")
            }
        }
        .padding(Spacing.xl)
        .frame(width: 400)
    }

    private func rename() {
        guard !renaming, !trimmed.isEmpty, trimmed != currentName else { return }
        renaming = true
        error = nil
        let name = trimmed
        Task {
            defer { renaming = false }
            do {
                try await model.renameWave(id: id, newName: name, repo: repo)
                onDone()
            } catch let failure {
                error = failure.localizedDescription
            }
        }
    }
}
