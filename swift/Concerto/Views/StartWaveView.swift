// Landing view shown when no wave is selected.
// Collects a wave name, creates a wave, and auto-launches a step (default: design).

import SwiftUI
import LoopflowCore

struct StartWaveView: View {
    private enum InteractiveStep: String, CaseIterable {
        case design
        case explore
        case review
        case refine

        var launchLabel: String {
            switch self {
            case .design: "Start designing"
            case .explore: "Start exploring"
            case .review: "Start reviewing"
            case .refine: "Start refining"
            }
        }
    }

    @Environment(RepoState.self) private var repoState
    @Environment(OutputBuffer.self) private var outputBuffer
    @Environment(\.palette) private var palette
    @State private var waveName = ""
    @State private var selectedStep: InteractiveStep = .design
    @State private var isCreating = false
    @State private var errorMessage: String?
    @FocusState private var isTextFieldFocused: Bool

    var body: some View {
        VStack(spacing: Spacing.xxl) {
            Spacer()

            VStack(spacing: Spacing.lg) {
                Text("Start a wave")
                    .font(Typography.heroTitle())
                    .foregroundStyle(palette.accent)

                TextField("Wave name", text: $waveName)
                    .textFieldStyle(.plain)
                    .font(Typography.body())
                    .padding(Spacing.md)
                    .background(palette.surfaceMuted)
                    .clipShape(RoundedRectangle(cornerRadius: CornerRadius.md))
                    .frame(maxWidth: 400)
                    .focused($isTextFieldFocused)
                    .onSubmit { startWave() }
                    .disabled(isCreating)

                HStack(spacing: Spacing.sm) {
                    Button {
                        startWave()
                    } label: {
                        HStack(spacing: Spacing.xs) {
                            if isCreating {
                                ProgressView()
                                    .scaleEffect(0.6)
                            }
                            Text(buttonLabel)
                        }
                    }
                    .buttonStyle(DarkButtonStyle())
                    .disabled(isCreating || trimmedName.isEmpty)

                    Menu {
                        ForEach(InteractiveStep.allCases, id: \.rawValue) { step in
                            Button {
                                selectedStep = step
                            } label: {
                                HStack {
                                    Text(step.rawValue)
                                    if step == selectedStep {
                                        Image(systemName: "checkmark")
                                    }
                                }
                            }
                        }
                    } label: {
                        Image(systemName: "chevron.down")
                            .font(Typography.caption())
                            .foregroundStyle(palette.textSecondary)
                            .frame(width: HitTarget.comfortable, height: HitTarget.comfortable)
                            .contentShape(Rectangle())
                    }
                    .buttonStyle(.plain)
                    .help("Choose which step to run")
                }

                if let errorMessage {
                    Text(errorMessage)
                        .font(Typography.caption())
                        .foregroundStyle(Color.statusError)
                }
            }

            Spacer()
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .onAppear { isTextFieldFocused = true }
    }

    private var trimmedName: String {
        waveName.trimmingCharacters(in: .whitespacesAndNewlines)
    }

    private var buttonLabel: String {
        selectedStep.launchLabel
    }

    private func startWave() {
        guard !isCreating, !trimmedName.isEmpty else { return }

        isCreating = true
        errorMessage = nil
        let name = trimmedName
        let step = selectedStep.rawValue

        Task {
            defer { isCreating = false }
            do {
                let wave = try await repoState.createWave(name: name)
                waveName = ""

                guard let worktree = wave.localWorktree else {
                    errorMessage = "Wave created but no worktree available."
                    return
                }

                outputBuffer.launchInteractiveSession(
                    waveId: wave.id,
                    step: step,
                    worktreePath: worktree
                )
            } catch {
                errorMessage = error.localizedDescription
            }
        }
    }
}

#Preview {
    StartWaveView()
        .environment(RepoState())
        .environment(OutputBuffer())
        .frame(width: 600, height: 500)
}
