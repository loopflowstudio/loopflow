import SwiftUI

public struct ConnectionsPanel: View {
    @Environment(\.palette) private var palette

    public let authStore: AuthProviderStore
    public let secretsStore: SecretsProviderStore
    public let enabledProviders: Set<AuthProvider>?
    public let browserFallback: Set<AuthProvider>
    public let onConnect: (AuthProvider) -> Void
    public let onDisconnect: (AuthProvider) -> Void
    public let onCancel: (AuthProvider) -> Void
    public let onToggle: ((AuthProvider, Bool) -> Void)?
    public let onCopy: (String) -> Void

    public init(
        authStore: AuthProviderStore,
        secretsStore: SecretsProviderStore,
        enabledProviders: Set<AuthProvider>? = nil,
        browserFallback: Set<AuthProvider> = [],
        onConnect: @escaping (AuthProvider) -> Void,
        onDisconnect: @escaping (AuthProvider) -> Void,
        onCancel: @escaping (AuthProvider) -> Void,
        onToggle: ((AuthProvider, Bool) -> Void)? = nil,
        onCopy: @escaping (String) -> Void
    ) {
        self.authStore = authStore
        self.secretsStore = secretsStore
        self.enabledProviders = enabledProviders
        self.browserFallback = browserFallback
        self.onConnect = onConnect
        self.onDisconnect = onDisconnect
        self.onCancel = onCancel
        self.onToggle = onToggle
        self.onCopy = onCopy
    }

    public var body: some View {
        VStack(alignment: .leading, spacing: Spacing.lg) {
            ForEach(ProviderRole.displayOrder, id: \.self) { role in
                let providers = statuses(for: role)
                if !providers.isEmpty {
                    if role == .secrets {
                        secretsGroup(providers: providers)
                    } else {
                        ProviderGroupSection(
                            role: role,
                            providers: providers,
                            pendingFlows: authStore.pendingFlows,
                            enabledProviders: enabledProviders,
                            errors: errorFor,
                            browserFallback: browserFallback,
                            onConnect: onConnect,
                            onDisconnect: onDisconnect,
                            onCancel: onCancel,
                            onToggle: onToggle,
                            onCopy: onCopy
                        )
                    }
                }
            }
        }
    }

    private func secretsGroup(providers: [AuthProviderStatus]) -> some View {
        ProviderGroupSection(
            role: .secrets,
            providers: providers,
            pendingFlows: authStore.pendingFlows,
            errors: errorFor,
            browserFallback: browserFallback,
            onConnect: onConnect,
            onDisconnect: onDisconnect,
            onCancel: onCancel,
            onCopy: onCopy
        ) {
            secretsConfigFooter
        }
    }

    @ViewBuilder
    private var secretsConfigFooter: some View {
        let dopplerConnected = authStore.ordered.first(where: { $0.provider == .doppler })?.status == .active
        if dopplerConnected {
            SecretsConfigView(store: secretsStore)
        }
    }

    private func statuses(for role: ProviderRole) -> [AuthProviderStatus] {
        AuthProvider.providers(for: role).map { provider in
            authStore.status(for: provider)
        }
    }

    private func errorFor(_ provider: AuthProvider) -> String? {
        guard authStore.errorProvider == provider else { return nil }
        return authStore.error
    }
}

/// Inline secrets config shown beneath Doppler when connected.
struct SecretsConfigView: View {
    @Environment(\.palette) private var palette
    @Bindable var store: SecretsProviderStore

    var body: some View {
        VStack(alignment: .leading, spacing: Spacing.sm) {
            if store.isLoadingProjects {
                HStack(spacing: Spacing.sm) {
                    ProgressView().controlSize(.small)
                    Text("Loading projects…")
                        .font(Typography.caption())
                        .foregroundStyle(palette.textSecondary)
                }
            } else if let project = store.status.project, let config = store.status.config {
                connectedConfig(project: project, config: config)
            } else {
                configSelection
            }

            if let error = store.error {
                Text(error)
                    .font(Typography.caption())
                    .foregroundStyle(Color.statusError)
            }
        }
        .task {
            if store.projects.isEmpty && store.status.project == nil {
                await store.loadProjects()
            }
        }
    }

    private func connectedConfig(project: String, config: String) -> some View {
        VStack(alignment: .leading, spacing: Spacing.sm) {
            Text("\(project) / \(config)")
                .font(Typography.code(13))
                .foregroundStyle(palette.textSecondary)

            ForEach(store.status.keys) { key in
                HStack(spacing: Spacing.xs) {
                    Circle()
                        .fill(key.present ? Color.statusSuccess : Color.statusNeutral)
                        .frame(width: 6, height: 6)
                    Text(key.envName)
                        .font(Typography.code(12))
                        .foregroundStyle(palette.text)
                    Spacer(minLength: 0)
                    Text(key.present ? "Present" : "Missing")
                        .font(Typography.caption())
                        .foregroundStyle(key.present ? palette.textSecondary : Color.statusWarning)
                }
            }

            HStack(spacing: Spacing.sm) {
                Button("Refresh") { Task { await store.sync() } }
                    .buttonStyle(GhostButtonStyle())
                    .controlSize(.small)
                    .disabled(store.isSyncing)

                Button("Disconnect") { Task { await store.disconnect() } }
                    .buttonStyle(GhostButtonStyle())
                    .controlSize(.small)

                if store.isSyncing {
                    ProgressView().controlSize(.small)
                }
            }
        }
    }

    @ViewBuilder
    private var configSelection: some View {
        if store.projects.count > 1 {
            Picker("Project", selection: Binding(
                get: { store.selectedProject },
                set: { project in
                    if let project { Task { await store.selectProject(project) } }
                }
            )) {
                Text("Select…").tag(nil as DopplerProject?)
                ForEach(store.projects) { project in
                    Text(project.name).tag(project as DopplerProject?)
                }
            }
            .pickerStyle(.menu)
            .labelsHidden()
        }

        if store.selectedProject != nil && !store.configs.isEmpty {
            Picker("Config", selection: Binding(
                get: { store.selectedConfig },
                set: { config in
                    if let config { Task { await store.selectConfig(config) } }
                }
            )) {
                Text("Select…").tag(nil as DopplerConfig?)
                ForEach(store.configs) { config in
                    Text(config.name).tag(config as DopplerConfig?)
                }
            }
            .pickerStyle(.menu)
            .labelsHidden()
        }

        if store.isSyncing {
            HStack(spacing: Spacing.sm) {
                ProgressView().controlSize(.small)
                Text("Syncing secrets…")
                    .font(Typography.caption())
                    .foregroundStyle(palette.textSecondary)
            }
        }
    }
}
