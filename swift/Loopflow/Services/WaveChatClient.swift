import Foundation

// Live client for a wave's chat server. A running `lf wave <name>` publishes its
// loopback address to `wave/<name>/.wave-endpoint`; this client discovers it,
// consumes the unified `GET /events` SSE stream (epoch, backing-health,
// message, message-delta, state, and playhead frames), and posts messages back.
// When the pointer file is absent or the server refuses the connection, the
// connection settles into `.notRunning` and keeps polling so it attaches when
// the wave starts.

public enum WaveChatError: Error, Sendable {
    case notRunning
    case badStatus(Int)
    case badEndpoint(String)
    case openDiscord(ChatAction)
}

public enum ChatAction: Codable, Sendable, Equatable {
    case openDiscord(label: String, url: String)

    private enum CodingKeys: String, CodingKey { case kind, label, url }

    public init(from decoder: Decoder) throws {
        let values = try decoder.container(keyedBy: CodingKeys.self)
        switch try values.decode(String.self, forKey: .kind) {
        case "open_discord":
            self = .openDiscord(
                label: try values.decode(String.self, forKey: .label),
                url: try values.decode(String.self, forKey: .url)
            )
        default:
            throw DecodingError.dataCorruptedError(
                forKey: .kind,
                in: values,
                debugDescription: "Unknown Wave Chat action"
            )
        }
    }

    public func encode(to encoder: Encoder) throws {
        var values = encoder.container(keyedBy: CodingKeys.self)
        switch self {
        case let .openDiscord(label, url):
            try values.encode("open_discord", forKey: .kind)
            try values.encode(label, forKey: .label)
            try values.encode(url, forKey: .url)
        }
    }
}

public enum ChatBacking: Codable, Sendable, Equatable {
    case local
    case discord(guildId: String, channelId: String, open: ChatAction)

    private enum CodingKeys: String, CodingKey {
        case kind
        case guildId = "guild_id"
        case channelId = "channel_id"
        case open
    }

    public init(from decoder: Decoder) throws {
        let values = try decoder.container(keyedBy: CodingKeys.self)
        switch try values.decode(String.self, forKey: .kind) {
        case "local": self = .local
        case "discord":
            self = .discord(
                guildId: try values.decode(String.self, forKey: .guildId),
                channelId: try values.decode(String.self, forKey: .channelId),
                open: try values.decode(ChatAction.self, forKey: .open)
            )
        default:
            throw DecodingError.dataCorruptedError(
                forKey: .kind,
                in: values,
                debugDescription: "Unknown Wave Chat backing"
            )
        }
    }

    public func encode(to encoder: Encoder) throws {
        var values = encoder.container(keyedBy: CodingKeys.self)
        switch self {
        case .local:
            try values.encode("local", forKey: .kind)
        case let .discord(guildId, channelId, open):
            try values.encode("discord", forKey: .kind)
            try values.encode(guildId, forKey: .guildId)
            try values.encode(channelId, forKey: .channelId)
            try values.encode(open, forKey: .open)
        }
    }
}

public struct ConversationEpoch: Codable, Sendable, Equatable {
    public let id: String
    public let number: UInt64
    public let backing: ChatBacking
    public let journalSeq: UInt64
    public let startedAt: String
    public let endedAt: String?

    private enum CodingKeys: String, CodingKey {
        case id, number, backing
        case journalSeq = "journal_seq"
        case startedAt = "started_at"
        case endedAt = "ended_at"
    }
}

public enum ChatMessageSource: Codable, Sendable, Equatable {
    case local(journalSeq: UInt64)
    case discord(
        guildId: String,
        channelId: String,
        messageId: String,
        authorId: String,
        url: String
    )

    private enum CodingKeys: String, CodingKey {
        case kind
        case journalSeq = "journal_seq"
        case guildId = "guild_id"
        case channelId = "channel_id"
        case messageId = "message_id"
        case authorId = "author_id"
        case url
    }

