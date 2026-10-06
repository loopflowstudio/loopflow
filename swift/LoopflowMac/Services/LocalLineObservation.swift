#if os(macOS)
import Darwin
import Foundation
import Loopflow

/// One owned `lf` reader speaking newline-delimited JSON over its pipes.
/// All mutable state and pipe I/O belong to queue. No callback touches AppKit.
final class LocalLineObservation<Frame: Sendable>: @unchecked Sendable {
    private let queue: DispatchQueue
    private let name: String
    private let process: Process
    private let input = Pipe()
    private let output = Pipe()
    private let diagnostics = Pipe()
    private let continuation: AsyncThrowingStream<Frame, any Error>.Continuation
    private let configurationChanged: @Sendable () throws -> Bool
    /// Runs on `queue`; a throw ends the observation.
    private let decode: (Data) throws -> Frame
    private var outputSource: DispatchSourceRead?
    private var errorSource: DispatchSourceRead?
    private var timer: DispatchSourceTimer?
    private var bytes = Data()
    private var searchedBytes = 0
    private var stderr = Data()
    private var pendingInput = Data()
    private var lastFrame = ProcessInfo.processInfo.systemUptime
    private var lastConfigurationCheck = ProcessInfo.processInfo.systemUptime
    private var stoppingAt: TimeInterval?
    private var terminated = false
    private var exited = false
    private var waiters: [CheckedContinuation<Void, Never>] = []
    private let frameLimit: Int

    struct Handle: Sendable {
        let frames: AsyncThrowingStream<Frame, any Error>
        /// Queue one newline-terminated request line.
        let send: @Sendable (Data) -> Void
        /// Returns only after the owned reader has exited.
        let cancel: @Sendable () async -> Void
    }

    /// `name` labels failures ("Active Session", "Workspace").
    static func start(
        name: String,
        process: Process,
        frameLimit: Int,
        buffering: AsyncThrowingStream<Frame, any Error>.Continuation.BufferingPolicy,
        configurationChanged: @escaping @Sendable () throws -> Bool,
        decode: @escaping (Data) throws -> Frame
    ) throws -> Handle {
        let (stream, continuation) = AsyncThrowingStream<Frame, any Error>
            .makeStream(bufferingPolicy: buffering)
        let reader = LocalLineObservation(name, process, frameLimit, continuation, configurationChanged, decode)
        // Stream cancellation also covers a consumer disappearing without an explicit stop.
        continuation.onTermination = { [weak reader] _ in
            reader?.queue.async { [weak reader] in reader?.stop() }
        }
        try reader.queue.sync { try reader.launch() }
        return Handle(frames: stream, send: { line in
            reader.queue.async { reader.enqueue(line) }
        }, cancel: {
            await withCheckedContinuation { waiter in
                reader.queue.async {
                    if reader.exited { waiter.resume(); return }
                    reader.waiters.append(waiter)
                    reader.stop()
                }
            }
        })
    }

    private init(
        _ name: String,
        _ process: Process,
        _ frameLimit: Int,
        _ continuation: AsyncThrowingStream<Frame, any Error>.Continuation,
        _ configurationChanged: @escaping @Sendable () throws -> Bool,
        _ decode: @escaping (Data) throws -> Frame
    ) {
        self.name = name
        self.queue = DispatchQueue(label: "studio.loopflow.reader.\(name)", qos: .utility)
        self.process = process
        self.frameLimit = frameLimit
        self.continuation = continuation
        self.configurationChanged = configurationChanged
        self.decode = decode
    }

