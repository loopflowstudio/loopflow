import SwiftUI
import LoopflowCore

private enum ConnectionMode: String, CaseIterable {
    case local
    case remote
}

struct ConnectionSettingsView: View {
    @Environment(RepoState.self) private var repoState
    @Environment(OutputBuffer.self) private var outputBuffer
    @Environment(\.palette) private var palette
    @Environment(\.dismiss) private var dismiss

    @State private var mode: ConnectionMode = .local
    @State private var serverURL = ""
    @State private var token = ""
    @State private var selectedRepoPath: String = ""
    @State private var isConnecting = false
    @State private var errorMessage: String?

    var body: some View {
        VStack(spacing: 0) {
            // Header
            HStack {
                Text("Connection")
                    .font(Typography.sectionTitle())
                    .foregroundStyle(palette.text)
                Spacer()
                Button("Close") { dismiss() }
                    .buttonStyle(GhostButtonStyle())
            }
            .padding(.horizontal, Spacing.xl)
            .padding(.top, Spacing.xl)
            .padding(.bottom, Spacing.lg)

            // Mode picker
            Picker("Mode", selection: $mode) {
                Text("Local").tag(ConnectionMode.local)
                Text("Remote").tag(ConnectionMode.remote)
            }
            .pickerStyle(.segmented)
            .labelsHidden()
            .padding(.horizontal, Spacing.xl)
            .padding(.bottom, Spacing.lg)

            // Content
            VStack(alignment: .leading, spacing: Spacing.lg) {
                if mode == .local {
                    localContent
                } else {
                    remoteContent
                }

                statusContent

                if let errorMessage {
                    Text(errorMessage)
                        .font(Typography.caption())
                        .foregroundStyle(Color.statusError)
                }

                actionsContent
            }
            .padding(.horizontal, Spacing.xl)
            .padding(.bottom, Spacing.xl)

            Spacer(minLength: 0)
        }
        .background(palette.background)
        .frame(width: 400, height: mode == .local ? 320 : 440)
        .onAppear { loadFromCurrentConnection() }
    }

    // MARK: - Local

    private var localContent: some View {
        VStack(alignment: .leading, spacing: Spacing.sm) {
            Text("Server")
                .font(Typography.caption())
                .foregroundStyle(palette.textSecondary)

            HStack(spacing: Spacing.sm) {
                Circle()
                    .fill(repoState.lfdConnected ? Color.statusSuccess : palette.border)
                    .frame(width: 8, height: 8)
                Text("http://127.0.0.1:2486")
                    .font(Typography.code(13))
                    .foregroundStyle(palette.text)
            }
            .padding(Spacing.md)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(palette.surfaceMuted)
            .clipShape(RoundedRectangle(cornerRadius: CornerRadius.md))

            Text("Session token auto-discovered from ~/.lf/session-token")
                .font(Typography.caption(11))
                .foregroundStyle(palette.textSecondary)
        }
    }

    // MARK: - Remote

    private var remoteContent: some View {
        VStack(alignment: .leading, spacing: Spacing.lg) {
            VStack(alignment: .leading, spacing: Spacing.sm) {
                Text("Server URL")
                    .font(Typography.caption())
                    .foregroundStyle(palette.textSecondary)
                TextField("https://lfd.example.com:2486", text: $serverURL)
                    .textFieldStyle(.plain)
                    .font(Typography.code(13))
                    .padding(Spacing.md)
                    .background(palette.surfaceMuted)
                    .clipShape(RoundedRectangle(cornerRadius: CornerRadius.md))
            }

            VStack(alignment: .leading, spacing: Spacing.sm) {
                Text("Token")
                    .font(Typography.caption())
                    .foregroundStyle(palette.textSecondary)
                SecureField("Bearer token", text: $token)
                    .textFieldStyle(.plain)
                    .font(Typography.code(13))
                    .padding(Spacing.md)
                    .background(palette.surfaceMuted)
                    .clipShape(RoundedRectangle(cornerRadius: CornerRadius.md))
            }

            repoContent
        }
    }

