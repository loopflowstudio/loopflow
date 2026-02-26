#if os(iOS)
import SwiftUI
import LoopflowCore

struct ConnectionSetupView: View {
    @Environment(RepoState.self) private var repoState
    @Environment(OutputBuffer.self) private var outputBuffer
    @Environment(\.openURL) private var openURL

    @State private var host = ""
    @State private var port = "2486"
    @State private var useTLS = false
    @State private var token = ""
    @State private var selectedRepoPath = ""
    @State private var isConnecting = false
    @State private var errorMessage: String?
    @State private var browserFallbackProviders: Set<AuthProvider> = []

    var body: some View {
        NavigationStack {
            Form {
                Section("Server") {
                    TextField("Host", text: $host)
                        .textInputAutocapitalization(.never)
                        .autocorrectionDisabled()
                    TextField("Port", text: $port)
                        .keyboardType(.numberPad)
                    Toggle("Use TLS", isOn: $useTLS)
                    SecureField("Token", text: $token)
                        .textInputAutocapitalization(.never)
                        .autocorrectionDisabled()
                }

                if !repoState.availableRemoteRepos.isEmpty {
                    Section("Repository") {
                        Picker("Repo", selection: $selectedRepoPath) {
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
                    }
                }

                if let errorMessage {
                    Section {
                        Text(errorMessage)
                            .foregroundStyle(Color.statusError)
                    }
                }

                providerConnectionsSection
            }
            .navigationTitle("Connect")
            .toolbar {
                ToolbarItem(placement: .topBarTrailing) {
                    Button {
                        connect()
                    } label: {
                        if isConnecting {
                            ProgressView()
                        } else {
                            Text("Connect")
                        }
                    }
                    .disabled(isConnecting)
                }
            }
            .onAppear {
                loadFromCurrentConnection()
                guard repoState.isConnected else { return }
                Task { await repoState.authProviderStore.refresh() }
            }
            .onChange(of: repoState.authProviderStore.browserLaunchRequest) { _, _ in
                handleBrowserLaunchRequest()
            }
        }
        .tint(.loopflowBurgundy)
    }

    @ViewBuilder
    private var providerConnectionsSection: some View {
        Section("Provider Connections") {
            if !repoState.isConnected {
                Text("Connect to server first.")
                    .foregroundStyle(.secondary)
            }

            ForEach(repoState.authProviderStore.ordered) { status in
                AuthProviderCard(
                    status: status,
                    pendingFlow: repoState.authProviderStore.pendingFlows[status.provider],
                    isEnabled: repoState.isConnected,
                    error: repoState.authProviderStore.errorProvider == status.provider
                        ? repoState.authProviderStore.error
                        : nil,
                    showURLFallback: browserFallbackProviders.contains(status.provider),
                    onConnect: connectProvider,
                    onDisconnect: disconnectProvider,
                    onCancel: disconnectProvider
                )
                .listRowInsets(EdgeInsets())
                .listRowSeparator(.hidden)
            }
        }
    }

    private func loadFromCurrentConnection() {
        let connection = repoState.connectionStore.configuredRemoteConnection ?? repoState.connectionStore.activeConnection
        if !connection.isLocal {
            host = connection.host
            port = "\(connection.port)"
            useTLS = connection.useTLS
            token = connection.staticToken
                ?? repoState.connectionStore.token(for: connection)
                ?? ""
        }

        if case .remote(let path, _) = repoState.repoTarget {
            selectedRepoPath = path
        }
    }

    private func connect() {
        errorMessage = nil
        isConnecting = true

        Task {
            defer { isConnecting = false }
            do {
                let connection = try makeConnection()
                try await repoState.connect(to: connection, outputBuffer: outputBuffer)
                if selectedRepoPath.isEmpty, let first = repoState.availableRemoteRepos.first {
                    selectedRepoPath = first.path
                    repoState.selectRemoteRepo(path: first.path)
                }
            } catch {
                errorMessage = error.localizedDescription
            }
        }
    }

    private func makeConnection() throws -> ServerConnection {
        let trimmedHost = host.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmedHost.isEmpty else {
            throw WaveServiceError.commandFailed("Host is required.")
        }

        guard let parsedPort = Int(port), (1 ... 65_535).contains(parsedPort) else {
            throw WaveServiceError.commandFailed("Port must be between 1 and 65535.")
        }

        let trimmedToken = token.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmedToken.isEmpty else {
            throw WaveServiceError.commandFailed("Token is required.")
        }

        return ServerConnection(
            host: trimmedHost,
            port: parsedPort,
            useTLS: useTLS,
            authMode: .staticToken,
            staticToken: trimmedToken
        )
    }

    private func connectProvider(_ provider: AuthProvider) {
        Task { await repoState.authProviderStore.connect(provider) }
    }

    private func disconnectProvider(_ provider: AuthProvider) {
        Task { await repoState.authProviderStore.disconnect(provider) }
    }

    private func handleBrowserLaunchRequest() {
        guard let launchRequest = repoState.authProviderStore.consumeBrowserLaunchRequest() else {
            return
        }

        openURL(launchRequest.url) { accepted in
            if accepted {
                browserFallbackProviders.remove(launchRequest.provider)
            } else {
                browserFallbackProviders.insert(launchRequest.provider)
            }
        }
    }
}

#endif
