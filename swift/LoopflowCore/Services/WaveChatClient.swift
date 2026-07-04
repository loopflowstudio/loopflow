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

    /// POST a message; the created user turn is applied immediately and also
    /// arrives over the stream (deduped by id). The assistant reply streams later.
    public func send(_ text: String) async throws {
        let trimmed = text.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return }
        guard let endpoint = currentEndpoint, let url = URL(string: "http://\(endpoint)/messages") else {
            throw WaveChatError.notRunning
        }
        var request = URLRequest(url: url)
        request.httpMethod = "POST"
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        request.httpBody = try JSONSerialization.data(withJSONObject: ["text": trimmed])
        let (data, response) = try await session.data(for: request)
        guard let http = response as? HTTPURLResponse, http.statusCode == 200 else {
            throw WaveChatError.badStatus((response as? HTTPURLResponse)?.statusCode ?? -1)
        }
        let turn = try decoder.decode(ChatTurn.self, from: data)
        upsert(turn)
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
        turns = []
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

    private func handle(event: String, data: String) {
        guard event.isEmpty || event == "turn", let json = data.data(using: .utf8) else { return }
        guard let turn = try? decoder.decode(ChatTurn.self, from: json) else { return }
        upsert(turn)
    }

    /// Replace a turn already in the thread (an in-progress turn re-sent as its
    /// text grows), or append a new one. Ordered by monotonic sequence so replay
    /// and live turns interleave cleanly.
    private func upsert(_ turn: ChatTurn) {
        if let index = turns.firstIndex(where: { $0.id == turn.id }) {
            turns[index] = turn
        } else {
            turns.append(turn)
        }
        turns.sort { $0.sequence < $1.sequence }
    }
}