    private func launch() throws {
        process.standardInput = input
        process.standardOutput = output
        process.standardError = diagnostics
        process.terminationHandler = { [self] _ in queue.async { self.didExit() } }
        do { try process.run() }
        catch {
            process.terminationHandler = nil
            continuation.finish(throwing: error)
            throw error
        }
        for handle in [input.fileHandleForWriting, output.fileHandleForReading, diagnostics.fileHandleForReading] {
            let fd = handle.fileDescriptor
            _ = fcntl(fd, F_SETFL, fcntl(fd, F_GETFL) | O_NONBLOCK)
            _ = fcntl(fd, F_SETNOSIGPIPE, 1)
        }
        outputSource = readSource(output.fileHandleForReading, isOutput: true)
        errorSource = readSource(diagnostics.fileHandleForReading, isOutput: false)
        let timer = DispatchSource.makeTimerSource(queue: queue)
        timer.schedule(deadline: .now() + .milliseconds(100), repeating: .milliseconds(100))
        timer.setEventHandler { [self] in tick() }
        self.timer = timer
        timer.resume()
    }

    private func readSource(_ handle: FileHandle, isOutput: Bool) -> DispatchSourceRead {
        let source = DispatchSource.makeReadSource(fileDescriptor: handle.fileDescriptor, queue: queue)
        source.setEventHandler { [self] in drain(handle, isOutput: isOutput) }
        source.setCancelHandler { try? handle.close() }
        source.resume()
        return source
    }

    private func drain(_ handle: FileHandle, isOutput: Bool) {
        var buffer = [UInt8](repeating: 0, count: 64 * 1024)
        // Yield to cancellation and timeout checks even when a child floods a pipe.
        for _ in 0..<16 {
            let count = Darwin.read(handle.fileDescriptor, &buffer, buffer.count)
            if count < 0 {
                if errno == EAGAIN || errno == EINTR { return }
                fail(RegistryQueryError("\(name) reader pipe failed"))
                return
            }
            if count == 0 {
                if isOutput {
                    outputSource?.cancel()
                    if stoppingAt == nil { readerEnded("\(name) reader closed its output") }
                } else { errorSource?.cancel() }
                return
            }
            if !isOutput {
                stderr.append(contentsOf: buffer.prefix(count))
                if stderr.count > 16 * 1024 { stderr.removeFirst(stderr.count - 16 * 1024) }
                continue
            }
            guard stoppingAt == nil else { continue }
            bytes.append(contentsOf: buffer.prefix(count))
            while let end = bytes.dropFirst(searchedBytes).firstIndex(of: 10) {
                guard bytes.distance(from: bytes.startIndex, to: end) <= frameLimit else {
                    fail(RegistryQueryError("\(name) frame exceeds \(frameLimit / (1024 * 1024)) MiB")); return
                }
                do {
                    let frame = try decode(Data(bytes[..<end]))
                    lastFrame = ProcessInfo.processInfo.systemUptime
                    continuation.yield(frame)
                } catch {
                    fail(RegistryQueryError("Invalid \(name.prefix(1).lowercased() + name.dropFirst()) observation: \(error.localizedDescription)"))
                    return
                }
                bytes.removeSubrange(...end)
                searchedBytes = 0
            }
            searchedBytes = bytes.count
            if bytes.count > frameLimit {
                fail(RegistryQueryError("\(name) frame exceeds \(frameLimit / (1024 * 1024)) MiB")); return
            }
        }
    }

    private func enqueue(_ line: Data) {
        guard stoppingAt == nil else { return }
        // A reader that stops draining its input has stopped; do not grow without bound.
        guard pendingInput.count + line.count <= 1024 * 1024 else {
            fail(RegistryQueryError("\(name) reader stopped accepting requests")); return
        }
        pendingInput.append(line)
        flushRequest()
    }

    private func flushRequest() {
        guard !pendingInput.isEmpty, stoppingAt == nil else { return }
        let count = pendingInput.withUnsafeBytes { Darwin.write(input.fileHandleForWriting.fileDescriptor, $0.baseAddress, $0.count) }
        // A nonblocking pipe may take part of a line; the rest follows on the next tick.
        if count > 0 { pendingInput.removeFirst(count) }
        else if count < 0 && (errno == EAGAIN || errno == EINTR) { return }
        else { fail(RegistryQueryError("\(name) reader stopped accepting requests")) }
    }

