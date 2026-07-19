import Foundation

// Live client for a wave's chat server. A running `lf wave <name>` publishes its
// loopback address to `wave/<name>/.wave-endpoint`; this client discovers it,
// consumes the unified `GET /events` SSE stream (turn, turn-delta, state, and
// playhead frames; thread replay on connect), and posts messages back. When the
// pointer file is absent or the server refuses the connection, the connection
// settles into `.notRunning` and keeps polling so it attaches when the wave starts.

public enum WaveChatError: Error, Sendable {
    case notRunning
    case badStatus(Int)
    case badEndpoint(String)
}

/// Evidence state of a read-only fold over the durable Wave journal.
public enum ChatHistoryState: String, Codable, Sendable, Equatable {
    case available
    case missing
    case partial
    case unavailable
}

/// Bounded `lf chat --history --json` response. Every field mirrors Rust.
public struct ChatHistorySnapshot: Codable, Sendable, Equatable {
    public let state: ChatHistoryState
    public let detail: String?
    public let turns: [ChatTurn]
    public let truncated: Bool

    public init(
        state: ChatHistoryState,
        detail: String?,
        turns: [ChatTurn],
        truncated: Bool
    ) {
        self.state = state
        self.detail = detail
        self.turns = turns
        self.truncated = truncated
    }
}

public typealias ChatHistoryLoader = @Sendable (
    _ repoPath: String,
    _ waveName: String,
    _ limit: Int
) async throws -> ChatHistorySnapshot

/// Reads the discovery pointer a running wave writes under its `wave/<name>/` dir.
///
/// Wave state lives at the wave's ORIGIN repo: a running wave publishes its
/// endpoint to the main checkout even when Loopflow was pointed at a worktree,
/// so `repoPath` goes through `WaveOrigin.resolve` before the read. Every
/// consumer (chat discovery, the launcher's double-launch guard) shares this
/// one resolution — guard and reader can't disagree.
public enum WaveEndpoint {
    public static let fileName = ".wave-endpoint"

    public static func path(repoPath: String, waveName: String) -> URL {
        URL(fileURLWithPath: WaveOrigin.resolve(repoPath))
            .appendingPathComponent("wave", isDirectory: true)
            .appendingPathComponent(waveName, isDirectory: true)
            .appendingPathComponent(fileName)
    }

    /// The `127.0.0.1:<port>` the wave is serving on, or nil when it isn't running.
    public static func read(repoPath: String, waveName: String) -> String? {
        let url = path(repoPath: repoPath, waveName: waveName)
        guard let raw = try? String(contentsOf: url, encoding: .utf8) else { return nil }
        let trimmed = raw.trimmingCharacters(in: .whitespacesAndNewlines)
        return trimmed.isEmpty ? nil : trimmed
    }
}

/// Incremental SSE frame parser, fed raw bytes. Line splitting is hand-rolled
/// because `URLSession.AsyncBytes.lines` silently drops empty lines — and the
/// empty line is precisely what terminates an SSE frame. (Observed live: with
/// `.lines`, no frame boundary ever fired, so nothing the server streamed —
/// replay, turns, loop state — reached the UI.) Comment lines (`:` keep-alive
/// pings) drop; `event:` names the pending frame; `data:` lines accumulate,
/// joined by `\n`; a blank line emits the frame when it carries data.
struct SSEFrameParser {
    struct Frame: Equatable {
        let event: String
        let data: String
    }

    /// Feed a whole chunk; frames complete inside it, in order. The byte loop
    /// stays synchronous — see `SSEChunkStream` for why that matters.
    mutating func consume(_ chunk: Data) -> [Frame] {
        var frames: [Frame] = []
        for byte in chunk {
            if let frame = consume(byte) { frames.append(frame) }
        }
        return frames
    }

    private var line: [UInt8] = []
    private var event = ""
    private var data = ""

    /// Feed one byte; the completed frame comes back on the blank line that ends it.
    mutating func consume(_ byte: UInt8) -> Frame? {
        guard byte == UInt8(ascii: "\n") else {
            line.append(byte)
            return nil
        }
        if line.last == UInt8(ascii: "\r") { line.removeLast() }
        let text = String(decoding: line, as: UTF8.self)
        line.removeAll(keepingCapacity: true)
        return consumeLine(text)
    }

