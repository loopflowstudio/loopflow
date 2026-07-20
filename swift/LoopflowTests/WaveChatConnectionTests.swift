// Streaming contract for WaveChatConnection: the server re-sends an
// in-progress turn whole under the same id as it grows, then a terminal frame
// at finalization. Repeated same-id frames must update in place, flip status
// running → completed/failed/interrupted, and keep the thread order
// deterministic.

import Foundation
import Testing
@testable import Loopflow

@MainActor
@Suite("WaveChat streaming upsert")
struct WaveChatConnectionTests {
    private func connection() -> WaveChatConnection {
        WaveChatConnection(repoPath: "/tmp/nowhere", waveName: "ship")
    }

    /// A wire-shaped `turn` SSE payload. `text` is spliced raw, so escape
    /// inside it (e.g. `\\n`) as the server would.
    private func frame(id: String, role: String = "assistant", text: String, status: String, items: String = "[]") -> String {
        "{\"id\":\"\(id)\",\"role\":\"\(role)\",\"text\":\"\(text)\",\"status\":\"\(status)\",\"items\":\(items),\"created_at\":\"2026-07-04T00:00:00Z\"}"
    }

    /// A wire-shaped `turn-delta` SSE payload growing the turn named `turnId`
    /// by one item.
    private func deltaFrame(turnId: String, item: String) -> String {
        "{\"turn_id\":\"\(turnId)\",\"item\":\(item)}"
    }

    private func streamMessage(id: String, text: String) -> String {
        "{\"type\":\"message\",\"id\":\"\(id)\",\"text\":\"\(text)\",\"phase\":\"stream\"}"
    }

    @Test("start installs durable history before waiting for a listener")
    func startLoadsDurableHistoryOffline() async throws {
        let saved = try ChatTurn(
            id: "turn-7",
            role: .user,
            text: "saved before restart",
            status: .completed,
            items: [],
            createdAt: "2026-07-15T04:00:00Z",
            body: nil,
            activity: nil
        )
        let conn = WaveChatConnection(
            repoPath: "/tmp/no-wave-listener",
            waveName: "ship",
            loadHistory: { repo, wave, limit in
                #expect(repo == "/tmp/no-wave-listener")
                #expect(wave == "ship")
                #expect(limit == 12)
                return ChatHistorySnapshot(
                    state: .partial,
                    detail: "Later history is unreadable.",
                    turns: [saved],
                    truncated: true
                )
            }
        )
        conn.start()
        defer { conn.stop() }

        for _ in 0..<100 where conn.historyState == nil {
            try await Task.sleep(for: .milliseconds(5))
        }

        #expect(conn.turns == [saved])
        #expect(conn.historyState == .partial)
        #expect(conn.historyDetail == "Later history is unreadable.")
        #expect(conn.historyTruncated)
        #expect(conn.phase == .notRunning)

        conn.handle(event: "turn", data: frame(id: "turn-8", text: "live", status: "completed"))
        #expect(conn.historyState == .partial, "a live frame cannot repair durable history")
    }

    @Test("the first durable turn promotes missing history")
    func firstTurnPromotesMissingHistory() async throws {
        let conn = WaveChatConnection(
            repoPath: "/tmp/no-wave-listener",
            waveName: "ship",
            loadHistory: { _, _, _ in
                ChatHistorySnapshot(state: .missing, detail: "No journal.", turns: [], truncated: false)
            }
        )
        conn.start()
        defer { conn.stop() }

        for _ in 0..<100 where conn.historyState == nil {
            try await Task.sleep(for: .milliseconds(5))
        }
        conn.handle(event: "turn", data: frame(id: "turn-1", text: "now durable", status: "completed"))

        #expect(conn.historyState == .available)
        #expect(conn.historyDetail == nil)
    }

