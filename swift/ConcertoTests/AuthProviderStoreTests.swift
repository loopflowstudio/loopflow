import Foundation
import Testing
@testable import LoopflowCore

@MainActor
@Suite("Auth provider store")
struct AuthProviderStoreTests {
    @Test("ordered returns all providers in stable order")
    func orderedProvidersStable() {
        let store = AuthProviderStore()

        let providers = store.ordered
        #expect(providers.map(\.provider) == AuthProvider.allCases)
        #expect(providers.allSatisfy { $0.status == .none })
    }

    @Test("connect handles 409 conflict by refreshing and keeping pending")
    func connectConflictKeepsPending() async {
        let service = MockAuthProviderService(
            listResponses: [
                .success([
                    AuthProviderStatus(provider: .github, status: .pending),
                    AuthProviderStatus(provider: .claude, status: .none),
                    AuthProviderStatus(provider: .codex, status: .none),
                ])
            ],
            startResults: [
                .github: .failure(WaveServiceError.serverError(status: 409, message: "already pending"))
            ]
        )

        let store = AuthProviderStore()
        store.bindService(service)

        await store.connect(.github)

        #expect(store.providers[.github]?.status == .pending)
        #expect(store.error == nil)
        #expect(await service.startCallCount == 1)
        #expect(await service.listCallCount == 1)
    }

    @Test("auth events reconcile pending, active, failed, and disconnected")
    func authEventReconciliation() {
        let store = AuthProviderStore()

        store.handleEvent(makeAuthEvent(type: .flowStarted, verificationURI: "https://github.com/login/device"))

        #expect(store.providers[.github]?.status == .pending)
        #expect(store.pendingFlows[.github] != nil)

        store.handleEvent(makeAuthEvent(type: .connected, login: "octocat"))

        #expect(store.providers[.github]?.status == .active)
        #expect(store.providers[.github]?.login == "octocat")
        #expect(store.pendingFlows[.github] == nil)

        store.handleEvent(makeAuthEvent(type: .failed, error: "Denied"))

        #expect(store.providers[.github]?.status == ProviderAuthStatus.none)
        #expect(store.error == "Denied")

        store.handleEvent(makeAuthEvent(type: .disconnected))

        #expect(store.providers[.github]?.status == ProviderAuthStatus.none)
        #expect(store.pendingFlows[.github] == nil)
    }

    @Test("refresh runs when connection transitions to connected")
    func refreshOnReconnectTransition() async {
        let service = MockAuthProviderService(
            listResponses: [
                .success([
                    AuthProviderStatus(provider: .github, status: .active, login: "octocat")
                ]),
                .success([
                    AuthProviderStatus(provider: .github, status: .none)
                ]),
            ]
        )

        let store = AuthProviderStore()
        store.bindService(service)

        await store.handleConnectionState(.connecting(.wsProbe))
        #expect(await service.listCallCount == 0)

        await store.handleConnectionState(.connected)
        #expect(await service.listCallCount == 1)
        #expect(store.providers[.github]?.status == .active)

        await store.handleConnectionState(.connected)
        #expect(await service.listCallCount == 1)

        await store.handleConnectionState(.disconnected(nil))
        await store.handleConnectionState(.connected)
        #expect(await service.listCallCount == 2)
        #expect(store.providers[.github]?.status == ProviderAuthStatus.none)
    }

    @Test("disconnect failure keeps current status and surfaces provider error")
    func disconnectFailurePreservesStatus() async {
        let service = MockAuthProviderService(
            disconnectResults: [
                .github: .failure(WaveServiceError.commandFailed("Disconnect failed"))
            ]
        )

        let store = AuthProviderStore()
        store.bindService(service)
        store.handleEvent(makeAuthEvent(type: .connected, login: "octocat"))

        await store.disconnect(.github)

        #expect(store.providers[.github]?.status == .active)
        #expect(store.errorProvider == .github)
        #expect(store.error == "Disconnect failed")
    }
}

private func makeAuthEvent(
    type: AuthEvent.EventType,
    provider: AuthProvider = .github,
    verificationURI: String? = nil,
    verificationURIComplete: String? = nil,
    login: String? = nil,
    error: String? = nil
) -> AuthEvent {
    AuthEvent(
        type: type,
        provider: provider,
        verificationURI: verificationURI,
        verificationURIComplete: verificationURIComplete,
        login: login,
        error: error,
        timestamp: Date()
    )
}

private actor MockAuthProviderService: AuthProviderService {
    private var listResponses: [Result<[AuthProviderStatus], Error>]
    private var startResults: [AuthProvider: Result<AuthFlow, Error>]
    private var disconnectResults: [AuthProvider: Result<AuthProviderStatus, Error>]

    private(set) var listCallCount = 0
    private(set) var startCallCount = 0
    private(set) var disconnectCallCount = 0

    init(
        listResponses: [Result<[AuthProviderStatus], Error>] = [],
        startResults: [AuthProvider: Result<AuthFlow, Error>] = [:],
        disconnectResults: [AuthProvider: Result<AuthProviderStatus, Error>] = [:]
    ) {
        self.listResponses = listResponses
        self.startResults = startResults
        self.disconnectResults = disconnectResults
    }

    func listAuthProviders() async throws -> [AuthProviderStatus] {
        listCallCount += 1
        if listResponses.isEmpty {
            return []
        }
        return try listResponses.removeFirst().get()
    }

    func startAuthFlow(provider: AuthProvider) async throws -> AuthFlow {
        startCallCount += 1
        if let result = startResults[provider] {
            return try result.get()
        }

        return AuthFlow(
            provider: provider,
            verification_uri: "https://example.com/device"
        )
    }

    func disconnectProvider(provider: AuthProvider) async throws -> AuthProviderStatus {
        disconnectCallCount += 1
        if let result = disconnectResults[provider] {
            return try result.get()
        }

        return AuthProviderStatus(provider: provider, status: .none)
    }
}