    private mutating func consumeLine(_ line: String) -> Frame? {
        if line.isEmpty {
            defer {
                event = ""
                data = ""
            }
            return data.isEmpty ? nil : Frame(event: event, data: data)
        }
        if line.hasPrefix(":") { return nil }
        if line.hasPrefix("event:") {
            event = String(line.dropFirst("event:".count)).trimmingCharacters(in: .whitespaces)
        } else if line.hasPrefix("data:") {
            let chunk = String(line.dropFirst("data:".count)).trimmingCharacters(in: .whitespaces)
            data += data.isEmpty ? chunk : "\n" + chunk
        }
        return nil
    }
}

/// Reads an SSE response as `Data` chunks.
///
/// `URLSession.AsyncBytes` yields one byte per `await`, and `WaveChatConnection`
/// is `@MainActor` — so a per-byte read loop paid a **main-actor hop per byte**.
/// Measured on a real connection: 0.14 MB/s (the same loop off the main actor
/// runs ~140x faster, which is why the isolation, not the byte count, is the
/// thing to keep in mind).
///
/// That is survivable only while frames are small, and they are not: the
/// listener re-sends the whole open turn — prose plus every accumulated tool
/// output — on every token, so a frame reaches ~106 KB and one turn puts ~68 MB
/// on the wire. Byte-at-a-time, one frame took ~734 ms to read, the connect
/// replay took ~22 s, and a live turn arrived at about a word per second. That
/// was the whole of the symptom.
///
/// Chunks pay one hop per network read instead of one per byte — measured at
/// >200 MB/s — while the parse inside a chunk stays byte-level, so the empty
/// line that delimits an SSE frame still registers (`.lines` drops it; see
/// `SSEFrameParser`). `WaveChatStreamTests` holds the budget.
final class SSEChunkStream: NSObject, URLSessionDataDelegate, @unchecked Sendable {
    private let lock = NSLock()
    private var chunks: AsyncThrowingStream<Data, Error>.Continuation?
    private var awaitingResponse: CheckedContinuation<HTTPURLResponse, Error>?
    private var task: URLSessionDataTask?

    /// The response headers, once the server has sent them. Throws if the
    /// connection fails before that.
    func response() async throws -> HTTPURLResponse {
        try await withCheckedThrowingContinuation { continuation in
            lock.lock()
            awaitingResponse = continuation
            lock.unlock()
        }
    }

    /// Start `request` on `session` and stream its body. Cancelling the
    /// consuming task cancels the transfer.
    func connect(_ request: URLRequest, on session: URLSession) -> AsyncThrowingStream<Data, Error> {
        let dataTask = session.dataTask(with: request)
        dataTask.delegate = self
        lock.lock()
        task = dataTask
        lock.unlock()
        return AsyncThrowingStream { continuation in
            lock.lock()
            chunks = continuation
            lock.unlock()
            continuation.onTermination = { _ in dataTask.cancel() }
            dataTask.resume()
        }
    }

    func urlSession(
        _ session: URLSession, dataTask: URLSessionDataTask, didReceive response: URLResponse
    ) async -> URLSession.ResponseDisposition {
        let waiter = lock.withLock { awaitingResponse.take() }
        if let http = response as? HTTPURLResponse {
            waiter?.resume(returning: http)
        } else {
            waiter?.resume(throwing: WaveChatError.badStatus(-1))
        }
        return .allow
    }

    func urlSession(_ session: URLSession, dataTask: URLSessionDataTask, didReceive data: Data) {
        lock.lock()
        let sink = chunks
        lock.unlock()
        sink?.yield(data)
    }

    func urlSession(_ session: URLSession, task: URLSessionTask, didCompleteWithError error: Error?) {
        lock.lock()
        let waiter = awaitingResponse.take()
        let sink = chunks
        lock.unlock()
        // A failure before the headers has nobody else to surface it.
        waiter?.resume(throwing: error ?? WaveChatError.badStatus(-1))
        if let error {
            sink?.finish(throwing: error)
        } else {
            sink?.finish()
        }
    }
}

