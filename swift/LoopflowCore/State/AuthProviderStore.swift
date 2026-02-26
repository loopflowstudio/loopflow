import Foundation

protocol AuthProviderService: Sendable {
    func listAuthProviders() async throws -> [AuthProviderStatus]
    func startAuthFlow(provider: AuthProvider) async throws -> AuthFlow
    func disconnectProvider(provider: AuthProvider) async throws -> AuthProviderStatus
}

extension WaveService: AuthProviderService {}

@MainActor
@Observable
public final class AuthProviderStore {
    public struct BrowserLaunchRequest: Sendable, Equatable {
        public let provider: AuthProvider
        public let url: URL

        public init(provider: AuthProvider, url: URL) {
            self.provider = provider
            self.url = url
        }
    }

    public private(set) var providers: [AuthProvider: AuthProviderStatus] = [:]
    public private(set) var pendingFlows: [AuthProvider: AuthFlow] = [:]
    public private(set) var browserLaunchRequest: BrowserLaunchRequest?
    public private(set) var error: String?
    public private(set) var errorProvider: AuthProvider?

    private var waveService: (any AuthProviderService)?
    private var wasConnected = false

    public init() {}

    public var ordered: [AuthProviderStatus] {
        AuthProvider.allCases.map(status(for:))
    }

    public func bindService(_ waveService: LocalWaveService) {
        bindService(waveService as any AuthProviderService)
    }

    func bindService(_ waveService: any AuthProviderService) {
        self.waveService = waveService
    }

    public func refresh() async {
        guard let waveService else {
            return
        }

        do {
            let statuses = try await waveService.listAuthProviders()
            providers = mergedStatuses(statuses)
            pendingFlows = pendingFlows.filter { provider, _ in
                providers[provider]?.status == .pending
            }
            setError(nil, provider: nil)
        } catch {
            setError(error.localizedDescription, provider: nil)
        }
    }

    public func connect(_ provider: AuthProvider) async {
        guard let waveService else {
            setError("Connect to server first.", provider: provider)
            return
        }

        setProvider(provider, status: .pending, login: status(for: provider).login)
        setError(nil, provider: nil)

        do {
            let flow = try await waveService.startAuthFlow(provider: provider)
            pendingFlows[provider] = flow
            enqueueBrowserLaunch(provider: provider, flow: flow)
        } catch let serviceError as WaveServiceError {
            if isConflict(serviceError) {
                await refresh()
                setProvider(provider, status: .pending, login: status(for: provider).login)
                setError(nil, provider: nil)
            } else {
                failConnect(provider: provider, message: serviceError.localizedDescription)
            }
        } catch {
            failConnect(provider: provider, message: error.localizedDescription)
        }
    }

    public func disconnect(_ provider: AuthProvider) async {
        guard let waveService else {
            setError("Connect to server first.", provider: provider)
            return
        }

        do {
            let status = try await waveService.disconnectProvider(provider: provider)
            providers[provider] = status
            pendingFlows.removeValue(forKey: provider)
            setError(nil, provider: nil)
        } catch {
            setError(error.localizedDescription, provider: provider)
        }
    }

    public func handleEvent(_ event: AuthEvent) {
        switch event.type {
        case .flowStarted:
            setProvider(event.provider, status: .pending, login: providers[event.provider]?.login)
            if let flow = flowFrom(event) {
                pendingFlows[event.provider] = flow
            }

        case .connected:
            setProvider(event.provider, status: .active, login: event.login)
            pendingFlows.removeValue(forKey: event.provider)
            setError(nil, provider: nil)

        case .failed:
            setProvider(event.provider, status: .none, login: nil)
            pendingFlows.removeValue(forKey: event.provider)
            setError(event.error ?? "Authentication failed.", provider: event.provider)

        case .disconnected:
            setProvider(event.provider, status: .none, login: nil)
            pendingFlows.removeValue(forKey: event.provider)
            if errorProvider == event.provider {
                setError(nil, provider: nil)
            }
        }
    }

    public func handleConnectionState(_ state: ConnectionState) async {
        let isConnected: Bool
        if case .connected = state {
            isConnected = true
        } else {
            isConnected = false
        }

        defer {
            wasConnected = isConnected
        }

        guard isConnected, !wasConnected else {
            return
        }

        await refresh()
    }

    public func consumeBrowserLaunchRequest() -> BrowserLaunchRequest? {
        defer {
            browserLaunchRequest = nil
        }
        return browserLaunchRequest
    }

    private func setProvider(_ provider: AuthProvider, status: ProviderAuthStatus, login: String?) {
        providers[provider] = AuthProviderStatus(provider: provider, status: status, login: login)
    }

    private func status(for provider: AuthProvider) -> AuthProviderStatus {
        providers[provider] ?? AuthProviderStatus(provider: provider, status: .none, login: nil)
    }

    private func mergedStatuses(_ statuses: [AuthProviderStatus]) -> [AuthProvider: AuthProviderStatus] {
        var next = Dictionary(
            uniqueKeysWithValues: AuthProvider.allCases.map {
                ($0, AuthProviderStatus(provider: $0, status: .none, login: nil))
            }
        )
        for status in statuses {
            next[status.provider] = status
        }
        return next
    }

    private func setError(_ message: String?, provider: AuthProvider?) {
        error = message
        errorProvider = provider
    }

    private func enqueueBrowserLaunch(provider: AuthProvider, flow: AuthFlow) {
        let preferredURL = flow.verification_uri_complete ?? flow.verification_uri
        if let url = URL(string: preferredURL) {
            browserLaunchRequest = BrowserLaunchRequest(provider: provider, url: url)
            return
        }

        setError("Could not open provider auth URL. Copy the link manually.", provider: provider)
    }

    private func failConnect(provider: AuthProvider, message: String) {
        pendingFlows.removeValue(forKey: provider)
        setProvider(provider, status: .none, login: nil)
        setError(message, provider: provider)
    }

    private func isConflict(_ error: WaveServiceError) -> Bool {
        if case .serverError(let status, _) = error {
            return status == 409
        }
        return false
    }

    private func flowFrom(_ event: AuthEvent) -> AuthFlow? {
        let verificationURI = event.verificationURI ?? event.verificationURIComplete
        guard let verificationURI else {
            return nil
        }

        return AuthFlow(
            provider: event.provider,
            verification_uri: verificationURI,
            verification_uri_complete: event.verificationURIComplete,
            user_code: nil,
            expires_in: nil
        )
    }
}