    @Test("turn-delta frames grow the open turn to match a whole-turn reconstruction")
    func turnDeltaFramesReconstructTheTurn() {
        let conn = connection()
        // The turn opens as a whole (empty, running) frame.
        conn.handle(event: "turn", data: frame(id: "turn-1", text: "", status: "running"))
        #expect(conn.turns.count == 1)
        #expect(conn.turns[0].text == "")

        // Prose arrives as stream-message increments — concatenated into text,
        // never added to items, exactly as the listener folds.
        conn.handle(event: "turn-delta", data: deltaFrame(turnId: "turn-1", item: streamMessage(id: "text-0", text: "I fixed ")))
        conn.handle(event: "turn-delta", data: deltaFrame(turnId: "turn-1", item: streamMessage(id: "text-1", text: "the parser.")))
        #expect(conn.turns[0].text == "I fixed the parser.")
        #expect(conn.turns[0].items.isEmpty)

        // A non-message item appends to items and leaves text alone.
        let tool = "{\"type\":\"tool\",\"id\":\"t-1\",\"name\":\"Bash\",\"status\":\"completed\",\"input\":null,\"output\":\"ok\"}"
        conn.handle(event: "turn-delta", data: deltaFrame(turnId: "turn-1", item: tool))
        #expect(conn.turns[0].text == "I fixed the parser.")
        #expect(conn.turns[0].items.count == 1)
        #expect(conn.turns[0].isInProgress)

        // The finalized whole turn re-baselines under the same id.
        conn.handle(event: "turn", data: frame(id: "turn-1", text: "I fixed the parser.", status: "completed", items: "[\(tool)]"))
        #expect(conn.turns.count == 1)
        #expect(!conn.turns[0].isInProgress)
    }

    @Test("a turn-delta for an unknown turn id is dropped, not misapplied")
    func deltaForUnknownTurnDrops() {
        let conn = connection()
        conn.handle(event: "turn", data: frame(id: "turn-1", text: "hi", status: "running"))
        conn.handle(event: "turn-delta", data: deltaFrame(turnId: "turn-99", item: streamMessage(id: "text-0", text: "stray")))
        #expect(conn.turns.count == 1)
        #expect(conn.turns[0].text == "hi", "a delta for a turn we never opened changes nothing")
    }

    @Test("repeated same-id frames grow the turn in place and finalize it")
    func sameIdFramesUpdateInPlace() {
        let conn = connection()
        conn.handle(event: "turn", data: frame(id: "turn-1", role: "user", text: "status?", status: "completed"))
        conn.handle(event: "turn", data: frame(id: "turn-2", text: "thinking", status: "running"))

        #expect(conn.turns.count == 2)
        #expect(conn.turns[1].isInProgress)
        #expect(conn.turns[1].text == "thinking")

        // The open turn re-sent with more text and a first item.
        let items = "[{\"type\":\"tool\",\"id\":\"item-0\",\"name\":\"Bash\",\"status\":\"completed\",\"input\":null,\"output\":\"ok\"}]"
        conn.handle(event: "turn", data: frame(id: "turn-2", text: "thinking\\nmore", status: "running", items: items))
        #expect(conn.turns.count == 2, "same id replaces, never appends")
        #expect(conn.turns[1].text == "thinking\nmore")
        #expect(conn.turns[1].items.count == 1)
        #expect(conn.turns[1].isInProgress)

        // Finalization flips the status under the same id.
        conn.handle(event: "turn", data: frame(id: "turn-2", text: "thinking\\nmore", status: "completed", items: items))
        #expect(conn.turns.count == 2)
        #expect(!conn.turns[1].isInProgress)
        #expect(conn.turns.map(\.id) == ["turn-1", "turn-2"])
    }

    @Test("running turns can finalize as failed or interrupted")
    func terminalStatusFlips() {
        let conn = connection()
        conn.handle(event: "turn", data: frame(id: "turn-1", text: "a", status: "running"))
        conn.handle(event: "turn", data: frame(id: "turn-1", text: "a", status: "interrupted"))
        #expect(conn.turns[0].status == .interrupted)

        conn.handle(event: "turn", data: frame(id: "turn-2", text: "b", status: "running"))
        conn.handle(event: "turn", data: frame(id: "turn-2", text: "b", status: "failed"))
        #expect(conn.turns[1].status == .failed)
        #expect(conn.turns.allSatisfy { !$0.isInProgress })
    }

