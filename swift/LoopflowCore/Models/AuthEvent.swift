// Provider-auth event value type.
//
// A live auth transition (a device-code flow starting, a provider connecting or
// failing). In the base wave model auth is a poll (`AuthProviderStore.refresh`
// over the REST provider list); this value type is the sink a future narrow
// `lf` auth SSE (see `scratch/eventing.md` §5b) would feed. It carries no
// transport — the old machine-wide `/ws` push is gone.

import Foundation

public struct AuthEvent: Sendable {
    public enum EventType: String, Sendable {
        case flowStarted = "auth.flow_started"
        case connected = "auth.connected"
        case failed = "auth.failed"
        case disconnected = "auth.disconnected"
    }

    public let type: EventType
    public let provider: AuthProvider
    public let verificationURI: String?
    public let verificationURIComplete: String?
    public let login: String?
    public let error: String?
    public let timestamp: Date

    public init(
        type: EventType,
        provider: AuthProvider,
        verificationURI: String? = nil,
        verificationURIComplete: String? = nil,
        login: String? = nil,
        error: String? = nil,
        timestamp: Date = Date()
    ) {
        self.type = type
        self.provider = provider
        self.verificationURI = verificationURI
        self.verificationURIComplete = verificationURIComplete
        self.login = login
        self.error = error
        self.timestamp = timestamp
    }
}