extension Optional {
    /// Read and clear in one step, so a continuation can only be resumed once.
    fileprivate mutating func take() -> Wrapped? {
        defer { self = nil }
        return self
    }
}

/// The wave loop's live state, streamed as `state` SSE events (the event data
/// is the bare state name). The composer keys its verb off it.
public enum WaveLoopState: String, Equatable, Sendable {
    case idle
    case turning
    case interrupting
    case failed
}

/// How a posted message asks to be handled — the required `op` of the
/// `POST /messages {op, text}` body. Explicit at the API, never inferred.
public enum WaveMessageOp: String, Equatable, Sendable {
    /// Queued; the loop's next turn answers it.
    case message
    /// Into the live turn (server degrades to a queued message when the
    /// harness can't steer or nothing is turning).
    case steer
    /// Cancel the open turn. Empty text = bare interrupt (no-op while idle);
    /// non-empty text becomes the next turn ("interrupt & send").
    case interrupt
}

/// What the composer's buttons should do for a loop state + text presence.
public enum ComposerVerb: Equatable, Sendable {
    case send            // POST op=message
    case steer           // POST op=steer
    case interrupt       // POST op=interrupt, empty text
    case interruptAndSend // POST op=interrupt carrying the text
}

/// The composer's action set: one primary button, an optional secondary
/// ("Interrupt & Send" while a steer is primary). `primaryEnabled` assumes
/// the connection is live; the view also gates on liveness.
public struct ComposerVerbs: Equatable, Sendable {
    public let primary: ComposerVerb
    public let primaryEnabled: Bool
    public let secondary: ComposerVerb?
}

/// Verb selection: idle+text = Send; turning+text = Steer (Interrupt & Send
/// one keypress away); turning+empty = Interrupt. While interrupting, text
/// degrades to a queued Send and a bare re-interrupt is pointless (disabled).
public func composerVerbs(state: WaveLoopState, hasText: Bool) -> ComposerVerbs {
    switch (state, hasText) {
    case (.turning, true):
        return ComposerVerbs(primary: .steer, primaryEnabled: true, secondary: .interruptAndSend)
    case (.turning, false):
        return ComposerVerbs(primary: .interrupt, primaryEnabled: true, secondary: nil)
    case (.interrupting, false):
        return ComposerVerbs(primary: .interrupt, primaryEnabled: false, secondary: nil)
    default:
        return ComposerVerbs(primary: .send, primaryEnabled: hasText, secondary: nil)
    }
}

/// `POST /messages {op, text}` response: the appended user turn (null for a
/// bare interrupt) plus the loop-state name at acceptance. Mirrors Rust
/// `PostMessageResponse` (wave/server.rs); pinned by the
/// `post_message_response.json` fixture in ContractTests.
struct PostMessageResponse: Decodable {
    let turn: ChatTurn?
    let state: String
}

/// Observable connection to one wave's chat server: the live thread plus a phase
/// the UI renders (not running / connecting / live).
@MainActor
@Observable
public final class WaveChatConnection {
    public enum Phase: Equatable, Sendable {
        case idle
        case notRunning
        case connecting
        case live
    }

    public let repoPath: String
    public let waveName: String

    public private(set) var turns: [ChatTurn] = []
    public private(set) var phase: Phase = .idle
    /// Nil while the bounded local read has not completed.
    public private(set) var historyState: ChatHistoryState?
    public private(set) var historyDetail: String?
    public private(set) var historyTruncated = false
    /// Last loop state seen — sent once on subscribe, again on every
    /// transition, and echoed by `POST /messages` responses.
    public private(set) var loopState: WaveLoopState = .idle
    /// Durable invocation stack, current step, local queue, and return point.
    public private(set) var playhead: PlayheadView?
    private var currentEndpoint: String?
    private var loop: Task<Void, Never>?
    private let session: URLSession
    private let loadHistory: ChatHistoryLoader?
    private let historyLimit: Int
    private let decoder = JSONDecoder()

