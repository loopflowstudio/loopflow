// Sheet for creating a new wave with auto-generated name.

import SwiftUI
import LoopflowCore

struct NewWaveSheet: View {
    @Environment(RepoState.self) private var repoState
    @Environment(\.dismiss) private var dismiss

    @State private var name = NameGenerator.generate()
    @State private var isCreating = false
    @State private var errorMessage: String?
    @FocusState private var isNameFocused: Bool

    var body: some View {
        VStack(spacing: 20) {
            Text("New Wave")
                .font(.headline)

            VStack(alignment: .leading, spacing: 6) {
                Text("Name")
                    .font(.caption)
                    .foregroundStyle(.secondary)

                TextField("wave-name", text: $name)
                    .textFieldStyle(.roundedBorder)
                    .focused($isNameFocused)
                    .onSubmit {
                        createWave()
                    }
            }

            if let error = errorMessage {
                Text(error)
                    .font(.caption)
                    .foregroundStyle(.red)
            }

            HStack {
                Button("Cancel") {
                    dismiss()
                }
                .keyboardShortcut(.escape)

                Spacer()

                Button("Create Wave") {
                    createWave()
                }
                .buttonStyle(.borderedProminent)
                .keyboardShortcut(.defaultAction)
                .disabled(isCreating)
            }
        }
        .padding(24)
        .frame(width: 320)
        .task {
            // Small delay to ensure field is mounted before focusing
            try? await Task.sleep(for: .milliseconds(100))
            isNameFocused = true
        }
    }

    private func createWave() {
        isCreating = true
        errorMessage = nil

        Task {
            do {
                try await repoState.createWave(name: name)
                dismiss()
            } catch {
                errorMessage = error.localizedDescription
            }
            isCreating = false
        }
    }
}

#Preview {
    NewWaveSheet()
        .environment(RepoState())
}
