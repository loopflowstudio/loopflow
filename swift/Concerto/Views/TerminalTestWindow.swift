// Simple test window for the embedded Ghostty terminal.
// Access via Window menu > Terminal Test

import SwiftUI

struct TerminalTestWindow: View {
    @Environment(\.palette) private var palette
    @StateObject private var ghosttyManager = GhosttyManager.shared

    var body: some View {
        VStack(spacing: 0) {
            // Status bar
            HStack {
                Image(systemName: "terminal.fill")
                Text("Ghostty Terminal Test")
                    .font(Typography.sectionTitle())

                Spacer()

                statusIndicator
            }
            .padding()
            .background(.regularMaterial)

            // Terminal or error
            if case .failed(let error) = ghosttyManager.state {
                errorView(error)
            } else {
                terminalView
            }
        }
        .onAppear {
            if case .uninitialized = ghosttyManager.state {
                ghosttyManager.initialize()
            }
        }
    }

    @ViewBuilder
    private var statusIndicator: some View {
        switch ghosttyManager.state {
        case .uninitialized:
            Label("Not initialized", systemImage: "circle")
                .foregroundStyle(palette.textSecondary)
        case .initializing:
            HStack(spacing: 4) {
                ProgressView()
                    .scaleEffect(0.7)
                Text("Initializing...")
            }
        case .ready:
            Label("Ready", systemImage: "checkmark.circle.fill")
                .foregroundStyle(Color.statusSuccess)
        case .failed(let error):
            Label("Error", systemImage: "exclamationmark.triangle.fill")
                .foregroundStyle(Color.statusError)
                .help(error)
        }
    }

    @ViewBuilder
    private func errorView(_ error: String) -> some View {
        VStack(spacing: 16) {
            Image(systemName: "exclamationmark.triangle.fill")
                .font(Typography.heroTitle(48))
                .foregroundStyle(Color.statusError)

            Text("Failed to initialize Ghostty")
                .font(Typography.sectionTitle())

            Text(error)
                .font(Typography.body())
                .foregroundStyle(palette.textSecondary)
                .multilineTextAlignment(.center)
                .padding(.horizontal)

            Button("Retry") {
                ghosttyManager.initialize()
            }
            .buttonStyle(.borderedProminent)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(Color.black.opacity(0.9))
    }

    @ViewBuilder
    private var terminalView: some View {
        let homeDir = FileManager.default.homeDirectoryForCurrentUser.path

        GhosttyTerminalView(
            workingDirectory: homeDir,
            manager: ghosttyManager
        )
        .background(Color.black)
    }
}

#Preview {
    TerminalTestWindow()
        .frame(width: 800, height: 600)
}
