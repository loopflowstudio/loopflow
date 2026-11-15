// The `lf desktop` API: newline-delimited JSON over a same-user Unix socket at
// `$LF_HOME/run/desktop/<bundle-id>.sock`. It lives and dies with the app and
// serves only what `lf desktop` needs — inspection, registry actions, terminal
// text and change events. Sessions never run here; lf's detached transport owns them.
//
// Request  `{"v":1,"id":N,"method":"inspect"|"perform"|"read_text"|"events","params":{…}}`
// Reply    `{"id":N,"result":…}` or `{"id":N,"error":{"code":"argument"|"target"|"version"|"internal","message":…}}`
// `events` replies once, then writes `{"event":DesktopChange}` lines until the client closes.

#if os(macOS)
import Darwin
import Foundation
import Loopflow

final class DesktopServer: @unchecked Sendable {
    /// Bumped when a method or its shape changes; `lf desktop` names the fix on mismatch.
    static let version = 1

    let path: String
    private let listener: Int32
    private let acceptQueue = DispatchQueue(label: "studio.loopflow.desktop-server")
    private var acceptSource: DispatchSourceRead?
    /// Main-actor state: subscribed connections and the last window each saw.
    private var subscribers: [ObjectIdentifier: Connection] = [:]
    private var published: [String: Data] = [:]
    private var poll: Timer?
    private let inspect: @MainActor () -> DesktopInspection
    private let perform: @MainActor (DesktopActionRequest) throws -> DesktopActionReceipt
    private let readText: @MainActor (DesktopTextRequest) throws -> DesktopTextReading

    /// The socket this Desktop serves: `LF_HOME` (else `~/.lf`), then its bundle identifier.
    static func socketPath(home: String? = ProcessInfo.processInfo.environment["LF_HOME"],
                           bundle: String = Bundle.main.bundleIdentifier ?? "com.loopflow.mac") -> String {
        let root = home.flatMap { $0.isEmpty ? nil : $0 } ?? (NSHomeDirectory() + "/.lf")
        return root + "/run/desktop/\(bundle).sock"
    }

    /// Serves the shared router. A failure is logged; Desktop runs without its API.
    @MainActor
    static func start(path: String = socketPath()) -> DesktopServer? {
        let router = WorkLinkRouter.shared
        do {
            return try DesktopServer(path: path, inspect: { router.inspect() },
                                     perform: { try router.perform($0) }, readText: { try router.readText($0) })
        } catch {
            DesktopLog.home.error("Desktop API unavailable", ["path": path, "error": error.localizedDescription])
            return nil
        }
    }

    @MainActor
    init(path: String,
         inspect: @escaping @MainActor () -> DesktopInspection,
         perform: @escaping @MainActor (DesktopActionRequest) throws -> DesktopActionReceipt,
         readText: @escaping @MainActor (DesktopTextRequest) throws -> DesktopTextReading) throws {
        self.path = path
        self.inspect = inspect
        self.perform = perform
        self.readText = readText
        let directory = (path as NSString).deletingLastPathComponent
        try FileManager.default.createDirectory(atPath: directory, withIntermediateDirectories: true,
                                                attributes: [.posixPermissions: 0o700])
        chmod(directory, 0o700)
        var address = sockaddr_un()
        address.sun_family = sa_family_t(AF_UNIX)
        let capacity = MemoryLayout.size(ofValue: address.sun_path)
        guard path.utf8.count < capacity else { throw RegistryQueryError("Socket path exceeds \(capacity - 1) bytes") }
        withUnsafeMutableBytes(of: &address.sun_path) { buffer in
            buffer.copyBytes(from: path.utf8)
        }
        let fd = socket(AF_UNIX, SOCK_STREAM, 0)
        guard fd >= 0 else { throw RegistryQueryError("socket() failed: \(String(cString: strerror(errno)))") }
        // A file left by a Desktop that crashed is stale: this process owns the bundle's socket now.
        unlink(path)
        let bound = withUnsafePointer(to: &address) {
            $0.withMemoryRebound(to: sockaddr.self, capacity: 1) {
                bind(fd, $0, socklen_t(MemoryLayout<sockaddr_un>.size))
            }
        }
        guard bound == 0, chmod(path, 0o600) == 0, listen(fd, 16) == 0 else {
            let reason = String(cString: strerror(errno))
            close(fd)
            throw RegistryQueryError("bind \(path) failed: \(reason)")
        }
        listener = fd
        startAccepting()
    }