    /// Poll interval while the wave isn't running yet, in nanoseconds.
    private let pollInterval: UInt64 = 1_000_000_000
    public init(
        repoPath: String,
        waveName: String,
        session: URLSession? = nil,
        historyLimit: Int = 12,
        loadHistory: ChatHistoryLoader? = nil
    ) {
        self.repoPath = repoPath
        self.waveName = waveName
        self.historyLimit = historyLimit
        self.loadHistory = loadHistory
        if let session {
            self.session = session
        } else {
            let config = URLSessionConfiguration.ephemeral
            config.timeoutIntervalForRequest = 3600
            config.timeoutIntervalForResource = 86400
            config.waitsForConnectivity = false
            self.session = URLSession(configuration: config)
        }
    }

    public func start() {
        guard loop == nil else { return }
        loop = Task { await run() }
    }

    public func stop() {
        loop?.cancel()
        loop = nil
    }

    /// POST a message with an explicit op; a created user turn is applied
    /// immediately and also arrives over the stream (deduped by id). The
    /// assistant reply streams later. Text may be empty only for `.interrupt`
    /// (a bare interrupt); empty message/steer sends are dropped client-side.
    public func send(_ text: String, op: WaveMessageOp = .message) async throws {
        let trimmed = text.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty || op == .interrupt else { return }
        guard let endpoint = currentEndpoint, let url = URL(string: "http://\(endpoint)/messages") else {
            throw WaveChatError.notRunning
        }
        var request = URLRequest(url: url)
        request.httpMethod = "POST"
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        request.httpBody = try JSONSerialization.data(withJSONObject: ["op": op.rawValue, "text": trimmed])
        let (data, response) = try await session.data(for: request)
        guard let http = response as? HTTPURLResponse, http.statusCode == 200 else {
            throw WaveChatError.badStatus((response as? HTTPURLResponse)?.statusCode ?? -1)
        }
        let posted = try decoder.decode(PostMessageResponse.self, from: data)
        if let turn = posted.turn {
            upsert(turn)
        }
        if let state = WaveLoopState(rawValue: posted.state) {
            loopState = state
        }
    }

    // MARK: - Discovery + streaming loop

    private func run() async {
        await loadDurableHistory()
        while !Task.isCancelled {
            guard let endpoint = WaveEndpoint.read(repoPath: repoPath, waveName: waveName) else {
                currentEndpoint = nil
                phase = .notRunning
                try? await Task.sleep(nanoseconds: pollInterval)
                continue
            }
            currentEndpoint = endpoint
            phase = .connecting
            do {
                try await stream(endpoint: endpoint)
            } catch is CancellationError {
                return
            } catch {
                // Connection refused or dropped — fall through and re-evaluate the
                // pointer file. A vanished file means the wave shut down.
            }
            if Task.isCancelled { return }
            phase = .notRunning
            try? await Task.sleep(nanoseconds: pollInterval)
        }
    }

    private func loadDurableHistory() async {
        guard let loadHistory else { return }
        do {
            let snapshot = try await loadHistory(repoPath, waveName, historyLimit)
            turns = snapshot.turns.sorted { a, b in
                if a.sequence != b.sequence { return a.sequence < b.sequence }
                return a.id < b.id
            }
            historyState = snapshot.state
            historyDetail = snapshot.detail
            historyTruncated = snapshot.truncated
        } catch is CancellationError {
            return
        } catch {
            historyState = .unavailable
            historyDetail = "Saved conversation could not be read: \(error.localizedDescription)"
            historyTruncated = false
        }
    }

