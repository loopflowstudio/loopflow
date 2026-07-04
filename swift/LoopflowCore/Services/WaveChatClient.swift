import Foundation

// Live client for a wave's chat server. A running `lf wave <name>` publishes its
// loopback address to `wave/<name>/.wave-endpoint`; this client discovers it,
// replays + streams the conversation over SSE, and posts messages back. When the
// pointer file is absent or the server refuses the connection, the connection
// settles into `.notRunning` and keeps polling so it attaches the moment the
// wave comes up.

public enum WaveChatError: Error, Sendable {
    case notRunning
    case badStatus(Int)
    case badEndpoint(String)
}

/// Reads the discovery pointer a running wave writes under its `wave/<name>/` dir.
public enum WaveEndpoint {
    public static let fileName = ".wave-endpoint"

    public static func path(repoPath: String, waveName: String) -> URL {
        URL(fileURLWithPath: repoPath)
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

/// The wave mind's live state, streamed as `state` SSE events (the event data
/// is the bare state name). The composer keys its verb off it.
public enum WaveMindState: String, Equatable, Sendable {
    case idle
    case turning
    case interrupting
    case failed
}

/// How a posted message asks to be handled — the required `op` of the
/// `POST /messages {op, text}` body. Explicit at the API, never inferred.
public enum WaveMessageOp: String, Equatable, Sendable {
    /// Queued; the mind's next turn answers it.
    case message
    /// Into the live turn (server degrades to a queued message when the
    /// harness can't steer or nothing is turning).
    case steer
    /// Cancel the open turn. Empty text = bare interrupt (no-op while idle);
    /// non-empty text becomes the next turn ("interrupt & send").
    case interrupt
}

/// What the composer's buttons should do for a mind state + text presence.
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
/// one step away); turning+empty = Interrupt. While interrupting, text
/// degrades to a queued Send and a bare re-interrupt is pointless (disabled).
public func composerVerbs(state: WaveMindState, hasText: Bool) -> ComposerVerbs {
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
    /// Last mind state seen — sent once on subscribe, again on every
    /// transition, and echoed by `POST /messages` responses.
    public private(set) var mindState: WaveMindState = .idle

    private var currentEndpoint: String?
    private var loop: Task<Void, Never>?
    private let session: URLSession
    private let decoder = JSONDecoder()

    /// Poll interval while the wave isn't running yet, in nanoseconds.
    private let pollInterval: UInt64 = 1_000_000_000

    public init(repoPath: String, waveName: String, session: URLSession? = nil) {
        self.repoPath = repoPath
        self.waveName = waveName
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

    /// `POST /messages {op, text}` response: the appended user turn (null for
    /// a bare interrupt) plus the mind-state name at acceptance.
    private struct PostMessageResponse: Decodable {
        let turn: ChatTurn?
        let state: String
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
        if let state = WaveMindState(rawValue: posted.state) {
            mindState = state
        }
    }

    // MARK: - Discovery + streaming loop

    private func run() async {
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

    private func stream(endpoint: String) async throws {
        guard let url = URL(string: "http://\(endpoint)/conversation/stream") else {
            throw WaveChatError.badEndpoint(endpoint)
        }
        var request = URLRequest(url: url)
        request.setValue("text/event-stream", forHTTPHeaderField: "Accept")
        let (bytes, response) = try await session.bytes(for: request)
        guard let http = response as? HTTPURLResponse, http.statusCode == 200 else {
            throw WaveChatError.badStatus((response as? HTTPURLResponse)?.statusCode ?? -1)
        }
        // A fresh stream replays the server's full transcript. Drop the previous
        // generation first: after a server restart, turn ids and sequences start
        // over, and stale high-sequence turns would interleave with the replay.
        // The mind state resets too; the server's first frame is a `state` event.
        turns = []
        mindState = .idle
        phase = .live

        var event = ""
        var payload = ""
        for try await line in bytes.lines {
            if Task.isCancelled { return }
            if line.isEmpty {
                if !payload.isEmpty { handle(event: event, data: payload) }
                event = ""
                payload = ""
            } else if line.hasPrefix(":") {
                continue // SSE comment / keep-alive ping
            } else if line.hasPrefix("event:") {
                event = String(line.dropFirst("event:".count)).trimmingCharacters(in: .whitespaces)
            } else if line.hasPrefix("data:") {
                let chunk = String(line.dropFirst("data:".count)).trimmingCharacters(in: .whitespaces)
                payload += payload.isEmpty ? chunk : "\n" + chunk
            }
        }
    }

    /// One SSE frame. Two event names: `state` carries the bare mind-state
    /// name (sent on subscribe and on every transition); `turn` carries a
    /// whole turn — re-sent under the same id as it grows, then a terminal
    /// frame at finalization, every frame replacing the previous state of its
    /// id. Unknown events and unparseable payloads drop. Internal for tests.
    func handle(event: String, data: String) {
        if event == "state" {
            guard let state = WaveMindState(rawValue: data) else { return }
            mindState = state
            return
        }
        guard event.isEmpty || event == "turn", let json = data.data(using: .utf8) else { return }
        guard let turn = try? decoder.decode(ChatTurn.self, from: json) else { return }
        upsert(turn)
    }

    /// Replace a turn already in the thread (an in-progress turn re-sent as it
    /// grows, then finalized under the same id), or append a new one. Ordered
    /// by monotonic sequence, id as tie-break: `sort` isn't guaranteed stable
    /// and unparseable ids share a `.max` sentinel sequence, so the tie-break
    /// keeps the order deterministic. Internal for tests.
    func upsert(_ turn: ChatTurn) {
        if let index = turns.firstIndex(where: { $0.id == turn.id }) {
            turns[index] = turn
        } else {
            turns.append(turn)
        }
        turns.sort { a, b in
            if a.sequence != b.sequence { return a.sequence < b.sequence }
            return a.id < b.id
        }
    }
}
