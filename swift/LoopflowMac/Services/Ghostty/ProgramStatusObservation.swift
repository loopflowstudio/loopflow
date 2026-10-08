import Darwin
import Foundation
import Loopflow
import Observation

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
    private var generation: Int64?
    private var writer: ProgramStatusWriter?
    private var publication: Task<Void, Never>?
    private var active = true

    func associate(sessionId: String?, terminalId: String?, generation: Int64?) {
        guard self.sessionId != sessionId || self.terminalId != terminalId || self.generation != generation else { return }
        let firstSessionReading = self.sessionId == sessionId && self.terminalId == terminalId && self.generation == nil
        writer?.stop()
        writer = nil
        association = UUID()
        self.sessionId = sessionId
        self.terminalId = terminalId
        self.generation = generation
        if !firstSessionReading { sessionReducer = ProgramStatusReducer() }
        observationError = nil
        if firstSessionReading { publish() }
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
        guard sessionReducer.snapshot.seen, let sessionId, let terminalId, let generation else { return }
        if writer == nil {
            let association = self.association
            writer = ProgramStatusWriter(sessionId: sessionId, terminalId: terminalId, generation: generation) { [weak self] error in
                Task { @MainActor in
                    guard let self, self.active, self.association == association else { return }
                    self.observationError = error
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

/// Queue-confined pipe owner. Nonblocking writes cannot stall terminal I/O, and
/// at most one replacement snapshot waits behind a partially written frame.
private final class ProgramStatusWriter: @unchecked Sendable {
    private let queue = DispatchQueue(label: "studio.loopflow.program-status")
    private let sessionId: String
    private let terminalId: String
    private let generation: Int64
    private let failure: @Sendable (String) -> Void
    private var process: Foundation.Process?
    private var input: FileHandle?
    private var pending: Data?
    private var frame = Data()
    private var offset = 0
    private var writable: DispatchSourceWrite?
    private var stopped = false
    private var drainDeadline: Date?
    private var last: ProgramStatusRecords?

    init(sessionId: String, terminalId: String, generation: Int64, failure: @escaping @Sendable (String) -> Void) {
        self.sessionId = sessionId
        self.terminalId = terminalId
        self.generation = generation
        self.failure = failure
    }

    func submit(_ snapshot: ProgramStatusRecords) {
        queue.async { [self] in
            guard !stopped, snapshot != last else { return }
            do {
                if process == nil { try start() }
                pending = try JSONEncoder().encode(snapshot) + Data([10])
                last = snapshot
                flush()
            } catch {
                fail(error.localizedDescription)
            }
        }
    }

    private func start() throws {
        let helper = try LocalWaveAgentLauncher.controlLfPath()
        let process = LocalWaveAgentLauncher.queryProcess(
            [helper, "session", "observe-status", sessionId, "--terminal", terminalId, "--generation", String(generation)]
        )
        let pipe = Pipe()
        process.standardInput = pipe
        process.standardOutput = FileHandle.nullDevice
        process.standardError = FileHandle.nullDevice
        process.terminationHandler = { [weak self] process in
            guard let self else { return }
            self.queue.async {
                if !self.stopped { self.fail("Status observation ended (\(process.terminationStatus))") }
            }
        }
        try process.run()
        pipe.fileHandleForReading.closeFile()
        input = pipe.fileHandleForWriting
        let fd = pipe.fileHandleForWriting.fileDescriptor
        _ = fcntl(fd, F_SETFL, fcntl(fd, F_GETFL) | O_NONBLOCK)
        _ = fcntl(fd, F_SETNOSIGPIPE, 1)
        self.process = process
    }

    private func observeWritable() {
        guard !stopped, !frame.isEmpty || pending != nil else {
            writable?.cancel()
            writable = nil
            return
        }
        guard writable == nil, let input else { return }
        let writable = DispatchSource.makeWriteSource(fileDescriptor: input.fileDescriptor, queue: queue)
        writable.setEventHandler { [self] in flush() }
        self.writable = writable
        writable.resume()
    }

    private func flush() {
        defer {
            if let deadline = drainDeadline, (frame.isEmpty && pending == nil) || Date() >= deadline {
                finish()
            } else {
                observeWritable()
            }
        }
        guard !stopped, let input else { return }
        if frame.isEmpty, let pending {
            frame = pending
            self.pending = nil
            offset = 0
        }
        guard !frame.isEmpty else { return }
        let written = frame.withUnsafeBytes { bytes in
            Darwin.write(input.fileDescriptor, bytes.baseAddress!.advanced(by: offset), bytes.count - offset)
        }
        if written > 0 {
            offset += written
            if offset == frame.count { frame = Data(); offset = 0 }
        } else if written < 0, errno != EAGAIN, errno != EINTR {
            fail("Status observation unavailable")
        }
    }

    private func fail(_ message: String) {
        failure(message)
        finish()
    }

    func stop(drain: Bool = false) {
        queue.async { [self] in
            if drain, !stopped {
                drainDeadline = Date().addingTimeInterval(2)
                flush()
                queue.asyncAfter(deadline: .now() + .seconds(2)) { [self] in finish() }
            } else {
                finish()
            }
        }
    }

    private func finish() {
        stopped = true
        writable?.cancel()
        writable = nil
        try? input?.close()
        input = nil
        pending = nil
        frame = Data()
        process = nil
    }
}
