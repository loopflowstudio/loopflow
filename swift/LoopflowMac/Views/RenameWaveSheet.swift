// Rename Wave: `lf wave rename <id> <new-name>`. The Wave keeps its id, Tasks and
// chapters, so selection stays on it. A failure keeps the sheet open with the
// error; a rename with leftovers (an Initiative that kept its title, prose that
// still names the old address) keeps it open showing the receipt's lines.

import SwiftUI
import Loopflow

enum RenameWavePhase: Equatable {
    case editing(error: String?)
    case renaming
    /// Renamed, with `summary` lines a person still has to act on.
    case leftovers(WaveRenamed)
}

/// The sheet's content for one phase; the owner runs the command.
struct RenameWaveSheetContent: View {
    let currentName: String
    @Binding var newName: String
    let phase: RenameWavePhase
    let onRename: () -> Void
    let onDone: () -> Void
    @Environment(\.palette) private var palette

    private var trimmed: String { newName.trimmingCharacters(in: .whitespacesAndNewlines) }

    var body: some View {
        VStack(alignment: .leading, spacing: Spacing.lg) {
            switch phase {
            case .leftovers(let renamed):
                Text("Renamed \(renamed.from) → \(renamed.to)")
                    .font(Typography.sectionTitle(20))
                    .foregroundStyle(palette.text)
                VStack(alignment: .leading, spacing: Spacing.xs) {
                    ForEach(Array(renamed.summary.enumerated()), id: \.offset) { _, line in
                        Text(line).font(Typography.body()).foregroundStyle(palette.textSecondary)
                    }
                }.accessibilityIdentifier("rename-wave-summary")
                HStack(spacing: Spacing.md) {
                    Spacer()
                    Button("Done", action: onDone)
                        .keyboardShortcut(.defaultAction)
                        .accessibilityIdentifier("rename-wave-done")
                }
            case .editing, .renaming:
                Text("Rename \(currentName)")
                    .font(Typography.sectionTitle(20))
                    .foregroundStyle(palette.text)
                TextField("Wave name", text: $newName)
                    .textFieldStyle(.roundedBorder)
                    .onSubmit(onRename)
                    .disabled(phase == .renaming)
                    .accessibilityIdentifier("rename-wave-name")
                if case .editing(let error?) = phase {
                    Text(error)
                        .font(Typography.caption())
                        .foregroundStyle(palette.failure)
                        .accessibilityIdentifier("rename-wave-error")
                }
                HStack(spacing: Spacing.md) {
                    Spacer()
                    Button("Cancel", action: onDone).buttonStyle(.plain)
                        .foregroundStyle(palette.textSecondary)
                    Button("Rename", action: onRename)
                        .keyboardShortcut(.defaultAction)
                        .disabled(phase == .renaming || trimmed.isEmpty || trimmed == currentName)
                        .accessibilityIdentifier("rename-wave-confirm")
                }
            }
        }
        .padding(Spacing.xl)
        .frame(width: 440)
    }
}

/// Owns the rename command for one Wave.
struct RenameWaveSheet: View {
    let model: WorkModel
    let id: String
    let currentName: String
    let repo: String
    let onDone: () -> Void
    @State private var newName: String
    @State private var phase: RenameWavePhase = .editing(error: nil)

    init(model: WorkModel, id: String, currentName: String, repo: String, onDone: @escaping () -> Void) {
        self.model = model
        self.id = id
        self.currentName = currentName
        self.repo = repo
        self.onDone = onDone
        _newName = State(initialValue: currentName)
    }

    var body: some View {
        RenameWaveSheetContent(
            currentName: currentName, newName: $newName, phase: phase, onRename: rename, onDone: onDone)
    }

    private func rename() {
        let name = newName.trimmingCharacters(in: .whitespacesAndNewlines)
        guard case .editing = phase, !name.isEmpty, name != currentName else { return }
        phase = .renaming
        Task {
            do {
                let renamed = try await model.renameWave(id: id, newName: name, repo: repo)
                if renamed.summary.isEmpty {
                    onDone()
                } else {
                    phase = .leftovers(renamed)
                }
            } catch let failure {
                phase = .editing(error: failure.localizedDescription)
            }
        }
    }
}
