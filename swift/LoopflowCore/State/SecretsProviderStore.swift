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

protocol SecretsProviderService: Sendable {
    func secretsStatus() async throws -> SecretsProviderStatus
    func connectSecrets(provider: String, token: String, project: String, config: String) async throws -> SecretsProviderStatus
    func syncSecrets() async throws -> SecretsProviderStatus
    func updateSecretsConfig(project: String?, config: String?) async throws -> SecretsProviderStatus
    func disconnectSecrets() async throws -> SecretsProviderStatus
}

extension WaveService: SecretsProviderService {}

@MainActor
@Observable
public final class SecretsProviderStore {
    public private(set) var status: SecretsProviderStatus = .disconnected
    public private(set) var isSyncing = false
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

    public func connect(provider: String, token: String, project: String, config: String) async {
        guard let service else {
            error = "Connect to server first."
            return
        }
        isSyncing = true
        defer { isSyncing = false }
        do {
            status = try await service.connectSecrets(
                provider: provider, token: token, project: project, config: config
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
            error = nil
        }
    }
}
