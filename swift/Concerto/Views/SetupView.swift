// First-run setup view for installing loopflow.

import SwiftUI
import LoopflowCore

struct SetupView: View {
    @Binding var isComplete: Bool

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
                    .font(.largeTitle)
                    .fontWeight(.bold)

                Text("First-time setup")
                    .font(.title3)
                    .foregroundStyle(.secondary)
            }

            Spacer()

            // Status
            VStack(spacing: 16) {
                if installStep == .installing {
                    ProgressView()
                        .scaleEffect(1.5)
                    Text("Installing loopflow...")
                        .foregroundStyle(.secondary)
                } else if installStep == .complete {
                    Image(systemName: "checkmark.circle.fill")
                        .font(.system(size: 48))
                        .foregroundStyle(Color.statusSuccess)
                    Text("Ready to go")
                        .font(.headline)
                } else if let path = status?.lfPath {
                    Image(systemName: "checkmark.circle.fill")
                        .font(.system(size: 48))
                        .foregroundStyle(Color.statusSuccess)
                    VStack(spacing: 4) {
                        Text("Loopflow installed")
                            .font(.headline)
                        Text(path)
                            .font(.caption)
                            .foregroundStyle(.tertiary)
                    }
                } else if installStep != .checking {
                    Image(systemName: "arrow.down.circle")
                        .font(.system(size: 48))
                        .foregroundStyle(.secondary)
                    Text("Loopflow CLI not found")
                        .font(.headline)
                }
            }

            Spacer()

            // Error message
            if let error = errorMessage {
                Text(error)
                    .font(.callout)
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
                    .font(.caption)
                    .foregroundStyle(.secondary)
                }
            }
        }
        .padding(40)
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
