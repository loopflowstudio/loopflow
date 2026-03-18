import Foundation

public struct SecretsEvent: Sendable {
    public enum EventType: String, Sendable {
        case connected = "secrets.connected"
        case synced = "secrets.synced"
        case disconnected = "secrets.disconnected"
    }

    public let type: EventType
    public let provider: String?
    public let timestamp: Date

    public init(type: EventType, provider: String? = nil, timestamp: Date = .now) {
        self.type = type
        self.provider = provider
        self.timestamp = timestamp
    }
}

/// Smart default config preference order.
private let preferredConfigs = ["dev", "prd", "prod"]

protocol SecretsProviderService: Sendable {
    func secretsStatus() async throws -> SecretsProviderStatus
    func listSecretsProjects() async throws -> [DopplerProject]
    func listSecretsConfigs(project: String) async throws -> [DopplerConfig]
    func selectSecretsConfig(project: String, config: String) async throws -> SecretsProviderStatus
    func syncSecrets() async throws -> SecretsProviderStatus
    func disconnectSecrets() async throws -> SecretsProviderStatus
}

extension WaveService: SecretsProviderService {}

@MainActor
@Observable
public final class SecretsProviderStore {
    public private(set) var status: SecretsProviderStatus = .disconnected
    public private(set) var projects: [DopplerProject] = []
    public private(set) var configs: [DopplerConfig] = []
    public private(set) var selectedProject: DopplerProject?
    public private(set) var selectedConfig: DopplerConfig?
    public private(set) var isSyncing = false
    public private(set) var isLoadingProjects = false
    public private(set) var error: String?

    private var service: (any SecretsProviderService)?

    public init() {}

    public func bindService(_ waveService: LocalWaveService) {
        bindService(waveService as any SecretsProviderService)
    }

    func bindService(_ service: any SecretsProviderService) {
        self.service = service
    }

    public func refresh() async {
        guard let service else { return }
        do {
            status = try await service.secretsStatus()
            error = nil
        } catch {
            self.error = error.localizedDescription
        }
    }

    public func loadProjects() async {
        guard let service else {
            error = "Connect to server first."
            return
        }
        isLoadingProjects = true
        defer { isLoadingProjects = false }
        do {
            projects = try await service.listSecretsProjects()
            error = nil

            // Auto-select if only one project
            if projects.count == 1 {
                selectedProject = projects[0]
                await loadConfigs(for: projects[0])
            }
        } catch {
            self.error = error.localizedDescription
        }
    }

    public func selectProject(_ project: DopplerProject) async {
        selectedProject = project
        selectedConfig = nil
        configs = []
        await loadConfigs(for: project)
    }

    public func loadConfigs(for project: DopplerProject) async {
        guard let service else { return }
        do {
            configs = try await service.listSecretsConfigs(project: project.slug)
            error = nil

            // Smart default: prefer dev > prd > prod > first
            selectedConfig = smartDefaultConfig(configs)
        } catch {
            self.error = error.localizedDescription
        }
    }

    public func selectConfig(_ config: DopplerConfig) async {
        guard let service, let project = selectedProject else { return }
        selectedConfig = config
        isSyncing = true
        defer { isSyncing = false }
        do {
            status = try await service.selectSecretsConfig(
                project: project.slug, config: config.name
            )
            error = nil
        } catch {
            self.error = error.localizedDescription
        }
    }

    public func sync() async {
        guard let service else {
            error = "Connect to server first."
            return
        }
        isSyncing = true
        defer { isSyncing = false }
        do {
            status = try await service.syncSecrets()
            error = nil
        } catch {
            self.error = error.localizedDescription
        }
    }

    public func disconnect() async {
        guard let service else {
            error = "Connect to server first."
            return
        }
        do {
            status = try await service.disconnectSecrets()
            projects = []
            configs = []
            selectedProject = nil
            selectedConfig = nil
            error = nil
        } catch {
            self.error = error.localizedDescription
        }
    }

    public func handleEvent(_ event: SecretsEvent) {
        switch event.type {
        case .connected, .synced:
            Task { await refresh() }
        case .disconnected:
            status = .disconnected
            projects = []
            configs = []
            selectedProject = nil
            selectedConfig = nil
            error = nil
        }
    }
}

func smartDefaultConfig(_ configs: [DopplerConfig]) -> DopplerConfig? {
    for preferred in preferredConfigs {
        if let config = configs.first(where: { $0.name == preferred }) {
            return config
        }
    }
    return configs.first
}