    public init(from decoder: Decoder) throws {
        let values = try decoder.container(keyedBy: CodingKeys.self)
        switch try values.decode(String.self, forKey: .kind) {
        case "local":
            self = .local(journalSeq: try values.decode(UInt64.self, forKey: .journalSeq))
        case "discord":
            self = .discord(
                guildId: try values.decode(String.self, forKey: .guildId),
                channelId: try values.decode(String.self, forKey: .channelId),
                messageId: try values.decode(String.self, forKey: .messageId),
                authorId: try values.decode(String.self, forKey: .authorId),
                url: try values.decode(String.self, forKey: .url)
            )
        default:
            throw DecodingError.dataCorruptedError(
                forKey: .kind,
                in: values,
                debugDescription: "Unknown Wave Chat message source"
            )
        }
    }

    public func encode(to encoder: Encoder) throws {
        var values = encoder.container(keyedBy: CodingKeys.self)
        switch self {
        case let .local(journalSeq):
            try values.encode("local", forKey: .kind)
            try values.encode(journalSeq, forKey: .journalSeq)
        case let .discord(guildId, channelId, messageId, authorId, url):
            try values.encode("discord", forKey: .kind)
            try values.encode(guildId, forKey: .guildId)
            try values.encode(channelId, forKey: .channelId)
            try values.encode(messageId, forKey: .messageId)
            try values.encode(authorId, forKey: .authorId)
            try values.encode(url, forKey: .url)
        }
    }
}

public struct WaveChatMessage: Codable, Sendable, Equatable {
    public let epochId: String
    public let source: ChatMessageSource
    public let turn: ChatTurn

    private enum CodingKeys: String, CodingKey {
        case epochId = "epoch_id"
        case source, turn
    }
}

/// Evidence state of a read-only fold over the durable Wave journal.
public enum ChatHistoryState: String, Codable, Sendable, Equatable {
    case available
    case missing
    case partial
    case unavailable
}

public enum ChatBackingHealth: Codable, Sendable, Equatable {
    case ready
    case retrying(detail: String)
    case blocked(detail: String)

    private enum CodingKeys: String, CodingKey { case state, detail }

    public init(from decoder: Decoder) throws {
        let values = try decoder.container(keyedBy: CodingKeys.self)
        switch try values.decode(String.self, forKey: .state) {
        case "ready": self = .ready
        case "retrying":
            self = .retrying(detail: try values.decode(String.self, forKey: .detail))
        case "blocked":
            self = .blocked(detail: try values.decode(String.self, forKey: .detail))
        default:
            throw DecodingError.dataCorruptedError(
                forKey: .state,
                in: values,
                debugDescription: "Unknown Wave Chat backing health"
            )
        }
    }

    public func encode(to encoder: Encoder) throws {
        var values = encoder.container(keyedBy: CodingKeys.self)
        switch self {
        case .ready:
            try values.encode("ready", forKey: .state)
        case let .retrying(detail):
            try values.encode("retrying", forKey: .state)
            try values.encode(detail, forKey: .detail)
        case let .blocked(detail):
            try values.encode("blocked", forKey: .state)
            try values.encode(detail, forKey: .detail)
        }
    }
}

/// Bounded `lf chat --history --json` response. Every field mirrors Rust.
public struct ChatHistorySnapshot: Codable, Sendable, Equatable {
    public let epochs: [ConversationEpoch]
    public let selectedEpochId: String?
    public let state: ChatHistoryState
    public let detail: String?
    public let messages: [WaveChatMessage]
    public let truncated: Bool

    public init(
        epochs: [ConversationEpoch],
        selectedEpochId: String?,
        state: ChatHistoryState,
        detail: String?,
        messages: [WaveChatMessage],
        truncated: Bool
    ) {
        self.epochs = epochs
        self.selectedEpochId = selectedEpochId
        self.state = state
        self.detail = detail
        self.messages = messages
        self.truncated = truncated
    }

