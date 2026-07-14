import Foundation
import Network
import Testing

@testable import Loopflow

/// A loopback HTTP server that writes a canned SSE body and closes.
///
/// It has to be a real socket. `URLSession.AsyncBytes`'s per-byte cost is a
/// property of the network stack — fed from a `URLProtocol` stub it runs at
/// ~20 MB/s, on a real connection at ~0.14 MB/s. A stubbed transport would let
/// the regression this test exists to catch pass.
final class LoopbackSSEServer: @unchecked Sendable {
    private let listener: NWListener
    let port: UInt16

    init(body: Data) throws {
        let params = NWParameters.tcp
        params.allowLocalEndpointReuse = true
        listener = try NWListener(using: params)

        let response = Data(
            """
            HTTP/1.1 200 OK\r
            Content-Type: text/event-stream\r
            Content-Length: \(body.count)\r
            Connection: close\r
            \r

            """.utf8) + body

        listener.newConnectionHandler = { connection in
            connection.start(queue: .global())
            // Read the request line, then write the whole body.
            connection.receive(minimumIncompleteLength: 1, maximumLength: 8192) { _, _, _, _ in
                connection.send(
                    content: response,
                    completion: .contentProcessed { _ in connection.cancel() })
            }
        }

        let ready = DispatchSemaphore(value: 0)
        listener.stateUpdateHandler = { if case .ready = $0 { ready.signal() } }
        listener.start(queue: .global())
        guard ready.wait(timeout: .now() + 5) == .success, let bound = listener.port else {
            throw WaveChatError.badStatus(-1)
        }
        port = bound.rawValue
    }

    func stop() { listener.cancel() }
}

/// `@MainActor`, because `WaveChatConnection` is: the read loop's cost is one
/// actor hop per `await`, so a per-byte loop pays a main-actor hop per byte.
/// Off the main actor the same loop is ~140x faster and this budget would not
/// bite — the isolation is part of what is under test, not a detail.
@Suite(.serialized)
@MainActor
struct WaveChatStreamTests {
    /// The Wave Chat read budget, in one test.
    ///
    /// The listener re-sends the whole open turn on every token, so a reader
    /// meets ~100 KB frames and a multi-megabyte replay on connect. Reading that
    /// stream one byte per async suspension (`URLSession.AsyncBytes`) sustains
    /// ~0.14 MB/s on a real connection: the 3 MB replay below took ~23 s, and a
    /// live turn arrived at about a word per second. Chunked reads do the same
    /// work at >200 MB/s.
    ///
    /// The deadline has two orders of magnitude of headroom over the chunked
    /// path and is still far under what a per-byte reader needs, so it fails on
    /// the regression, not on a slow machine. Verified against the byte-at-a-time
    /// transport: it takes ~23 s here and trips this budget.
    @Test("a multi-megabyte SSE replay is read well inside the budget")
    func replayIsReadWithinBudget() async throws {
        // Turn frames the size the accumulated product wave produces: prose plus
        // the tool output that rides in `items`.
        let filler = String(repeating: "a", count: 100 * 1024)
        let turn = """
            {"id":"turn-1","role":"assistant","status":"completed","created_at":"2026-07-14T00:00:00Z",\
            "text":"\(filler)","items":[]}
            """
        var body = ""
        for _ in 0..<30 {
            body += "event: turn\ndata: \(turn)\n\n"
        }
        #expect(body.utf8.count > 3_000_000)

        let server = try LoopbackSSEServer(body: Data(body.utf8))
        defer { server.stop() }

        let config = URLSessionConfiguration.ephemeral
        config.timeoutIntervalForRequest = 60
        let session = URLSession(configuration: config)

        let transport = SSEChunkStream()
        let url = URL(string: "http://127.0.0.1:\(server.port)/events")!
        let stream = transport.connect(URLRequest(url: url), on: session)
        let response = try await transport.response()
        #expect(response.statusCode == 200)

        let start = Date()
        var parser = SSEFrameParser()
        var frames = 0
        for try await chunk in stream {
            frames += parser.consume(chunk).count
        }
        let elapsed = Date().timeIntervalSince(start)

        // Every frame arrived — a chunk boundary landing mid-frame never
        // swallowed one.
        #expect(frames == 30)
        #expect(elapsed < 5.0, "SSE replay took \(elapsed)s; the reader is back on the slow path")
    }
}