    @Test("frames sort by sequence however they arrive")
    func framesSortBySequence() {
        let conn = connection()
        // Replay serves the open turn after the finalized thread; a user turn
        // committed mid-turn can also arrive out of id order.
        conn.handle(event: "turn", data: frame(id: "turn-3", role: "user", text: "hey", status: "completed"))
        conn.handle(event: "turn", data: frame(id: "turn-2", text: "grinding", status: "running"))
        #expect(conn.turns.map(\.id) == ["turn-2", "turn-3"])

        // A replace frame (same id, so the same (sequence, id) sort key) skips
        // the sort — the order must survive the in-place growth untouched.
        conn.handle(event: "turn", data: frame(id: "turn-2", text: "grinding\\nstill", status: "running"))
        #expect(conn.turns.map(\.id) == ["turn-2", "turn-3"])
        #expect(conn.turns[0].text == "grinding\nstill")
    }

    @Test("unparseable ids keep a deterministic order")
    func unparseableIdsAreDeterministic() {
        let conn = connection()
        // Both fall to the `.max` sentinel sequence; id breaks the tie.
        conn.handle(event: "turn", data: frame(id: "weird-b", text: "b", status: "completed"))
        conn.handle(event: "turn", data: frame(id: "weird-a", text: "a", status: "completed"))
        conn.handle(event: "turn", data: frame(id: "turn-7", text: "real", status: "completed"))
        #expect(conn.turns.map(\.id) == ["turn-7", "weird-a", "weird-b"])
    }

    @Test("non-turn events are ignored")
    func nonTurnEventsIgnored() {
        let conn = connection()
        conn.handle(event: "ping", data: frame(id: "turn-1", text: "x", status: "running"))
        #expect(conn.turns.isEmpty)
    }

    @Test("state events update the observable loop state")
    func stateEventsUpdateLoopState() {
        let conn = connection()
        #expect(conn.loopState == .idle)
        conn.handle(event: "state", data: "turning")
        #expect(conn.loopState == .turning)
        conn.handle(event: "state", data: "interrupting")
        #expect(conn.loopState == .interrupting)
        conn.handle(event: "state", data: "idle")
        #expect(conn.loopState == .idle)

        // Unknown state names are dropped, never crash the stream.
        conn.handle(event: "state", data: "transcending")
        #expect(conn.loopState == .idle)
        // State payloads never masquerade as turns.
        #expect(conn.turns.isEmpty)
    }

    @Test("playhead events expose location queue and return target")
    func playheadEventsExposeNavigation() {
        let conn = connection()
        let json = """
        {
          "stack": [{
            "id": "inv-wave", "flow": "wave",
            "steps": [{"name":"pursue","kind":"skill"}],
            "cursor": 0, "iteration": 3,
            "queue": [{
              "id":"inv-review", "flow":"review-design",
              "steps":[{"name":"clarify","kind":"skill"}]
            }]
          }],
          "active": null,
          "now": {
            "invocation_id":"inv-wave", "flow":"wave",
            "step":"pursue", "kind":"skill", "index":0, "total":1,
            "iteration":3
          },
          "next": {
            "invocation_id":"inv-review", "flow":"review-design",
            "step":"clarify", "kind":"skill", "index":0, "total":1,
            "iteration":0
          },
          "return_to": null
        }
        """

        conn.handle(event: "playhead", data: json)

        #expect(conn.playhead?.now?.step == "pursue")
        #expect(conn.playhead?.next?.flow == "review-design")
        #expect(conn.playhead?.stack.last?.queue.map(\.flow) == ["review-design"])
        #expect(conn.turns.isEmpty)
    }

    @Test("message ops encode to the wire values the server expects")
    func opWireEncoding() {
        #expect(WaveMessageOp.message.rawValue == "message")
        #expect(WaveMessageOp.steer.rawValue == "steer")
        #expect(WaveMessageOp.interrupt.rawValue == "interrupt")
    }