    private enum CodingKeys: String, CodingKey {
        case epochs
        case selectedEpochId = "selected_epoch_id"
        case state, detail, messages, truncated
    }
}

public typealias ChatHistoryLoader = @Sendable (
    _ repoPath: String,
    _ waveName: String,
    _ limit: Int,
    _ epoch: String?
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

/// `POST /messages {op, text}` response: the source-bearing committed message
/// (null for a bare interrupt), active epoch, and loop state. Mirrors Rust
/// `PostMessageResponse` (wave/server.rs); pinned by the
/// `post_message_response.json` fixture in ContractTests.
struct PostMessageResponse: Decodable {
    let message: WaveChatMessage?
    let state: String
    let epoch: ConversationEpoch
}

struct PostMessageErrorResponse: Decodable {
    let error: String
    let epoch: ConversationEpoch
}

/// Observable connection to one wave's active conversation and backing state.
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

    public private(set) var messages: [WaveChatMessage] = []
    public var turns: [ChatTurn] { messages.map(\.turn) }
    public private(set) var activeEpoch: ConversationEpoch?
    public private(set) var epochs: [ConversationEpoch] = []
    public private(set) var selectedEpoch: ConversationEpoch?
    public private(set) var backingHealth: ChatBackingHealth = .ready
    public private(set) var phase: Phase = .idle
    /// Nil while the initial bounded history read has not completed.
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
        guard let http = response as? HTTPURLResponse else {
            throw WaveChatError.badStatus(-1)
        }
        guard http.statusCode == 200 else {
            if http.statusCode == 409,
               let rejection = try? decoder.decode(PostMessageErrorResponse.self, from: data),
               case let .discord(_, _, action) = rejection.epoch.backing {
                applyActiveEpoch(rejection.epoch)
                throw WaveChatError.openDiscord(action)
            }
            throw WaveChatError.badStatus(http.statusCode)
        }
        let posted = try decoder.decode(PostMessageResponse.self, from: data)
        applyActiveEpoch(posted.epoch)
        if let message = posted.message {
            upsert(message)
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
            applyHistory(try await loadHistory(repoPath, waveName, historyLimit, nil))
        } catch is CancellationError {
            return
        } catch {
            historyState = .unavailable
            historyDetail = "Saved conversation could not be read: \(error.localizedDescription)"
            historyTruncated = false
        }
    }

    public func selectEpoch(_ id: String) async {
        guard let loadHistory, epochs.contains(where: { $0.id == id }) else { return }
        do {
            applyHistory(try await loadHistory(repoPath, waveName, historyLimit, id))
        } catch is CancellationError {
            return
        } catch {
            historyState = .unavailable
            historyDetail = "Conversation epoch could not be read: \(error.localizedDescription)"
        }
    }

    private func applyHistory(_ snapshot: ChatHistorySnapshot) {
        epochs = snapshot.epochs
        activeEpoch = snapshot.epochs.last
        selectedEpoch = snapshot.selectedEpochId.flatMap { id in
            snapshot.epochs.first { $0.id == id }
        }
        messages = snapshot.messages.sorted(by: messageOrder)
        historyState = snapshot.state
        historyDetail = snapshot.detail
        historyTruncated = snapshot.truncated
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
        // A fresh stream replays the selected backing's bounded history. Keep
        // the saved snapshot painted and upsert replay frames by id; connecting
        // cannot repair a partial or unavailable read.
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
                // `resync`: the live message stream lagged, so a
                // `message-delta` may have been dropped. End this connection;
                // `run()` reconnects for a fresh provider or journal snapshot.
                if frame.event == "resync" { return }
                handle(event: frame.event, data: frame.data)
            }
        }
    }

    /// One SSE frame off `/events`. `state` carries the bare loop-state name
    /// (sent on subscribe and on every transition); `message` carries a whole
    /// source-bearing message, while `message-delta` grows one local message;
    /// `resync`
    /// (handled in `stream`, not here) means the message stream lagged and the
    /// connection reconnects. Unknown events drop. A message payload that fails
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
        if event == "epoch" {
            guard let json = data.data(using: .utf8),
                  let epoch = try? decoder.decode(ConversationEpoch.self, from: json) else { return }
            applyActiveEpoch(epoch)
            return
        }
        if event == "backing-health" {
            guard let json = data.data(using: .utf8),
                  let health = try? decoder.decode(ChatBackingHealth.self, from: json) else { return }
            backingHealth = health
            return
        }
        if event == "message-delta" {
            guard let json = data.data(using: .utf8) else { return }
            do {
                applyDelta(try decoder.decode(TurnDelta.self, from: json))
            } catch {
                LoggingService.wave("wave chat: dropped message-delta frame (\(error)): \(data.prefix(200))")
                assertionFailure("wave chat message-delta frame failed to decode: \(error)")
            }
            return
        }
        if event == "message" {
            guard let json = data.data(using: .utf8) else { return }
            do {
                let message = try decoder.decode(WaveChatMessage.self, from: json)
                if selectedEpoch?.id == message.epochId {
                    upsert(message)
                }
                if historyState == .missing {
                    historyState = .available
                    historyDetail = nil
                }
            } catch {
                LoggingService.wave("wave chat: dropped message frame (\(error)): \(data.prefix(200))")
                assertionFailure("wave chat message frame failed to decode: \(error)")
            }
            return
        }
    }

    private func applyActiveEpoch(_ epoch: ConversationEpoch) {
        let followsActive = selectedEpoch == nil || selectedEpoch?.id == activeEpoch?.id
        activeEpoch = epoch
        if let index = epochs.firstIndex(where: { $0.id == epoch.id }) {
            epochs[index] = epoch
        } else {
            epochs.append(epoch)
            epochs.sort { $0.number < $1.number }
        }
        if followsActive {
            selectedEpoch = epoch
            messages.removeAll { $0.epochId != epoch.id }
        }
    }

    /// Grow the open source-bearing message named by a `message-delta` frame
    /// through the same `absorb` rule the listener folds with. No message for
    /// this id means we missed its opening (a gap the server heals with
    /// `resync`) — drop the delta until message replay rebuilds it. Internal
    /// for tests.
    func applyDelta(_ delta: TurnDelta) {
        guard let index = messages.firstIndex(where: { $0.turn.id == delta.turnId }) else { return }
        do {
            let message = messages[index]
            messages[index] = WaveChatMessage(
                epochId: message.epochId,
                source: message.source,
                turn: try message.turn.absorbing(delta.item)
            )
        } catch {
            LoggingService.wave("wave chat: message-delta absorb failed (\(error))")
            assertionFailure("wave chat message-delta absorb failed: \(error)")
        }
    }

    /// Replace a message already in the selected epoch (an in-progress local
    /// turn re-sent as it
    /// grows, then finalized under the same id), or append a new one. Ordered
    /// by monotonic sequence, id as tie-break: `sort` isn't guaranteed stable
    /// and unparseable ids share a `.max` sentinel sequence, so the tie-break
    /// keeps the order deterministic. A replace skips the sort — the key is
    /// (sequence, id) and both derive from the id, so an in-place frame can't
    /// move; no reason to re-sort the epoch on every SSE growth frame.
    /// Internal for tests.
    func upsert(_ message: WaveChatMessage) {
        if let index = messages.firstIndex(where: { $0.turn.id == message.turn.id }) {
            messages[index] = message
            return
        }
        messages.append(message)
        messages.sort(by: messageOrder)
    }

}

private func messageOrder(_ lhs: WaveChatMessage, _ rhs: WaveChatMessage) -> Bool {
    if let left = lhs.turn.createdAtDate, let right = rhs.turn.createdAtDate, left != right {
        return left < right
    }
    if lhs.turn.sequence != rhs.turn.sequence { return lhs.turn.sequence < rhs.turn.sequence }
    return lhs.turn.id < rhs.turn.id
}
