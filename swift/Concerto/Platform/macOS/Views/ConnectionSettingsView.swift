import SwiftUI
import LoopflowCore
import AppKit

struct ConnectionSettingsView: View {
    @Environment(RepoState.self) private var repoState
    @Environment(OutputBuffer.self) private var outputBuffer
    @Environment(\.authService) private var authService
    @Environment(\.palette) private var palette
    @Environment(\.dismiss) private var dismiss

    @State private var mode: ConnectionMode = .bundled
    @State private var serverURL = ""
    @State private var token = ""
    @State private var selectedRepoPath: String = ""
    @State private var isConnecting = false
    @State private var errorMessage: String?
    @State private var cliMessage: String?
    @State private var selectedInstallDirectory = CLIInstallManager.defaultInstallDir.path
    @State private var browserFallbackProviders: Set<AuthProvider> = []
    @State private var connectWithPhone = BundledDaemonManager.connectWithPhoneEnabled

    private let cliInstallManager = CLIInstallManager()

    var body: some View {
        VStack(spacing: 0) {
            header
            modePicker

            ScrollView {
                VStack(alignment: .leading, spacing: Spacing.lg) {
                    if mode == .bundled {
                        bundledContent
                    } else {
                        remoteContent
                    }

                    statusContent
                    providerConnectionsContent
                    supportedHarnessesContent

                    if let cliMessage, mode == .bundled {
                        Text(cliMessage)
                            .font(Typography.caption(11))
                            .foregroundStyle(palette.textSecondary)
                    }

                    if let errorMessage {
                        Text(errorMessage)
                            .font(Typography.caption())
                            .foregroundStyle(Color.statusError)
                    }

                    actionsContent
                }
                .padding(.horizontal, Spacing.xl)
                .padding(.bottom, Spacing.xl)
            }
        }
        .background(palette.background)
        .frame(width: 420)
        .frame(minHeight: 500, idealHeight: 760)
        .onAppear {
            loadFromCurrentConnection()
            refreshProviderStatuses()
        }
        .onChange(of: repoState.connectionState) { _, _ in
            refreshProviderStatuses()
        }
        .onChange(of: mode) { _, _ in
            refreshProviderStatuses()
        }
        .onChange(of: repoState.authProviderStore.browserLaunchRequest) { _, _ in
            handleBrowserLaunchRequest()
        }
    }

    private var header: some View {
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
    }

    private var modePicker: some View {
        Picker("Mode", selection: $mode) {
            Text("Bundled").tag(ConnectionMode.bundled)
            Text("Remote").tag(ConnectionMode.remote)
        }
        .pickerStyle(.segmented)
        .labelsHidden()
        .padding(.horizontal, Spacing.xl)
        .padding(.bottom, Spacing.lg)
    }

    private var bundledContent: some View {
        VStack(alignment: .leading, spacing: Spacing.lg) {
            VStack(alignment: .leading, spacing: Spacing.sm) {
                Text("Bundled lfd")
                    .font(Typography.caption())
                    .foregroundStyle(palette.textSecondary)

                HStack(spacing: Spacing.sm) {
                    Circle()
                        .fill(repoState.lfdConnected ? Color.statusSuccess : palette.border)
                        .frame(width: 8, height: 8)
                    Text(repoState.connectionStore.activeConnection.displayName)
                        .font(Typography.code(13))
                        .foregroundStyle(palette.text)
                }
                .padding(Spacing.md)
                .frame(maxWidth: .infinity, alignment: .leading)
                .background(palette.surfaceMuted)
                .clipShape(RoundedRectangle(cornerRadius: CornerRadius.md))

                Text("Concerto runs one bundled lfd. Runtime port and token are regenerated each launch.")
                    .font(Typography.caption(11))
                    .foregroundStyle(palette.textSecondary)

                if BundledDaemonManager.prefersNativeMode {
                    Text("Native mode fallback is enabled.")
                        .font(Typography.caption(11))
                        .foregroundStyle(Color.statusWarning)
                }

                Toggle("Connect with my phone", isOn: $connectWithPhone)
                    .font(Typography.caption())
                    .onChange(of: connectWithPhone) { _, enabled in
                        handleConnectWithPhoneChange(enabled)
                    }

                Text("Enables mobile access and exposes lfd for discovery when signed in to studio.")
                    .font(Typography.caption(11))
                    .foregroundStyle(palette.textSecondary)
            }

            cliToolsContent
        }
    }

