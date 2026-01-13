// First-run setup view for installing dependencies.

import SwiftUI

struct SetupView: View {
    @Binding var isComplete: Bool

    @State private var status: SetupService.DependencyStatus?
    @State private var isInstalling = false
    @State private var installStep: InstallStep = .checking
    @State private var errorMessage: String?

    private let setupService = SetupService()

    enum InstallStep {
        case checking
        case needsLf
        case installingLf
        case needsWt
        case installingWt
        case complete
        case failed
    }

    var body: some View {
        VStack(spacing: 32) {
            // Header
            VStack(spacing: 8) {
                Text("Loopflow Maestro")
                    .font(.largeTitle)
                    .fontWeight(.bold)

                Text("First-time setup")
                    .font(.title3)
                    .foregroundStyle(.secondary)
            }

            Spacer()

            // Status
            VStack(spacing: 24) {
                statusRow(
                    title: "Loopflow CLI",
                    description: "Core command-line tool",
                    isInstalled: status?.lfInstalled ?? false,
                    isInstalling: installStep == .installingLf
                )

                statusRow(
                    title: "Worktrunk (wt)",
                    description: "Git worktree manager",
                    isInstalled: status?.wtInstalled ?? false,
                    isInstalling: installStep == .installingWt
                )
            }
            .padding(.horizontal, 40)

            Spacer()

            // Error message
            if let error = errorMessage {
                Text(error)
                    .font(.callout)
                    .foregroundStyle(.red)
                    .multilineTextAlignment(.center)
                    .padding(.horizontal)
            }

            // Action button
            actionButton
        }
        .padding(40)
        .frame(width: 500, height: 400)
        .onAppear {
            checkStatus()
        }
    }

    private func statusRow(title: String, description: String, isInstalled: Bool, isInstalling: Bool) -> some View {
        HStack(spacing: 16) {
            if isInstalling {
                ProgressView()
                    .scaleEffect(0.8)
                    .frame(width: 24, height: 24)
            } else if isInstalled {
                Image(systemName: "checkmark.circle.fill")
                    .font(.title2)
                    .foregroundStyle(.green)
            } else {
                Image(systemName: "circle")
                    .font(.title2)
                    .foregroundStyle(.secondary)
            }

            VStack(alignment: .leading, spacing: 2) {
                Text(title)
                    .fontWeight(.medium)
                Text(description)
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }

            Spacer()

            if isInstalled, let path = (title == "Loopflow CLI" ? status?.lfPath : status?.wtPath) {
                Text(path)
                    .font(.caption)
                    .foregroundStyle(.tertiary)
            }
        }
    }

    @ViewBuilder
    private var actionButton: some View {
        switch installStep {
        case .checking:
            ProgressView("Checking dependencies...")

        case .needsLf:
            Button("Install Loopflow") {
                Task { await installLf() }
            }
            .buttonStyle(.borderedProminent)
            .controlSize(.large)

        case .installingLf:
            Button("Installing...") {}
                .buttonStyle(.borderedProminent)
                .controlSize(.large)
                .disabled(true)

        case .needsWt:
            Button("Install Worktrunk") {
                Task { await installWt() }
            }
            .buttonStyle(.borderedProminent)
            .controlSize(.large)

        case .installingWt:
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
                checkStatus()
            }
            .buttonStyle(.borderedProminent)
            .controlSize(.large)
        }
    }

    private func checkStatus() {
        installStep = .checking
        errorMessage = nil

        Task {
            try? await Task.sleep(for: .milliseconds(500))

            status = setupService.checkDependencies()

            if status?.allInstalled == true {
                installStep = .complete
            } else if status?.lfInstalled == false {
                installStep = .needsLf
            } else {
                installStep = .needsWt
            }
        }
    }

    private func installLf() async {
        installStep = .installingLf
        errorMessage = nil

        do {
            try await setupService.installLoopflow()
            status = setupService.checkDependencies()

            if status?.lfInstalled == true {
                if status?.wtInstalled == true {
                    installStep = .complete
                } else {
                    installStep = .needsWt
                }
            } else {
                errorMessage = "Installation completed but lf not found. Try running: pip install loopflow"
                installStep = .failed
            }
        } catch {
            errorMessage = "Failed to install: \(error.localizedDescription)"
            installStep = .failed
        }
    }

    private func installWt() async {
        installStep = .installingWt
        errorMessage = nil

        do {
            try await setupService.installWt()
            status = setupService.checkDependencies()

            if status?.wtInstalled == true {
                installStep = .complete
            } else {
                errorMessage = "Installation completed but wt not found. Try running: lf ops install"
                installStep = .failed
            }
        } catch {
            errorMessage = "Failed to install: \(error.localizedDescription)"
            installStep = .failed
        }
    }
}

#Preview {
    SetupView(isComplete: .constant(false))
}
