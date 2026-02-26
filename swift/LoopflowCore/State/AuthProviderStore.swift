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
        AuthProvider.allCases.map { provider in
            providers[provider] ?? AuthProviderStatus(provider: provider, status: .none, login: nil)
        }
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
            var next: [AuthProvider: AuthProviderStatus] = [:]
            for provider in AuthProvider.allCases {
                next[provider] = AuthProviderStatus(provider: provider, status: .none, login: nil)
            }
            for status in statuses {
                next[status.provider] = status
            }
            providers = next
            pendingFlows = pendingFlows.filter { provider, _ in
                providers[provider]?.status == .pending
            }
            error = nil
            errorProvider = nil
        } catch {
            self.error = error.localizedDescription
            self.errorProvider = nil
        }
    }

    public func connect(_ provider: AuthProvider) async {
        guard let waveService else {
            error = "Connect to server first."
            errorProvider = provider
            return
        }

        setProvider(provider, status: .pending, login: providers[provider]?.login)
        error = nil
        errorProvider = nil

        do {
            let flow = try await waveService.startAuthFlow(provider: provider)
            pendingFlows[provider] = flow
            setProvider(provider, status: .pending, login: providers[provider]?.login)

            let preferredURL = flow.verification_uri_complete ?? flow.verification_uri
            if let url = URL(string: preferredURL) {
                browserLaunchRequest = BrowserLaunchRequest(provider: provider, url: url)
            } else {
                error = "Could not open provider auth URL. Copy the link manually."
                errorProvider = provider
            }
        } catch let serviceError as WaveServiceError {
            switch serviceError {
            case .serverError(let status, _) where status == 409:
                await refresh()
                setProvider(provider, status: .pending, login: providers[provider]?.login)
                error = nil
                errorProvider = nil
            default:
                pendingFlows.removeValue(forKey: provider)
                setProvider(provider, status: .none, login: nil)
                error = serviceError.localizedDescription
                errorProvider = provider
            }
        } catch let caughtError {
            pendingFlows.removeValue(forKey: provider)
            setProvider(provider, status: .none, login: nil)
            error = caughtError.localizedDescription
            errorProvider = provider
        }
    }

    public func disconnect(_ provider: AuthProvider) async {
        guard let waveService else {
            error = "Connect to server first."
            errorProvider = provider
            return
        }

        do {
            let status = try await waveService.disconnectProvider(provider: provider)
            providers[provider] = status
            pendingFlows.removeValue(forKey: provider)
            error = nil
            errorProvider = nil
        } catch let caughtError {
            error = caughtError.localizedDescription
            errorProvider = provider
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
            error = nil
            errorProvider = nil

        case .failed:
            setProvider(event.provider, status: .none, login: nil)
            pendingFlows.removeValue(forKey: event.provider)
            error = event.error ?? "Authentication failed."
            errorProvider = event.provider

        case .disconnected:
            setProvider(event.provider, status: .none, login: nil)
            pendingFlows.removeValue(forKey: event.provider)
            if errorProvider == event.provider {
                error = nil
                errorProvider = nil
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
