// First-run setup view for installing loopflow.

import SwiftUI
import LoopflowCore

struct SetupView: View {
    @Binding var isComplete: Bool

    @Environment(\.palette) private var palette
    @State private var status: SetupService.DependencyStatus?
    @State private var installStep: InstallStep = .checking
    @State private var errorMessage: String?

    private let setupService = SetupService()

    enum InstallStep {
        case checking
        case needsInstall
        case installing
        case complete
        case failed
    }

    var body: some View {
        VStack(spacing: 32) {
            // Header
            VStack(spacing: 8) {
                Text("Loopflow Concerto")
                    .font(Typography.heroTitle())
                    .fontWeight(.bold)

                Text("First-time setup")
                    .font(Typography.sectionTitle())
                    .foregroundStyle(palette.textSecondary)
            }

            Spacer()

            // Status
            VStack(spacing: 16) {
                if installStep == .installing {
                    ProgressView()
                        .scaleEffect(1.5)
                    Text("Installing loopflow...")
                        .foregroundStyle(palette.textSecondary)
                } else if installStep == .complete {
                    Image(systemName: "checkmark.circle.fill")
                        .font(Typography.heroTitle(48))
                        .foregroundStyle(Color.statusSuccess)
                    Text("Ready to go")
                        .font(Typography.sectionTitle())
                } else if let path = status?.lfPath {
                    Image(systemName: "checkmark.circle.fill")
                        .font(Typography.heroTitle(48))
                        .foregroundStyle(Color.statusSuccess)
                    VStack(spacing: 4) {
                        Text("Loopflow installed")
                            .font(Typography.sectionTitle())
                        Text(path)
                            .font(Typography.caption())
                            .foregroundStyle(palette.textSecondary)
                    }
                } else if installStep != .checking {
                    Image(systemName: "arrow.down.circle")
                        .font(Typography.heroTitle(48))
                        .foregroundStyle(palette.textSecondary)
                    Text("Loopflow CLI not found")
                        .font(Typography.sectionTitle())
                }
            }

            Spacer()

            // Error message
            if let error = errorMessage {
                Text(error)
                    .font(Typography.body())
                    .foregroundStyle(Color.statusError)
                    .multilineTextAlignment(.center)
                    .padding(.horizontal)
            }

            // Action buttons
            VStack(spacing: 12) {
                actionButton

                if installStep == .needsInstall || installStep == .failed {
                    Button("Skip, I'll install manually") {
                        isComplete = true
                    }
                    .buttonStyle(.plain)
                    .font(Typography.caption())
                    .foregroundStyle(palette.textSecondary)
                }
            }
        }
        .padding(Spacing.xxxl + Spacing.sm)
        .frame(width: 500, height: 400)
        .onAppear {
            checkStatus()
        }
    }

    @ViewBuilder
    private var actionButton: some View {
        switch installStep {
        case .checking:
            ProgressView("Checking...")

        case .needsInstall:
            Button("Install Loopflow") {
                Task { await install() }
            }
            .buttonStyle(.borderedProminent)
            .controlSize(.large)

        case .installing:
            Button("Installing...") {}
                .buttonStyle(.borderedProminent)
                .controlSize(.large)
                .disabled(true)

        case .complete:
            Button("Get Started") {
                isComplete = true
            }
            .buttonStyle(.borderedProminent)
            .controlSize(.large)

        case .failed:
            Button("Retry") {
                Task { await install() }
            }
            .buttonStyle(.borderedProminent)
            .controlSize(.large)
        }
    }

    private func checkStatus() {
        installStep = .checking
        errorMessage = nil

        Task {
            try? await Task.sleep(for: .milliseconds(300))
            status = setupService.checkDependencies()

            if status?.lfInstalled == true {
                installStep = .complete
            } else {
                installStep = .needsInstall
            }
        }
    }

    private func install() async {
        installStep = .installing
        errorMessage = nil

        do {
            try await setupService.install()
            status = setupService.checkDependencies()

            if status?.lfInstalled == true {
                installStep = .complete
            } else {
                errorMessage = "Installation completed but lf not found.\n\nTry running: uv tool install loopflow"
                installStep = .failed
            }
        } catch {
            errorMessage = error.localizedDescription
            installStep = .failed
        }
    }
}

#Preview {
    SetupView(isComplete: .constant(false))
}