    private func stream(endpoint: String) async throws {
        guard let url = URL(string: "http://\(endpoint)/events") else {
            throw WaveChatError.badEndpoint(endpoint)
        }
        var request = URLRequest(url: url)
        request.setValue("text/event-stream", forHTTPHeaderField: "Accept")
        // Chunks, not `session.bytes(for:)`: one suspension per byte cannot keep
        // up with whole-turn frames (see SSEChunkStream).
        let transport = SSEChunkStream()
        let body = transport.connect(request, on: session)
        let http = try await transport.response()
        guard http.statusCode == 200 else {
            throw WaveChatError.badStatus(http.statusCode)
        }
        // A fresh stream replays the same journal tail as the bounded local
        // read. Keep the saved snapshot painted and upsert replay frames by id;
        // connecting cannot repair a partial or unavailable durable read.
        loopState = .idle
        playhead = nil
        if historyState == nil {
            historyState = .available
        }
        phase = .live

        var parser = SSEFrameParser()
        for try await chunk in body {
            for frame in parser.consume(chunk) {
                if Task.isCancelled { return }
                // `resync`: the live turn stream lagged, so a `turn-delta` may
                // have been dropped and the open-turn reconstruction could be
                // short a fragment. End this connection; `run()` reconnects for
                // a fresh whole-turn replay into the stable turn ids.
                if frame.event == "resync" { return }
                handle(event: frame.event, data: frame.data)
            }
        }
    }

    /// One SSE frame off `/events`. `state` carries the bare loop-state name
    /// (sent on subscribe and on every transition); `turn` carries a whole turn
    /// — sent when a turn opens and finalizes, replacing its id's state;
    /// `turn-delta` carries one in-turn increment absorbed into the matching
    /// turn (so a per-token turn does not re-send whole each frame); `resync`
    /// (handled in `stream`, not here) means the turn stream lagged and the
    /// connection reconnects. Unknown events drop. A turn payload that fails
    /// to decode is a hole in the transcript: logged always, asserted in debug
    /// — never silent. Internal for tests.
    func handle(event: String, data: String) {
        if event == "state" {
            guard let state = WaveLoopState(rawValue: data) else { return }
            loopState = state
            return
        }
        if event == "playhead" {
            guard let json = data.data(using: .utf8),
                  let snapshot = try? decoder.decode(PlayheadView.self, from: json) else { return }
            playhead = snapshot
            return
        }
        if event == "turn-delta" {
            guard let json = data.data(using: .utf8) else { return }
            do {
                applyDelta(try decoder.decode(TurnDelta.self, from: json))
            } catch {
                LoggingService.wave("wave chat: dropped turn-delta frame (\(error)): \(data.prefix(200))")
                assertionFailure("wave chat turn-delta frame failed to decode: \(error)")
            }
            return
        }
        guard event.isEmpty || event == "turn", let json = data.data(using: .utf8) else { return }
        do {
            upsert(try decoder.decode(ChatTurn.self, from: json))
            if historyState == .missing {
                historyState = .available
                historyDetail = nil
            }
        } catch {
            LoggingService.wave("wave chat: dropped turn frame (\(error)): \(data.prefix(200))")
            assertionFailure("wave chat turn frame failed to decode: \(error)")
        }
    }

    /// Grow the open turn named by a `turn-delta` frame through the same
    /// `absorb` rule the listener folds with. No turn for this id means we
    /// missed its opening (a gap the server heals with `resync`) — drop the
    /// delta until the whole-turn replay rebuilds it. Internal for tests.
    func applyDelta(_ delta: TurnDelta) {
        guard let index = turns.firstIndex(where: { $0.id == delta.turnId }) else { return }
        do {
            turns[index] = try turns[index].absorbing(delta.item)
        } catch {
            LoggingService.wave("wave chat: turn-delta absorb failed (\(error))")
            assertionFailure("wave chat turn-delta absorb failed: \(error)")
        }
    }

    /// Replace a turn already in the thread (an in-progress turn re-sent as it
    /// grows, then finalized under the same id), or append a new one. Ordered
    /// by monotonic sequence, id as tie-break: `sort` isn't guaranteed stable
    /// and unparseable ids share a `.max` sentinel sequence, so the tie-break
    /// keeps the order deterministic. A replace skips the sort — the key is
    /// (sequence, id) and both derive from the id, so an in-place frame can't
    /// move; no reason to re-sort the thread on every SSE growth frame.
    /// Internal for tests.
    func upsert(_ turn: ChatTurn) {
        if let index = turns.firstIndex(where: { $0.id == turn.id }) {
            turns[index] = turn
            return
        }
        turns.append(turn)
        turns.sort { a, b in
            if a.sequence != b.sequence { return a.sequence < b.sequence }
            return a.id < b.id
        }
    }
}
