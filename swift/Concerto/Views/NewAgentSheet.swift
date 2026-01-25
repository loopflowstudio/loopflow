// Sheet for creating a new agent with optional name.

import SwiftUI
import LoopflowCore

struct NewAgentSheet: View {
    @Bindable var appState: AppState
    @Environment(\.dismiss) private var dismiss

    @State private var name = ""
    @State private var isCreating = false
    @State private var errorMessage: String?
    @FocusState private var isNameFocused: Bool

    var body: some View {
        VStack(spacing: 20) {
            Text("New Agent")
                .font(.headline)

            VStack(alignment: .leading, spacing: 6) {
                HStack {
                    Text("Name")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                    Text("(optional)")
                        .font(.caption2)
                        .foregroundStyle(.tertiary)
                }

                TextField("Auto-generates if empty", text: $name)
                    .textFieldStyle(.roundedBorder)
                    .focused($isNameFocused)
                    .onSubmit {
                        createAgent()
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

                Button("Create Agent") {
                    createAgent()
                }
                .buttonStyle(.borderedProminent)
                .keyboardShortcut(.defaultAction)
                .disabled(isCreating)
            }
        }
        .padding(24)
        .frame(width: 320)
        .onAppear {
            isNameFocused = true
        }
    }

    private func createAgent() {
        isCreating = true
        errorMessage = nil

        Task {
            do {
                try await appState.createAgent(name: name)
                dismiss()
            } catch {
                errorMessage = error.localizedDescription
            }
            isCreating = false
        }
    }
}

#Preview {
    NewAgentSheet(appState: AppState())
}