    @ViewBuilder
    private var repoContent: some View {
        if !repoState.availableRemoteRepos.isEmpty {
            VStack(alignment: .leading, spacing: Spacing.sm) {
                Text("Repository")
                    .font(Typography.caption())
                    .foregroundStyle(palette.textSecondary)
                Picker("Repository", selection: $selectedRepoPath) {
                    ForEach(repoState.availableRemoteRepos) { repo in
                        Text("\(repo.name) (\(repo.waveCount))")
                            .tag(repo.path)
                    }
                }
                .labelsHidden()
                .onChange(of: selectedRepoPath) { _, newValue in
                    guard !newValue.isEmpty else { return }
                    repoState.selectRemoteRepo(path: newValue)
                    Task {
                        await repoState.refreshWaves()
                        await repoState.refreshFlowsAsync()
                    }
                }
            }
        } else if repoState.isConnected {
            VStack(alignment: .leading, spacing: Spacing.xs) {
                Text("No repos found on server.")
                    .font(Typography.caption())
                    .foregroundStyle(palette.textSecondary)
                Text("Run lfq create <name> <repo> on the server.")
                    .font(Typography.caption(11))
                    .foregroundStyle(palette.textSecondary)
            }
        }
    }

    // MARK: - Status

    private var statusContent: some View {
        HStack(spacing: Spacing.sm) {
            Circle()
                .fill(statusColor)
                .frame(width: 8, height: 8)
            Text(repoState.connectionSummary)
                .font(Typography.caption())
                .foregroundStyle(palette.textSecondary)
        }
    }

    private var statusColor: Color {
        switch repoState.connectionState {
        case .connected: return Color.statusSuccess
        case .connecting, .reconnecting: return Color.statusWarning
        case .authFailed: return Color.statusError
        case .trustRequired: return Color.statusWarning
        case .disconnected: return palette.border
        }
    }

    // MARK: - Actions

    private var actionsContent: some View {
        HStack(spacing: Spacing.sm) {
            Button {
                connect()
            } label: {
                if isConnecting {
                    ProgressView()
                        .controlSize(.small)
                        .frame(width: 80)
                } else {
                    Text(mode == .local ? "Reconnect" : "Connect")
                        .frame(width: 80)
                }
            }
            .buttonStyle(DarkButtonStyle())
            .disabled(isConnecting)

            if mode == .remote {
                Button("Switch to Local") {
                    mode = .local
                    connectLocal()
                }
                .buttonStyle(GhostButtonStyle())
            }

            if case .trustRequired = repoState.connectionState {
                Button("Trust Certificate") {
                    repoState.trustNewCertificate()
                    connect()
                }
                .buttonStyle(GhostButtonStyle())
            }

            Spacer()
        }
    }

    // MARK: - Logic

    private func loadFromCurrentConnection() {
        let connection = repoState.connectionStore.activeConnection
        if connection.isLocal {
            mode = .local
            serverURL = ""
        } else {
            mode = .remote
            serverURL = connection.displayName
        }
        token = connection.staticToken
            ?? repoState.connectionStore.token(for: connection)
            ?? ""

        if case .remote(let path) = repoState.repoTarget {
            selectedRepoPath = path
        }
    }

    private func connect() {
        errorMessage = nil

        guard let connection = makeConnectionFromForm() else { return }
        isConnecting = true

        Task {
            do {
                try await repoState.connect(to: connection, outputBuffer: outputBuffer)
                if let first = repoState.availableRemoteRepos.first,
                   !connection.isLocal,
                   selectedRepoPath.isEmpty {
                    selectedRepoPath = first.path
                    repoState.selectRemoteRepo(path: first.path)
                }
            } catch {
                errorMessage = error.localizedDescription
            }
            isConnecting = false
        }
    }

    private func connectLocal() {
        Task {
            do {
                try await repoState.switchToLocal(outputBuffer: outputBuffer)
            } catch {
                errorMessage = error.localizedDescription
            }
        }
    }

    private func makeConnectionFromForm() -> ServerConnection? {
        if mode == .local {
            return .local
        }

        let trimmed = serverURL.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else {
            errorMessage = "Server URL is required."
            return nil
        }

        let urlString = trimmed.contains("://") ? trimmed : "https://\(trimmed)"
        guard let url = URL(string: urlString),
              let host = url.host, !host.isEmpty else {
            errorMessage = "Invalid server URL."
            return nil
        }

        let useTLS = url.scheme == "https" || url.scheme == "wss"
        let port = url.port ?? (useTLS ? 443 : 2486)

        guard (1 ... 65_535).contains(port) else {
            errorMessage = "Port must be between 1 and 65535."
            return nil
        }

        let trimmedToken = token.trimmingCharacters(in: .whitespacesAndNewlines)
        if trimmedToken.isEmpty {
            errorMessage = "Token is required for remote connections."
            return nil
        }

        return ServerConnection(
            host: host,
            port: port,
            useTLS: useTLS,
            authMode: .staticToken,
            staticToken: trimmedToken
        )
    }
}
