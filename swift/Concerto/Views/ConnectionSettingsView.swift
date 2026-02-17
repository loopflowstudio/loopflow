import SwiftUI
import LoopflowCore

struct ConnectionSettingsView: View {
    @Environment(RepoState.self) private var repoState
    @Environment(OutputBuffer.self) private var outputBuffer
    @Environment(\.dismiss) private var dismiss

    @State private var host = "127.0.0.1"
    @State private var port = "2486"
    @State private var useTLS = false
    @State private var authMode: ConnectionAuthMode = .none
    @State private var token = ""
    @State private var selectedRepoPath: String = ""
    @State private var isConnecting = false
    @State private var errorMessage: String?

    var body: some View {
        NavigationStack {
            Form {
                connectionSection
                repoSection
                stateSection
                actionsSection
            }
            .navigationTitle("Connection")
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Close") { dismiss() }
                }
            }
            .onAppear {
                loadFromCurrentConnection()
            }
        }
        .frame(width: 480, height: 460)
    }

    private var connectionSection: some View {
        Section("Server") {
            TextField("Host", text: $host)
            TextField("Port", text: $port)
            Toggle("Use TLS", isOn: $useTLS)

            Picker("Auth", selection: $authMode) {
                Text("None").tag(ConnectionAuthMode.none)
                Text("Static token").tag(ConnectionAuthMode.staticToken)
            }

            if authMode.requiresToken {
                SecureField("Token", text: $token)
            }
        }
    }

    @ViewBuilder
    private var repoSection: some View {
        if !repoState.availableRemoteRepos.isEmpty {
            Section("Remote Repo") {
                Picker("Repository", selection: $selectedRepoPath) {
                    ForEach(repoState.availableRemoteRepos) { repo in
                        Text("\(repo.name) (\(repo.waveCount))")
                            .tag(repo.path)
                    }
                }
                .onChange(of: selectedRepoPath) { _, newValue in
                    guard !newValue.isEmpty else { return }
                    repoState.selectRemoteRepo(path: newValue)
                    Task {
                        await repoState.refreshWaves()
                        await repoState.refreshFlowsAsync()
                    }
                }

                if selectedRepoPath.isEmpty {
                    Text("No remote repo selected")
                        .foregroundStyle(.secondary)
                } else {
                    Text(selectedRepoPath)
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
            }
        } else if !repoState.connectionStore.activeConnection.isLocal {
            Section("Remote Repo") {
                Text("No repos found on server.")
                Text("Run `lfq create <name> <repo>` on the server to bootstrap the first wave.")
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }
        }
    }

    private var stateSection: some View {
        Section("Status") {
            Text(repoState.connectionSummary)

            if let errorMessage {
                Text(errorMessage)
                    .foregroundStyle(Color.statusError)
            }

            if case .trustRequired = repoState.connectionState {
                Button("Trust New Certificate") {
                    repoState.trustNewCertificate()
                    connect()
                }
                Button("Clear Pinned Certificate") {
                    repoState.clearPinnedCertificate()
                    connect()
                }
            }
        }
    }

    private var actionsSection: some View {
        Section {
            Button {
                connect()
            } label: {
                if isConnecting {
                    ProgressView()
                } else {
                    Text("Connect / Test")
                }
            }
            .disabled(isConnecting)

            Button("Switch to Local") {
                Task {
                    do {
                        try await repoState.switchToLocal(outputBuffer: outputBuffer)
                    } catch {
                        errorMessage = error.localizedDescription
                    }
                }
            }
        }
    }

    private func loadFromCurrentConnection() {
        let connection = repoState.connectionStore.activeConnection
        host = connection.host
        port = String(connection.port)
        useTLS = connection.useTLS
        authMode = connection.authMode
        token = connection.staticToken ?? repoState.connectionStore.token(for: connection) ?? ""

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

    private func makeConnectionFromForm() -> ServerConnection? {
        let trimmedHost = host.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmedHost.isEmpty else {
            errorMessage = "Host is required."
            return nil
        }
        guard !trimmedHost.contains("://") else {
            errorMessage = "Enter host only (no http:// or https://)."
            return nil
        }

        let trimmedPort = port.trimmingCharacters(in: .whitespacesAndNewlines)
        let portValue: Int
        if trimmedPort.isEmpty {
            portValue = useTLS ? 443 : 2486
        } else if let parsedPort = Int(trimmedPort), (1 ... 65_535).contains(parsedPort) {
            portValue = parsedPort
        } else {
            errorMessage = "Port must be between 1 and 65535."
            return nil
        }

        let trimmedToken = token.trimmingCharacters(in: .whitespacesAndNewlines)
        if authMode.requiresToken, trimmedToken.isEmpty {
            errorMessage = "Token is required for static token auth."
            return nil
        }

        return ServerConnection(
            host: trimmedHost,
            port: portValue,
            useTLS: useTLS,
            authMode: authMode,
            staticToken: trimmedToken.isEmpty ? nil : trimmedToken
        )
    }
}
