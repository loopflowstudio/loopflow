import Foundation
import Loopflow
import Observation

/// The provider named by one Session reading. A conversation Loopflow never
/// launched has no AgentProcess; that absence is itself the reading.
struct SessionProviderReading: Equatable, Sendable {
    let agentProcessId: AgentProcessId?

    init(agentProcessId: AgentProcessId?) { self.agentProcessId = agentProcessId }

    init?(record: SessionRecord, terminal: String) {
        guard record.state == .active, record.terminalIds.contains(terminal) else { return nil }
        agentProcessId = record.agentProcessId
    }
}

/// Owned by a retained surface, never by a SwiftUI mount or its current focus.
@MainActor @Observable
final class ProgramStatusSurface {
    let incarnation = UUID()
    private(set) var snapshot = ProgramStatusRecords(seen: false, records: [])
    private(set) var observationError: String?
    private var reducer = ProgramStatusReducer()
    private var sessionReducer = ProgramStatusReducer()
    private(set) var sessionId: String?
    private var association = UUID()
    private var terminalId: String?
    private var provider: SessionProviderReading?
    private var writer: ProgramStatusWriter?
    private var publication: Task<Void, Never>?
    private var active = true
    private var hasAssociatedSession = false
    private let send: Send

    /// Delivers one reading; throws when the server did not record it.
    typealias Send = @Sendable (ServedWrite) async throws -> Void

    init(send: @escaping Send = { _ = try await ServerClient.write($0) }) {
        self.send = send
    }

    func associate(sessionId: String?, terminalId: String?, provider: SessionProviderReading?) {
        guard self.sessionId != sessionId || self.terminalId != terminalId || self.provider != provider else { return }
        let firstSessionReading = self.sessionId == sessionId && self.terminalId == terminalId && self.provider == nil
        let firstAssociation = !hasAssociatedSession && sessionId != nil
        writer?.stop()
        writer = nil
        association = UUID()
        self.sessionId = sessionId
        self.terminalId = terminalId
        self.provider = provider
        if firstAssociation {
            // A restored screen can report before Desktop's Session subscription names
            // its owner. Preserve that first reading; later providers start empty.
            sessionReducer = reducer
            hasAssociatedSession = true
        } else if !firstSessionReading {
            sessionReducer = ProgramStatusReducer()
        }
        observationError = nil
        if firstSessionReading || firstAssociation { publish() }
    }

    /// The callback must capture this surface's incarnation before actor dispatch.
    func receive(_ event: ProgramStatusEvent, incarnation: UUID) {
        guard active, incarnation == self.incarnation else { return }
        reducer.apply(event)
        if sessionId != nil { sessionReducer.apply(event) }
        guard publication == nil else { return }
        publication = Task { [weak self] in
            try? await Task.sleep(for: .milliseconds(250))
            guard !Task.isCancelled, let self, self.active else { return }
            self.publication = nil
            self.publish()
        }
    }

    private func publish() {
        if snapshot != reducer.snapshot { snapshot = reducer.snapshot }
        guard sessionReducer.snapshot.seen, let sessionId, let terminalId, let provider else { return }
        if writer == nil {
            association = UUID()
            let association = self.association
            writer = ProgramStatusWriter(sessionId: sessionId, terminalId: terminalId, provider: provider, send: send) { [weak self] error in
                Task { @MainActor in
                    guard let self, self.active, self.association == association else { return }
                    self.observationError = error
                    if error != nil { self.writer = nil }
                }
            }
        }
        writer?.submit(sessionReducer.snapshot)
    }

    func close() {
        guard active else { return }
        reducer.apply(.exit)
        sessionReducer.apply(.exit)
        snapshot = reducer.snapshot
        if sessionReducer.snapshot.seen { writer?.submit(sessionReducer.snapshot) }
        active = false
        publication?.cancel()
        publication = nil
        writer?.stop(drain: true)
        writer = nil
    }
}

/// Sends each changed snapshot to the Machine server as one `session.status`
/// write. One request is in flight at a time and at most one newer snapshot
/// waits behind it, so a slow server never stalls terminal I/O.
private final class ProgramStatusWriter: @unchecked Sendable {
    private let queue = DispatchQueue(label: "studio.loopflow.program-status")
    private let sessionId: String
    private let terminalId: String
    private let provider: SessionProviderReading
    private let send: ProgramStatusSurface.Send
    private let observation: @Sendable (String?) -> Void
    /// A failed writer is replaced, and the server fences the stream it leaves.
    private let stream = UUID().uuidString.lowercased()
    private var sequence = 0
    private var pending: ProgramStatusRecords?
    private var sending = false
    private var stopped = false
    private var last: ProgramStatusRecords?

    init(sessionId: String, terminalId: String, provider: SessionProviderReading,
         send: @escaping ProgramStatusSurface.Send, observation: @escaping @Sendable (String?) -> Void) {
        self.sessionId = sessionId
        self.terminalId = terminalId
        self.provider = provider
        self.send = send
        self.observation = observation
    }

    func submit(_ snapshot: ProgramStatusRecords) {
        queue.async { [self] in
            guard !stopped, snapshot != last else { return }
            last = snapshot
            pending = snapshot
            flush()
        }
    }

    private func flush() {
        guard !sending, let snapshot = pending else { return }
        pending = nil
        sending = true
        sequence += 1
        let write = ServedWrite.sessionStatus(
            directory: "/", id: sessionId, terminal: terminalId, agentProcess: provider.agentProcessId?.rawValue,
            stream: stream, sequence: sequence, records: snapshot)
        Task { [self] in
            let failure: String?
            do {
                try await send(write)
                failure = nil
            } catch {
                failure = error.localizedDescription
            }
            queue.async { [self] in
                sending = false
                if let failure {
                    let report = !stopped
                    stopped = true
                    pending = nil
                    if report { observation(failure) }
                } else {
                    if !stopped { observation(nil) }
                    flush()
                }
            }
        }
    }

    /// Draining lets the snapshot already submitted reach the server.
    func stop(drain: Bool = false) {
        queue.async { [self] in
            stopped = true
            if !drain { pending = nil }
        }
    }
}