    private var cliToolsContent: some View {
        VStack(alignment: .leading, spacing: Spacing.sm) {
            Text("Install CLI tools")
                .font(Typography.caption())
                .foregroundStyle(palette.textSecondary)

            Picker("Install directory", selection: $selectedInstallDirectory) {
                Text(CLIInstallManager.defaultInstallDir.path).tag(CLIInstallManager.defaultInstallDir.path)
                Text(CLIInstallManager.systemInstallDir.path).tag(CLIInstallManager.systemInstallDir.path)
            }
            .labelsHidden()

            HStack(spacing: Spacing.sm) {
                Button(isCLIInstalled ? "Uninstall CLI tools" : "Install CLI tools") {
                    updateCLIInstallation()
                }
                .buttonStyle(DarkButtonStyle())

                Text(selectedInstallDirectory)
                    .font(Typography.code(11))
                    .foregroundStyle(palette.textSecondary)
            }

            if let pathExport = cliInstallManager.pathExportLineIfNeeded(for: selectedInstallDirectoryURL) {
                Text(pathExport)
                    .font(Typography.code(11))
                    .foregroundStyle(palette.textSecondary)
                    .textSelection(.enabled)
            }
        }
        .padding(Spacing.md)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(palette.surfaceMuted)
        .clipShape(RoundedRectangle(cornerRadius: CornerRadius.md))
    }

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
                SecureField("Connection token", text: $token)
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

    @ViewBuilder
    private var providerConnectionsContent: some View {
        VStack(alignment: .leading, spacing: Spacing.sm) {
            Text("Provider Connections")
                .font(Typography.caption())
                .foregroundStyle(palette.textSecondary)

            if !isProviderAuthAvailable {
                Text("Connect to server first.")
                    .font(Typography.caption())
                    .foregroundStyle(palette.textSecondary)
            }

            ForEach(repoState.authProviderStore.ordered) { status in
                AuthProviderCard(
                    status: status,
                    pendingFlow: repoState.authProviderStore.pendingFlows[status.provider],
                    isEnabled: isProviderAuthAvailable,
                    error: repoState.authProviderStore.errorProvider == status.provider
                        ? repoState.authProviderStore.error
                        : nil,
                    showURLFallback: browserFallbackProviders.contains(status.provider),
                    onConnect: connectProvider,
                    onDisconnect: disconnectProvider,
                    onCancel: disconnectProvider,
                    onCopy: copyToClipboard
                )
            }
        }
    }

