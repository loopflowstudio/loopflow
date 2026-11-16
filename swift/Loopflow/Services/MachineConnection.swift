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
    private nonisolated let state: ConnectionState

    public init(machine: String? = nil, open: @escaping @Sendable () async throws -> ServerLink) {
        self.open = open
        state = ConnectionState(machine: machine)
    }

    /// Cached destinations keep offline changes on their original Machine.
    nonisolated func rememberMachine(_ machine: String) { state.rememberMachine(machine) }
    nonisolated func isWaiting(for machine: String) -> Bool { state.isWaiting(for: machine) }

    nonisolated func isCurrent(_ link: ServerLink) -> Bool { state.isCurrent(link) }
    nonisolated func disconnected(_ link: ServerLink) { state.disconnect(link) }
    nonisolated func establish(_ link: ServerLink, attempt: UUID) -> Bool {
        state.connected(link, attempt: attempt)
    }
    /// Nil while disconnected. Saved observation values then remain last-known.
    public nonisolated var observationSource: String? { state.observationSource }
    struct ObservationConnection: Equatable, Sendable {
        let source: String
        let generation: UUID
    }
    nonisolated var observationConnection: ObservationConnection? { state.observationConnection }

    nonisolated func validate(_ received: ReceivingLink?, machine: String) -> ObservationConnection? {
        state.validate(received, machine: machine)
    }

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
        guard !Task.isCancelled else { return }
        let generation = state.beginOpening()
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
                guard try await link.establish(self, attempt: generation), !Task.isCancelled else {
                    await link.close()
                    return
                }
                current = link
                if case .string(let machine) = link.welcome.machine["id"] { state.identify(machine) }
                opening = nil
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
    private var attempt: UUID?
    private var generation: UUID?
    private var source: String?
    var observationSource: String? { lock.withLock { source } }
    var observationConnection: MachineConnection.ObservationConnection? {
        lock.withLock {
            guard let source, let generation else { return nil }
            return MachineConnection.ObservationConnection(source: source, generation: generation)
        }
    }
    func validate(_ received: ReceivingLink?, machine: String) -> MachineConnection.ObservationConnection? {
        lock.withLock {
            guard let received, received.machine == machine, self.machine == machine,
                  received.generation == generation, received.source == source else { return nil }
            return MachineConnection.ObservationConnection(source: received.source, generation: received.generation)
        }
    }
    func isCurrent(_ link: ServerLink) -> Bool { lock.withLock { generation == link.generation } }
    func beginOpening() -> UUID {
        lock.withLock {
            let next = UUID()
            attempt = next
            generation = nil
            source = nil
            return next
        }
    }
    func connected(_ link: ServerLink, attempt: UUID) -> Bool {
        lock.withLock {
            guard self.attempt == attempt else { return false }
            if case .string(let machine) = link.welcome.machine["id"] { self.machine = machine }
            generation = link.generation
            source = link.welcome.observations
            waiting = false
            for observer in observers.values { observer.yield(()) }
            return true
        }
    }
    func disconnect(_ link: ServerLink) {
        lock.withLock {
            guard generation == link.generation else { return }
            generation = nil
            source = nil
            waiting = true
            for observer in observers.values { observer.yield(()) }
        }
    }
    private var machine: String?
    init(machine: String?) { self.machine = machine }
    private var observers: [UUID: AsyncStream<Void>.Continuation] = [:]
    var isWaiting: Bool { lock.withLock { waiting } }

    func isWaiting(for machine: String) -> Bool { lock.withLock { waiting && self.machine == machine } }
    func rememberMachine(_ machine: String) {
        lock.withLock { if self.machine == nil { self.machine = machine } }
    }
    func identify(_ machine: String) {
        lock.withLock {
            guard self.machine != machine else { return }
            self.machine = machine
            for observer in observers.values { observer.yield(()) }
        }
    }

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