    /// Nonisolated, so the handler is not inferred to the main actor it never runs on.
    private func startAccepting() {
        let source = DispatchSource.makeReadSource(fileDescriptor: listener, queue: acceptQueue)
        source.setEventHandler { [weak self] in self?.accept() }
        source.resume()
        acceptSource = source
    }

    deinit {
        acceptSource?.cancel()
        close(listener)
        unlink(path)
    }

    private func accept() {
        let fd = Darwin.accept(listener, nil, nil)
        guard fd >= 0 else { return }
        var uid: uid_t = 0
        var gid: gid_t = 0
        guard getpeereid(fd, &uid, &gid) == 0, uid == getuid() else {
            close(fd)
            return
        }
        var on: Int32 = 1
        setsockopt(fd, SOL_SOCKET, SO_NOSIGPIPE, &on, socklen_t(MemoryLayout<Int32>.size))
        let connection = Connection(fd: fd)
        Thread.detachNewThread { [weak self] in
            connection.readLines { line in self?.handle(line, on: connection) }
            DispatchQueue.main.async { [weak self] in
                MainActor.assumeIsolated { self?.unsubscribe(connection) }
            }
            connection.close()
        }
    }

    /// Runs on the connection's reader thread.
    private func handle(_ line: Data, on connection: Connection) {
        DispatchQueue.main.sync {
            MainActor.assumeIsolated {
                let (reply, subscribes) = self.reply(to: line)
                // The acknowledgement precedes the first frame on the same serial writer.
                connection.write(reply)
                if subscribes { self.subscribe(connection) }
            }
        }
    }

    @MainActor
    private func reply(to line: Data) -> (Data, subscribes: Bool) {
        let decoder = JSONDecoder()
        guard let envelope = try? decoder.decode(Envelope.self, from: line) else {
            return (Self.encode(ErrorReply(id: nil, error: .init(code: "argument", message: "Desktop could not read the request line."))), false)
        }
        guard envelope.v == Self.version else {
            return (Self.encode(ErrorReply(id: envelope.id, error: .init(code: "version",
                message: "This Desktop speaks API version \(Self.version), not \(envelope.v)."))), false)
        }
        do {
            switch envelope.method {
            case "inspect":
                return (Self.encode(Reply(id: envelope.id, result: inspect())), false)
            case "perform":
                return (Self.encode(Reply(id: envelope.id, result: try perform(Self.params(DesktopActionRequest.self, line)))), false)
            case "read_text":
                return (Self.encode(Reply(id: envelope.id, result: try readText(Self.params(DesktopTextRequest.self, line)))), false)
            case "events":
                return (Self.encode(Reply(id: envelope.id, result: [String: String]())), true)
            default:
                throw DesktopArgumentError("Desktop has no method \(envelope.method).")
            }
        } catch {
            let code = switch error {
            case is DesktopTargetError: "target"
            case is DesktopArgumentError: "argument"
            default: "internal"
            }
            return (Self.encode(ErrorReply(id: envelope.id, error: .init(code: code, message: error.localizedDescription))), false)
        }
    }

    private static func params<P: Decodable>(_ type: P.Type, _ line: Data) throws -> P {
        do { return try JSONDecoder().decode(Params<P>.self, from: line).params }
        catch { throw DesktopArgumentError("Desktop could not read the request: \(error.localizedDescription)") }
    }