    // MARK: - Current wire frame

    @Test("a current assistant-turn SSE frame decodes and renders")
    func currentAssistantFrameDecodes() throws {
        // A completed Wave turn about one Project-owned Task Work. Four
        // command items exercise the live renderer, including one failure.
        let conn = connection()
        var parser = SSEFrameParser()
        var frames: [SSEFrameParser.Frame] = []
        let stream = "event: state\ndata: turning\n\nevent: turn\ndata: \(currentTurnFrame)\n\n"
        for byte in stream.utf8 {
            if let frame = parser.consume(byte) { frames.append(frame) }
        }

        #expect(frames.count == 2)
        for frame in frames {
            conn.handle(event: frame.event, data: frame.data)
        }

        #expect(conn.loopState == .turning)
        #expect(conn.turns.count == 1)
        let turn = try #require(conn.turns.first)
        #expect(turn.id == "turn-101")
        #expect(turn.role == .assistant)
        #expect(turn.status == .completed)
        #expect(turn.text.hasPrefix("Project runtime-simplification is pursuing Task INF-872"))
        #expect(turn.createdAtDate != nil)
        #expect(turn.items.count == 4)
        guard case let .command(_, command, cwd, status, output, exitCode, _) = turn.items[3] else {
            Issue.record("expected a command item")
            return
        }
        #expect(command == ["gh", "pr", "checks", "912"])
        #expect(cwd == "/Users/jack/src/loopflow.infrastructure.inf-872")
        #expect(status == .failed)
        #expect(output?.contains("build is still pending") == true)
        #expect(exitCode == 1)
    }
}

// SSE framing. Hand-rolled line splitting is the point under test:
// `AsyncBytes.lines` drops the empty lines that terminate SSE frames, which is
// exactly the bug that left the pane blank against a healthy server. The
// parser must emit a frame per blank line, join multi-line data, strip CRs,
// and swallow keep-alive comments.
@Suite("SSE frame parsing")
struct SSEFrameParserTests {
    private func frames(_ raw: String) -> [SSEFrameParser.Frame] {
        var parser = SSEFrameParser()
        var out: [SSEFrameParser.Frame] = []
        for byte in raw.utf8 {
            if let frame = parser.consume(byte) { out.append(frame) }
        }
        return out
    }

    @Test("blank lines delimit frames; event names and data come through")
    func framesSplitOnBlankLines() {
        let out = frames("event: state\ndata: turning\n\nevent: turn\ndata: {\"id\":1}\n\n")
        #expect(out == [
            .init(event: "state", data: "turning"),
            .init(event: "turn", data: "{\"id\":1}"),
        ])
    }

    @Test("CRLF line endings parse identically")
    func crlfLineEndings() {
        let out = frames("event: state\r\ndata: idle\r\n\r\n")
        #expect(out == [.init(event: "state", data: "idle")])
    }

    @Test("multi-line data joins with newlines")
    func multiLineData() {
        let out = frames("event: turn\ndata: first\ndata: second\n\n")
        #expect(out == [.init(event: "turn", data: "first\nsecond")])
    }

    @Test("comment keep-alives and dataless blank lines emit nothing")
    func commentsAndEmptyFramesDrop() {
        let out = frames(": ping\n\n: ping\n\nevent: state\ndata: idle\n\n")
        #expect(out == [.init(event: "state", data: "idle")])
    }

    /// The transport reads `Data` chunks, not single bytes, and a chunk lands
    /// wherever the network splits it — mid-frame, mid-line, or mid-UTF-8. The
    /// frames must come out the same either way, blank-line boundaries included:
    /// losing those is what ruled out `AsyncBytes.lines`.
    @Test("chunked feeding yields the same frames as byte-at-a-time, at any split")
    func chunkedFeedingMatchesByteFeeding() {
        let raw = "event: state\ndata: turning\n\n: ping\n\nevent: turn\ndata: {\"id\":\"t-1\"}\ndata: more\n\n"
        let expected = frames(raw)
        #expect(expected.count == 2)

        let bytes = Array(raw.utf8)
        for split in 1..<bytes.count {
            var parser = SSEFrameParser()
            var out = parser.consume(Data(bytes[..<split]))
            out += parser.consume(Data(bytes[split...]))
            #expect(out == expected, "split at \(split) changed the frames")
        }
    }

    @Test("an unterminated frame stays pending")
    func unterminatedFramePends() {
        let out = frames("event: turn\ndata: {\"id\":1}\n")
        #expect(out.isEmpty, "no blank line yet — the turn may still be streaming")
    }
}

