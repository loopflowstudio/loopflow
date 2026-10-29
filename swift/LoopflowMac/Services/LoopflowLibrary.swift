#if os(macOS)
import Foundation
import Loopflow
import LoopflowFFI

/// Loopflow's reads served inside the app by its Rust library (`loopflow.h`),
/// with no `lf` process. Each returns what its `lf` command prints.
enum LoopflowLibrary {
    /// Serve `read` as `lf <read> --watch` streams it. Loopflow's
    /// wakeup only schedules a pull on this watch's own queue, where the
    /// reading runs; nothing reads on the caller's thread.
    static func watch(_ read: ServedRead) throws -> LineWatch {
        let served = ServedWatch(try JSONEncoder().encode(read))
        return LineWatch(lines: served.lines, send: served.send, cancel: served.cancel)
    }

    /// What `read.argv` prints, read off the caller's thread.
    static func read(_ read: ServedRead) async throws -> String {
        let json = String(decoding: try JSONEncoder().encode(read), as: UTF8.self)
        return try await Task.detached(priority: .userInitiated) {
            var error: UnsafeMutablePointer<CChar>?
            guard let result = lf_read(json, &error) else {
                throw RegistryQueryError(take(error) ?? "Loopflow could not read \(json)")
            }
            return take(result) ?? ""
        }.value
    }
}

/// A string Loopflow returned, freed.
private func take(_ string: UnsafeMutablePointer<CChar>?) -> String? {
    guard let string else { return nil }
    defer { lf_string_free(string) }
    return String(cString: string)
}

/// One subscription. Every call into Loopflow runs on `queue`; the wakeup
/// context retains the watch until it is unsubscribed.
private final class ServedWatch: @unchecked Sendable {
    let lines: AsyncThrowingStream<Data, any Error>
    private let continuation: AsyncThrowingStream<Data, any Error>.Continuation
    private let queue = DispatchQueue(label: "studio.loopflow.watch", qos: .utility)
    private var subscription: OpaquePointer?

    init(_ read: Data) {
        (lines, continuation) = AsyncThrowingStream.makeStream()
        continuation.onTermination = { [self] _ in queue.async { self.end() } }
        queue.async { self.subscribe(String(decoding: read, as: UTF8.self)) }
    }

    private func subscribe(_ read: String) {
        let context = Unmanaged.passRetained(self).toOpaque()
        var error: UnsafeMutablePointer<CChar>?
        subscription = lf_subscribe(read, { context in
            guard let context else { return }
            let watch = Unmanaged<ServedWatch>.fromOpaque(context).takeUnretainedValue()
            watch.queue.async { watch.pull() }
        }, context, &error)
        if subscription == nil {
            Unmanaged<ServedWatch>.fromOpaque(context).release()
            continuation.finish(throwing: RegistryQueryError(take(error) ?? "Loopflow could not serve \(read)"))
        }
    }

    private func pull() {
        guard let subscription else { return }
        var length = 0
        let line = lf_pull(subscription, &length).map { frame in
            defer { lf_frame_free(frame, length) }
            return Data(bytes: frame, count: length)
        }
        if let line { continuation.yield(line) }
        // A failed watch ends, as its `lf` process would have exited, on the
        // pull that returned its `unavailable` frame.
        if lf_ended(subscription) {
            let failure = line.flatMap { try? JSONSerialization.jsonObject(with: $0) as? [String: Any] }?["unavailable"]
            continuation.finish(throwing: RegistryQueryError(failure as? String ?? "Loopflow stopped serving this read"))
            end()
        }
    }

    func send(_ line: Data) {
        queue.async { [self] in
            guard let subscription else { return }
            line.withUnsafeBytes { bytes in
                lf_request(subscription, bytes.bindMemory(to: UInt8.self).baseAddress, bytes.count)
            }
        }
    }

    func cancel() async {
        await withCheckedContinuation { (done: CheckedContinuation<Void, Never>) in
            queue.async { [self] in
                end()
                done.resume()
            }
        }
    }

    /// No wakeup follows `lf_unsubscribe`, so the context's retain ends here.
    private func end() {
        continuation.finish()
        guard let subscription else { return }
        self.subscription = nil
        lf_unsubscribe(subscription)
        Unmanaged.passUnretained(self).release()
    }
}
#endif