    private static func encode<T: Encodable>(_ value: T) -> Data {
        var data = (try? JSONEncoder().encode(value)) ?? Data(#"{"error":{"code":"internal","message":"encoding failed"}}"#.utf8)
        data.append(10)
        return data
    }

    // MARK: Events

    @MainActor
    private func subscribe(_ connection: Connection) {
        subscribers[ObjectIdentifier(connection)] = connection
        // The first frame is the whole reading; later frames carry only what changed.
        let reading = inspect()
        connection.write(Self.encode(EventFrame(event: DesktopChange(observedAt: reading.observedAt,
            windows: reading.windows, closed: []))))
        if poll == nil {
            published = Self.windows(reading)
            // Desktop state spans Observation, ObservableObject and AppKit; one coalesced
            // reading every quarter second is the single change signal, and only while subscribed.
            poll = Timer.scheduledTimer(withTimeInterval: 0.25, repeats: true) { [weak self] _ in
                MainActor.assumeIsolated { self?.publish() }
            }
        }
    }

    @MainActor
    private func unsubscribe(_ connection: Connection) {
        subscribers[ObjectIdentifier(connection)] = nil
        if subscribers.isEmpty {
            poll?.invalidate()
            poll = nil
            published = [:]
        }
    }

    @MainActor
    private func publish() {
        let reading = inspect()
        let current = Self.windows(reading)
        let changed = reading.windows.filter { current[$0.window] != published[$0.window] }
        let closed = published.keys.filter { current[$0] == nil }.sorted()
        published = current
        guard !changed.isEmpty || !closed.isEmpty else { return }
        let frame = Self.encode(EventFrame(event: DesktopChange(observedAt: reading.observedAt, windows: changed, closed: closed)))
        for connection in subscribers.values { connection.write(frame) }
    }

    private static func windows(_ reading: DesktopInspection) -> [String: Data] {
        Dictionary(reading.windows.map { ($0.window, (try? JSONEncoder().encode($0)) ?? Data()) },
                   uniquingKeysWith: { first, _ in first })
    }

    // MARK: Wire

    private struct Envelope: Decodable {
        let v: Int
        let id: Int
        let method: String
    }

    private struct Params<P: Decodable>: Decodable {
        let params: P
    }

    private struct Reply<R: Encodable>: Encodable {
        let id: Int
        let result: R
    }

    private struct ErrorReply: Encodable {
        struct Failure: Encodable {
            let code: String
            let message: String
        }
        let id: Int?
        let error: Failure
    }

    private struct EventFrame: Encodable {
        let event: DesktopChange
    }

    /// One accepted client. Reads block its own thread; writes are serialized.
    private final class Connection: @unchecked Sendable {
        let fd: Int32
        private let writes = DispatchQueue(label: "studio.loopflow.desktop-server.connection")
        private var closed = false

        init(fd: Int32) { self.fd = fd }

        /// Calls `line` for each newline-terminated request until EOF.
        func readLines(_ line: (Data) -> Void) {
            var pending = Data()
            var buffer = [UInt8](repeating: 0, count: 64 * 1024)
            while true {
                let count = Darwin.read(fd, &buffer, buffer.count)
                if count < 0, errno == EINTR { continue }
                guard count > 0 else { return }
                pending.append(contentsOf: buffer.prefix(count))
                // A request is small; an unbounded line is a broken client.
                guard pending.count <= 4 * 1024 * 1024 else { return }
                while let end = pending.firstIndex(of: 10) {
                    let request = pending[pending.startIndex..<end]
                    pending.removeSubrange(pending.startIndex...end)
                    if !request.isEmpty { line(Data(request)) }
                }
            }
        }

        func write(_ data: Data) {
            writes.async { [self] in
                guard !closed else { return }
                data.withUnsafeBytes { raw in
                    var offset = 0
                    while offset < raw.count {
                        let written = Darwin.write(fd, raw.baseAddress! + offset, raw.count - offset)
                        if written < 0, errno == EINTR { continue }
                        guard written > 0 else { closed = true; return }
                        offset += written
                    }
                }
            }
        }

        func close() {
            writes.async { [self] in
                closed = true
                Darwin.close(fd)
            }
        }
    }
}
#endif
