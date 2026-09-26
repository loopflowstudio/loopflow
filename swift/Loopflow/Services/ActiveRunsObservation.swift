import Foundation

/// One foreground reader, shared by all Monitor panes in a window.
public struct ActiveRunsObservation: Sendable {
    public enum Request: String, Sendable {
        case refresh, rescan
    }

    public let snapshots: AsyncThrowingStream<ActiveRunsSnapshot, any Error>
    public let request: @Sendable (Request) async -> Void
    /// Returns only after the owned reader has exited.
    public let cancel: @Sendable () async -> Void

    public init(
        snapshots: AsyncThrowingStream<ActiveRunsSnapshot, any Error>,
        request: @escaping @Sendable (Request) async -> Void,
        cancel: @escaping @Sendable () async -> Void
    ) {
        self.snapshots = snapshots
        self.request = request
        self.cancel = cancel
    }
}

public enum ActiveRunsObservationError: Error, Sendable {
    /// Replace the reader only after draining its previous launch authority.
    case configurationChanged
}
