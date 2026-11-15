import Foundation

/// One Machine link and one reconnect loop, shared by reads and receipt watches.
/// Only opening the transport retries here; commands retain their own recovery.
public actor MachineConnection {
    private struct Waiter {
        let reconnect: Bool
        let continuation: CheckedContinuation<ServerLink, any Error>
    }
    private let open: @Sendable () async throws -> ServerLink
    private var current: ServerLink?
    private var reopening = false
    private var opening: Task<Void, Never>?
    private var waiters: [UUID: Waiter] = [:]
    private nonisolated let state = ConnectionState()

    public init(open: @escaping @Sendable () async throws -> ServerLink) { self.open = open }

    public nonisolated var isWaiting: Bool { state.isWaiting }
    public nonisolated func changes() -> AsyncStream<Void> { state.changes() }

    public func link(reconnect: Bool = false) async throws -> ServerLink {
        try Task.checkCancellation()
        while let link = current {
            if await !link.isClosed { return link }
            if current === link { current = nil; reopening = true; state.setWaiting(true) }
        }
        let id = UUID()
        return try await withTaskCancellationHandler {
            try await withCheckedThrowingContinuation { continuation in
                guard !Task.isCancelled else { continuation.resume(throwing: CancellationError()); return }
                waiters[id] = Waiter(reconnect: reconnect, continuation: continuation)
                if opening == nil { opening = Task { await self.connect() } }
            }
        } onCancel: { Task { await self.cancel(id) } }
    }

    private func cancel(_ id: UUID) {
        waiters.removeValue(forKey: id)?.continuation.resume(throwing: CancellationError())
        if waiters.isEmpty { opening?.cancel(); opening = nil }
    }

    private func connect() async {
        var attempt = 0
        if reopening {
            reopening = false
            do { try await Task.sleep(for: .milliseconds(100)) }
            catch { return }
        }
        while !Task.isCancelled {
            do {
                let link = try await open()
                guard !Task.isCancelled else { await link.close(); return }
                current = link
                opening = nil
                state.setWaiting(false)
                let ready = waiters.values
                waiters = [:]
                for waiter in ready { waiter.continuation.resume(returning: link) }
                return
            } catch {
                guard !Task.isCancelled else { return }
                let unavailable = error is ServerConnectionUnavailable
                state.setWaiting(unavailable)
                let failed = waiters.filter { !unavailable || !$0.value.reconnect }
                for (id, waiter) in failed {
                    waiters[id] = nil
                    waiter.continuation.resume(throwing: error)
                }
                guard !waiters.isEmpty else { opening = nil; return }
                attempt = min(attempt + 1, 6)
                do { try await Task.sleep(for: .milliseconds(min(100 << (attempt - 1), 3200))) }
                catch { return }
            }
        }
    }
}

/// Synchronous status access keeps every watch on the same Machine fact.
private final class ConnectionState: @unchecked Sendable {
    private let lock = NSLock()
    private var waiting = false
    private var observers: [UUID: AsyncStream<Void>.Continuation] = [:]
    var isWaiting: Bool { lock.withLock { waiting } }

    func setWaiting(_ value: Bool) {
        lock.withLock {
            guard waiting != value else { return }
            waiting = value
            for observer in observers.values { observer.yield(()) }
        }
    }

    func changes() -> AsyncStream<Void> {
        let id = UUID()
        let (stream, continuation) = AsyncStream<Void>.makeStream(bufferingPolicy: .bufferingNewest(1))
        lock.withLock { observers[id] = continuation; continuation.yield(()) }
        continuation.onTermination = { [weak self] _ in
            guard let self else { return }
            _ = self.lock.withLock { self.observers.removeValue(forKey: id) }
        }
        return stream
    }
}
