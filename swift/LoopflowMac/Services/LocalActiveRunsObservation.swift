#if os(macOS)
import Darwin
import Foundation
import Loopflow

/// All mutable state and pipe I/O belong to queue. No callback touches AppKit.
final class LocalActiveRunsObservation: @unchecked Sendable {
    private let queue = DispatchQueue(label: "studio.loopflow.active-runs", qos: .utility)
    private let process: Process
    private let input = Pipe()
    private let output = Pipe()
    private let diagnostics = Pipe()
    private let continuation: AsyncThrowingStream<ActiveRunsSnapshot, any Error>.Continuation
    private let configurationChanged: @Sendable () throws -> Bool
    private var outputSource: DispatchSourceRead?
    private var errorSource: DispatchSourceRead?
    private var timer: DispatchSourceTimer?
    private var bytes = Data()
    private var searchedBytes = 0
    private var stderr = Data()
    private var pendingRequest: ActiveRunsObservation.Request?
    private var home: String?
    private var lastFrame = ProcessInfo.processInfo.systemUptime
    private var lastConfigurationCheck = ProcessInfo.processInfo.systemUptime
    private var stoppingAt: TimeInterval?
    private var terminated = false
    private var exited = false
    private var waiters: [CheckedContinuation<Void, Never>] = []
    private static let frameLimit = 16 * 1024 * 1024

    static func start(
        process: Process,
        configurationChanged: @escaping @Sendable () throws -> Bool
    ) throws -> ActiveRunsObservation {
        let (stream, continuation) = AsyncThrowingStream<ActiveRunsSnapshot, any Error>
            .makeStream(bufferingPolicy: .bufferingNewest(1))
        let reader = LocalActiveRunsObservation(process, continuation, configurationChanged)
        // Stream cancellation also covers a consumer disappearing without an explicit stop.
        continuation.onTermination = { [weak reader] _ in
            reader?.queue.async { [weak reader] in reader?.stop() }
        }
        try reader.queue.sync { try reader.launch() }
        return ActiveRunsObservation(snapshots: stream, request: { request in
            reader.queue.async { reader.enqueue(request) }
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
        _ process: Process,
        _ continuation: AsyncThrowingStream<ActiveRunsSnapshot, any Error>.Continuation,
        _ configurationChanged: @escaping @Sendable () throws -> Bool
    ) {
        self.process = process
        self.continuation = continuation
        self.configurationChanged = configurationChanged
        let environment = process.environment ?? ProcessInfo.processInfo.environment
        if let selected = environment["LF_CONTROL_HOME"] ?? environment["LF_HOME"], !selected.isEmpty {
            home = URL(fileURLWithPath: selected).resolvingSymlinksInPath().standardizedFileURL.path
        }
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
                fail(RegistryQueryError("Active Run reader pipe failed"))
                return
            }
            if count == 0 {
                if isOutput {
                    outputSource?.cancel()
                    if stoppingAt == nil { readerEnded("Active Run reader closed its output") }
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
                guard bytes.distance(from: bytes.startIndex, to: end) <= Self.frameLimit else {
                    fail(RegistryQueryError("Active Run frame exceeds 16 MiB")); return
                }
                let frame = bytes[..<end]
                do {
                    let snapshot = try JSONDecoder().decode(ActiveRunsSnapshot.self, from: frame)
                    let observedHome = URL(fileURLWithPath: snapshot.home).resolvingSymlinksInPath().standardizedFileURL.path
                    guard home == nil || home == observedHome, snapshot.task == nil else {
                        throw RegistryQueryError("Active Run reader changed Home or returned a Task-scoped frame")
                    }
                    home = observedHome
                    lastFrame = ProcessInfo.processInfo.systemUptime
                    continuation.yield(snapshot)
                } catch {
                    fail(RegistryQueryError("Invalid active Run observation: \(error.localizedDescription)"))
                    return
                }
                bytes.removeSubrange(...end)
                searchedBytes = 0
            }
            searchedBytes = bytes.count
            if bytes.count > Self.frameLimit {
                fail(RegistryQueryError("Active Run frame exceeds 16 MiB")); return
            }
        }
    }

    private func enqueue(_ request: ActiveRunsObservation.Request) {
        guard stoppingAt == nil else { return }
        if pendingRequest != .rescan { pendingRequest = request }
        flushRequest()
    }

    private func flushRequest() {
        guard let request = pendingRequest, stoppingAt == nil else { return }
        let data = Data("{\"action\":\"\(request.rawValue)\"}\n".utf8)
        let count = data.withUnsafeBytes { Darwin.write(input.fileHandleForWriting.fileDescriptor, $0.baseAddress, $0.count) }
        // Each request fits PIPE_BUF, so a nonblocking pipe write is all or nothing.
        if count == data.count { pendingRequest = nil }
        else if count < 0 && (errno == EAGAIN || errno == EINTR) { return }
        else { fail(RegistryQueryError("Active Run reader stopped accepting requests")) }
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
                    fail(ActiveRunsObservationError.configurationChanged)
                    return
                }
            }
            if now - lastFrame >= 10 {
                fail(RegistryQueryError("Active Run reader sent no observation for ten seconds"))
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
        pendingRequest = nil
        bytes.removeAll()
        try? input.fileHandleForWriting.close()
        continuation.finish()
    }

    private func didExit() {
        if stoppingAt == nil {
            readerEnded("Active Run reader exited (\(process.terminationStatus))")
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
#endif