    private func tick() {
        let now = ProcessInfo.processInfo.systemUptime
        if let stoppingAt {
            guard process.isRunning else { return }
            if now - stoppingAt >= 2 {
                // Only the Process owned by this transport; never its process group.
                _ = Darwin.kill(process.processIdentifier, SIGKILL)
            } else if now - stoppingAt >= 1 && !terminated {
                terminated = true
                process.terminate()
            }
            return
        }
        do {
            if now - lastConfigurationCheck >= 2 {
                lastConfigurationCheck = now
                if try configurationChanged() {
                    fail(ActiveSessionsObservationError.configurationChanged)
                    return
                }
            }
            if now - lastFrame >= 10 {
                fail(RegistryQueryError("\(name) reader sent no observation for ten seconds"))
                return
            }
            flushRequest()
        } catch { fail(error) }
    }

    private func fail(_ error: any Error) {
        guard stoppingAt == nil else { return }
        continuation.finish(throwing: error)
        stop()
    }

    private func stop() {
        guard stoppingAt == nil else { return }
        stoppingAt = ProcessInfo.processInfo.systemUptime
        pendingInput.removeAll()
        bytes.removeAll()
        try? input.fileHandleForWriting.close()
        continuation.finish()
    }

    private func didExit() {
        if stoppingAt == nil {
            readerEnded("\(name) reader exited (\(process.terminationStatus))")
        }
        exited = true
        outputSource?.cancel()
        errorSource?.cancel()
        timer?.cancel()
        outputSource = nil
        errorSource = nil
        timer = nil
        process.terminationHandler = nil
        for waiter in waiters { waiter.resume() }
        waiters.removeAll()
    }

    private func readerEnded(_ fallback: String) {
        // stdout EOF and process exit can arrive before stderr's read callback.
        if let errorSource, !errorSource.isCancelled {
            drain(diagnostics.fileHandleForReading, isOutput: false)
        }
        let detail = String(decoding: stderr, as: UTF8.self).trimmingCharacters(in: .whitespacesAndNewlines)
        fail(RegistryQueryError(detail.isEmpty ? fallback : detail))
    }
}

enum LocalActiveSessionsObservation {
    static func start(
        process: Process,
        configurationChanged: @escaping @Sendable () throws -> Bool
    ) throws -> ActiveSessionsObservation {
        func resolved(_ path: String) -> String {
            URL(fileURLWithPath: path).resolvingSymlinksInPath().standardizedFileURL.path
        }
        let environment = process.environment ?? ProcessInfo.processInfo.environment
        var home = environment["LF_HOME"].flatMap { $0.isEmpty ? nil : resolved($0) }
        // A full queue keeps only the newest snapshot.
        let reader = try LocalLineObservation<ActiveSessionsSnapshot>.start(
            name: "Active Session", process: process, frameLimit: 16 * 1024 * 1024,
            buffering: .bufferingNewest(1), configurationChanged: configurationChanged
        ) { line in
            let snapshot = try JSONDecoder().decode(ActiveSessionsSnapshot.self, from: line)
            let observed = resolved(snapshot.home)
            guard home == nil || home == observed, snapshot.task == nil else {
                throw RegistryQueryError("Active Session reader changed Home or returned a Task-scoped frame")
            }
            home = observed
            return snapshot
        }
        return ActiveSessionsObservation(snapshots: reader.frames, request: { request in
            reader.send(Data("{\"action\":\"\(request.rawValue)\"}\n".utf8))
        }, cancel: reader.cancel)
    }
}

enum LocalWorkspaceObservation {
    /// Every frame is kept: the reader already holds at most one per part.
    static func start(
        process: Process,
        configurationChanged: @escaping @Sendable () throws -> Bool
    ) throws -> WorkspaceObservation {
        let reader = try LocalLineObservation<WorkspaceFrame>.start(
            name: "Workspace", process: process, frameLimit: 64 * 1024 * 1024,
            buffering: .unbounded, configurationChanged: configurationChanged,
            decode: WorkspaceFrame.decode(line:))
        return WorkspaceObservation(frames: reader.frames, request: { request in
            reader.send(request.line)
        }, cancel: reader.cancel)
    }
}
#endif