/// A complete `turn` frame using the current Wave → Project → Task vocabulary.
private let currentTurnFrame = #"{"id":"turn-101","role":"assistant","text":"Project runtime-simplification is pursuing Task INF-872. The Task Work is running in its worktree and opened PR #912. Focused Swift tests passed; the required build check is still pending, so the Task remains submitted.","status":"completed","items":[{"type":"command","id":"call_status","command":["lf","status","infrastructure","--json"],"cwd":"/Users/jack/src/loopflow.infrastructure","status":"completed","output":"INF-872 · running · Task Work ts_872\n","exit_code":0,"duration_ms":81},{"type":"command","id":"call_diff","command":["git","diff","--stat","origin/main...HEAD"],"cwd":"/Users/jack/src/loopflow.infrastructure.inf-872","status":"completed","output":"2 files changed, 18 insertions(+), 6 deletions(-)\n","exit_code":0,"duration_ms":34},{"type":"command","id":"call_test","command":["swift","test","--package-path","swift","--filter","RegistryQueryTests"],"cwd":"/Users/jack/src/loopflow.infrastructure.inf-872","status":"completed","output":"9 tests passed\n","exit_code":0,"duration_ms":2140},{"type":"command","id":"call_checks","command":["gh","pr","checks","912"],"cwd":"/Users/jack/src/loopflow.infrastructure.inf-872","status":"failed","output":"build is still pending\n","exit_code":1,"duration_ms":417}],"created_at":"2026-07-13T20:00:00Z","body":null,"activity":null}"#

// Composer verb selection: the smallest honest mapping from (loop state, has
// text) to what the primary/secondary buttons do. The view renders this
// directly, so the table below IS the composer's behavior.
@Suite("Composer verb selection")
struct ComposerVerbTests {
    @Test("idle + text sends; idle + empty is disabled")
    func idleSends() {
        let withText = composerVerbs(state: .idle, hasText: true)
        #expect(withText.primary == .send)
        #expect(withText.primaryEnabled)
        #expect(withText.secondary == nil)

        let empty = composerVerbs(state: .idle, hasText: false)
        #expect(empty.primary == .send)
        #expect(!empty.primaryEnabled)
    }

    @Test("turning + text steers, with Interrupt & Send one skill away")
    func turningSteers() {
        let verbs = composerVerbs(state: .turning, hasText: true)
        #expect(verbs.primary == .steer)
        #expect(verbs.primaryEnabled)
        #expect(verbs.secondary == .interruptAndSend)
    }

    @Test("turning + empty interrupts")
    func turningEmptyInterrupts() {
        let verbs = composerVerbs(state: .turning, hasText: false)
        #expect(verbs.primary == .interrupt)
        #expect(verbs.primaryEnabled)
        #expect(verbs.secondary == nil)
    }

    @Test("interrupting degrades: text queues a Send, re-interrupt is disabled")
    func interruptingDegrades() {
        let withText = composerVerbs(state: .interrupting, hasText: true)
        #expect(withText.primary == .send)
        #expect(withText.primaryEnabled)
        #expect(withText.secondary == nil)

        let empty = composerVerbs(state: .interrupting, hasText: false)
        #expect(empty.primary == .interrupt)
        #expect(!empty.primaryEnabled, "the cancel is already in flight")
    }

    @Test("failed behaves like idle: a message revives the loop")
    func failedSends() {
        let verbs = composerVerbs(state: .failed, hasText: true)
        #expect(verbs.primary == .send)
        #expect(verbs.primaryEnabled)
    }
}