    private var supportedHarnessesContent: some View {
        VStack(alignment: .leading, spacing: Spacing.xs) {
            Text("Supported harnesses")
                .font(Typography.caption())
                .foregroundStyle(palette.textSecondary)

            if repoState.supportedHarnesses.isEmpty {
                Text("None configured — using system default (claude)")
                    .font(Typography.caption(11))
                    .foregroundStyle(palette.textSecondary)
            } else {
                Text(repoState.supportedHarnesses.joined(separator: ", "))
                    .font(Typography.code(11))
                    .foregroundStyle(palette.text)
            }
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

    private var actionsContent: some View {
        HStack(spacing: Spacing.sm) {
            Button {
                connect()
            } label: {
                if isConnecting {
                    ProgressView()
                        .controlSize(.small)
                        .frame(width: 90)
                } else {
                    Text(mode == .bundled ? "Reconnect" : "Connect")
                        .frame(width: 90)
                }
            }
            .buttonStyle(DarkButtonStyle())
            .disabled(isConnecting)

            if case .trustRequired = repoState.connectionState {
                Button("Trust Certificate") {
                    repoState.trustNewCertificate()
                    connect()
                }
                .buttonStyle(GhostButtonStyle())
            }

            if shouldShowDockerFallback {
                Button("Use Native Mode") {
                    BundledDaemonManager.setPreferNativeMode(true)
                    connect()
                }
                .buttonStyle(GhostButtonStyle())
            } else if mode == .bundled && BundledDaemonManager.prefersNativeMode {
                Button("Use Docker Mode") {
                    BundledDaemonManager.setPreferNativeMode(false)
                }
                .buttonStyle(GhostButtonStyle())
            }

            Spacer()
        }
        .overlay(alignment: .bottomLeading) {
            if shouldShowDockerFallback {
                Link("Install Docker Desktop", destination: URL(string: "https://www.docker.com/products/docker-desktop/")!)
                    .font(Typography.caption(11))
                    .padding(.top, 36)
            }
        }
    }

    private var selectedInstallDirectoryURL: URL {
        URL(fileURLWithPath: selectedInstallDirectory, isDirectory: true)
    }

    private var isCLIInstalled: Bool {
        cliInstallManager.isInstalled(in: selectedInstallDirectoryURL)
    }

    private var isProviderAuthAvailable: Bool {
        repoState.isConnected || bundledProviderConnection != nil
    }

    private var bundledProviderConnection: ServerConnection? {
        guard mode == .bundled else { return nil }
        return SharedDaemon.currentConnection
    }

    private func loadFromCurrentConnection() {
        mode = repoState.connectionStore.mode
        if mode == .remote {
            let connection = repoState.connectionStore.configuredRemoteConnection
                ?? repoState.connectionStore.activeConnection
            serverURL = connection.displayName
            token = connection.staticToken
                ?? repoState.connectionStore.token(for: connection)
                ?? ""
        } else {
            serverURL = ""
            token = ""
            connectWithPhone = BundledDaemonManager.connectWithPhoneEnabled
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
                switch mode {
                case .bundled:
                    try await repoState.connectBundled(outputBuffer: outputBuffer)
                case .remote:
                    guard let connection = makeRemoteConnectionFromForm() else {
                        return
                    }
                    try await repoState.connect(to: connection, outputBuffer: outputBuffer)
                    if let first = repoState.availableRemoteRepos.first, selectedRepoPath.isEmpty {
                        selectedRepoPath = first.path
                        repoState.selectRemoteRepo(path: first.path)
                    }
                }
            } catch {
                errorMessage = error.localizedDescription
            }
        }
    }

    private func updateCLIInstallation() {
        do {
            if isCLIInstalled {
                try cliInstallManager.uninstall(from: selectedInstallDirectoryURL)
                cliMessage = "Removed symlinks from \(selectedInstallDirectory)."
            } else {
                try cliInstallManager.install(to: selectedInstallDirectoryURL)
                cliMessage = "Installed lf and lfd to \(selectedInstallDirectory)."
            }
        } catch {
            errorMessage = error.localizedDescription
        }
    }

    private func makeRemoteConnectionFromForm() -> ServerConnection? {
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

        let opened = NSWorkspace.shared.open(launchRequest.url)
        if opened {
            browserFallbackProviders.remove(launchRequest.provider)
        } else {
            browserFallbackProviders.insert(launchRequest.provider)
        }
    }

    private var shouldShowDockerFallback: Bool {
        guard mode == .bundled, let errorMessage else {
            return false
        }
        return errorMessage.localizedCaseInsensitiveContains("docker is not running")
    }

    private func handleConnectWithPhoneChange(_ enabled: Bool) {
        if enabled && authService.currentToken() == nil {
            connectWithPhone = false
            errorMessage = "Sign in to loopflow.studio on this Mac before enabling phone access."
            return
        }
        if !enabled && !BundledDaemonManager.connectWithPhoneEnabled {
            return
        }

        errorMessage = nil
        BundledDaemonManager.setConnectWithPhoneEnabled(enabled)

        guard mode == .bundled else { return }
        connect()
    }

    private func refreshProviderStatuses() {
        Task {
            if repoState.isConnected {
                await repoState.authProviderStore.refresh()
                return
            }

            guard mode == .bundled else { return }

            for _ in 0..<8 {
                if let connection = bundledProviderConnection {
                    let service = WaveService(
                        connection: connection,
                        tokenProvider: { connection.staticToken }
                    )
                    repoState.authProviderStore.bindService(service)
                    await repoState.authProviderStore.refresh()
                    return
                }

                try? await Task.sleep(for: .milliseconds(150))
            }
        }
    }
}
