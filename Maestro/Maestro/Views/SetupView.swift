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

            // Progress indicator
            HStack(spacing: 0) {
                ForEach(0..<3, id: \.self) { index in
                    Circle()
                        .fill(stepFillColor(for: index))
                        .frame(width: 10, height: 10)

                    if index < 2 {
                        Rectangle()
                            .fill(index < currentStepIndex ? Color.accentColor : Color.secondary.opacity(0.3))
                            .frame(height: 2)
                            .frame(maxWidth: 40)
                    }
                }
            }
            .padding(.top, 8)

            Spacer()

            // Status
            VStack(spacing: 24) {
                statusRow(
                    title: "Loopflow CLI",
                    description: "Runs AI coding tasks from your terminal",
                    isInstalled: status?.lfInstalled ?? false,
                    isInstalling: installStep == .installingLf
                )

                statusRow(
                    title: "Worktrunk (wt)",
                    description: "Keeps each feature in its own folder",
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

            // Action buttons
            VStack(spacing: 12) {
                actionButton

                // Skip option for manual installation
                if installStep != .complete && installStep != .checking {
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

    private var currentStepIndex: Int {
        switch installStep {
        case .checking, .needsLf, .installingLf:
            return 0
        case .needsWt, .installingWt:
            return 1
        case .complete:
            return 2
        case .failed:
            return status?.lfInstalled == true ? 1 : 0
        }
    }

    private func stepFillColor(for index: Int) -> Color {
        if index < currentStepIndex {
            return .accentColor
        } else if index == currentStepIndex {
            switch installStep {
            case .installingLf, .installingWt:
                return .accentColor.opacity(0.5)
            case .failed:
                return .red
            default:
                return .accentColor
            }
        }
        return .secondary.opacity(0.3)
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
                errorMessage = "Installation completed but lf not found.\n\nTry opening Terminal and running:\n  pip install loopflow\n\nThen click Retry."
                installStep = .failed
            }
        } catch {
            errorMessage = "Failed to install Loopflow.\n\n\(error.localizedDescription)\n\nMake sure Python and pip are installed, then click Retry."
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
                errorMessage = "Installation completed but wt not found.\n\nTry opening Terminal and running:\n  lfops install\n\nThen click Retry."
                installStep = .failed
            }
        } catch {
            errorMessage = "Failed to install worktrunk.\n\n\(error.localizedDescription)\n\nMake sure Loopflow is installed correctly, then click Retry."
            installStep = .failed
        }
    }
}

#Preview {
    SetupView(isComplete: .constant(false))
}
