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

    @Test("channel-tagged frames: own channel renders, other channels skip")
    func channelTaggedFramesFilterToThePrimaryChannel() {
        let conn = connection()
        // A family subscription tags work-line frames with their channel;
        // the wave's own frames ride untagged. WaveChat renders the primary
        // channel only — a tagged frame for a work line is skipped, one
        // tagged with the wave's own name (or untagged) renders.
        let child = "{\"id\":\"turn-1\",\"role\":\"user\",\"text\":\"child report\",\"status\":\"completed\",\"items\":[],\"created_at\":\"2026-07-04T00:00:00Z\",\"channel\":\"ship.148e\"}"
        conn.handle(event: "turn", data: child)
        #expect(conn.turns.isEmpty, "a work-line channel's frame is not this thread")

        let own = "{\"id\":\"turn-2\",\"role\":\"user\",\"text\":\"to the wave\",\"status\":\"completed\",\"items\":[],\"created_at\":\"2026-07-04T00:00:00Z\",\"channel\":\"ship\"}"
        conn.handle(event: "turn", data: own)
        conn.handle(event: "turn", data: frame(id: "turn-3", role: "user", text: "untagged", status: "completed"))
        #expect(conn.turns.map(\.id) == ["turn-2", "turn-3"], "own-channel and untagged frames render")
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

    @Test("memory events expose the latest curation summary")
    func memoryEventsExposeSummary() {
        let conn = connection()
        #expect(conn.memorySummary == nil)
        conn.handle(event: "memory", data: "fold is truth")
        #expect(conn.memorySummary == "fold is truth")
        conn.handle(event: "memory", data: "second curation")
        #expect(conn.memorySummary == "second curation")
        // Memory payloads never masquerade as turns or states.
        #expect(conn.turns.isEmpty)
        #expect(conn.loopState == .idle)
    }

    @Test("op frames expose the wave's latest run motion")
    func opFramesExposeMotion() {
        let conn = connection()
        #expect(conn.lastOp == nil)

        // A worker run starting — kind mirrors the run_events ledger vocabulary.
        let started = "{\"kind\":\"run.started\",\"run_id\":\"run-42\",\"flow\":\"build\",\"task\":\"wire it\",\"summary\":null,\"ts\":\"2026-07-06T00:00:00Z\"}"
        conn.handle(event: "op", data: started)
        #expect(conn.lastOp?.kind == "run.started")
        #expect(conn.lastOp?.runID == "run-42")
        #expect(conn.lastOp?.flow == "build")
        #expect(conn.lastOp?.task == "wire it")

        // op frames never masquerade as turns or loop states.
        #expect(conn.turns.isEmpty)
        #expect(conn.loopState == .idle)

        // A finish carries a summary, no flow — the same channel, latest wins.
        let completed = "{\"kind\":\"run.completed\",\"run_id\":\"run-42\",\"flow\":null,\"task\":null,\"summary\":\"pr opened\",\"ts\":\"2026-07-06T00:01:00Z\"}"
        conn.handle(event: "op", data: completed)
        #expect(conn.lastOp?.kind == "run.completed")
        #expect(conn.lastOp?.flow == nil)
        #expect(conn.lastOp?.summary == "pr opened")
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

    @Test("message ops encode to the wire values the server expects")
    func opWireEncoding() {
        #expect(WaveMessageOp.message.rawValue == "message")
        #expect(WaveMessageOp.steer.rawValue == "steer")
        #expect(WaveMessageOp.interrupt.rawValue == "interrupt")
    }

    // MARK: - Real wire frame

    @Test("a captured real assistant-turn SSE frame decodes and renders")
    func realAssistantFrameDecodes() throws {
        // Captured verbatim from a live `lf loop goals` server on 2026-07-04
        // (then served on /conversation/stream; the same turn frames now ride
        // GET /events): a completed assistant turn with four command items,
        // one failed.
        let conn = connection()
        var parser = SSEFrameParser()
        var frames: [SSEFrameParser.Frame] = []
        let stream = "event: state\ndata: turning\n\nevent: turn\ndata: \(realTurnFrame)\n\n"
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
        #expect(turn.text.hasPrefix("This supersedes the kickoff design"))
        #expect(turn.createdAtDate != nil)
        #expect(turn.from == nil, "explicit null decodes as absent")
        #expect(turn.items.count == 4)
        guard case let .command(_, command, cwd, status, output, exitCode, _) = turn.items[3] else {
            Issue.record("expected a command item")
            return
        }
        #expect(command.count == 1)
        #expect(cwd == "/Users/jack/src/loopflow.goals")
        #expect(status == .failed)
        #expect(output?.contains("worker capacity") == true)
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

    @Test("an unterminated frame stays pending")
    func unterminatedFramePends() {
        let out = frames("event: turn\ndata: {\"id\":1}\n")
        #expect(out.isEmpty, "no blank line yet — the turn may still be streaming")
    }
}

/// A complete, unedited `turn` frame payload from a live wave server.
private let realTurnFrame = #"{"id":"turn-101","role":"assistant","text":"This supersedes the kickoff design in an important way: the flow must avoid interactive `design` or it will park headlessly. I’m recording the corrected shape and writing it back to Asana from the wave root, then I’ll try the stacked implementation dispatch again.\nMemory and Asana are updated; the worker session still exists, but the registry may have cleared if the process exited inside tmux. I’m using the queue as the authority for whether the stacked implementation can start.\nCaptured the corrected review-design result in memory and wrote it back to Asana.\n\nThe important correction is now durable: `greenfield` must be `scaffold -> implement -> run -> gate`, not `scaffold -> design -> ...`, because interactive `design` parks in headless flow execution.\n\nTried to start the stacked implementation worker on commit `7c64bcef1`, but the queue still reports `goals` at worker capacity `1/1`. No new worker launched.","status":"completed","items":[{"type":"command","id":"call_KgomG7XIADii2YrIuFuCY8nL","command":["/bin/zsh -lc \"tmux list-sessions | rg 'lf-jack-heart-goals|148e0e02|68a936f3'\""],"cwd":"/Users/jack/src/loopflow.goals","status":"completed","output":"lf-jack-heart-goals-20260704_1626-68a936f3-25ac-4daf-9579-6206602def88: 1 windows (created Sat Jul  4 16:26:19 2026)\n","exit_code":0,"duration_ms":0},{"type":"command","id":"call_YgukOJma1DKNLCnfcKXxjQgU","command":["/bin/zsh -lc 'lf memory add \"2026-07-04 worker review-design result for Asana 1216257471904678: reshaped scratch/prove-the-language.md at commit 7c64bcef1 after checking actual code. Fixed real design defect: greenfield flow must not chain interactive design because headless flows park on interactive skills (flow.rs:272 -> WaitInteractive). Correct flow is scaffold -> implement -> run -> gate, with scaffold seeding scratch/<branch>.md directly from GOAL. Done gates split: deterministic CI registry test proves scaffold/run/greenfield resolve and greenfield expands with zero interactive skills; manual one-shot probe covers the paid agentic build. Metric is binary: probe dir .lf/skills/ must stay empty.\"'"],"cwd":"/Users/jack/src/loopflow.goals","status":"completed","output":"memory fact added for wave 'goals': 2026-07-04 worker review-design result for Asana 1216257471904678: reshaped scratch/prove-the-language.md at commit 7c64bcef1 after checking actual code. Fixed real design defect: greenfield flow must not chain interactive design because headless flows park on interactive skills (flow.rs:272 -> WaitInteractive). Correct flow is scaffold -> implement -> run -> gate, with scaffold seeding scratch/<branch>.md directly from GOAL. Done gates split: deterministic CI registry test proves scaffold/run/greenfield resolve and greenfield expands with zero interactive skills; manual one-shot probe covers the paid agentic build. Metric is binary: probe dir .lf/skills/ must stay empty.\n","exit_code":0,"duration_ms":0},{"type":"command","id":"call_EeLYzG54t0Z7M3fiEuqw96TA","command":["/bin/zsh -lc \"lf op pm update --wave goals --id 1216257471904678 --title \\\"Prove the language — three reference builds from goals\\\" --notes \\\"Review-design landed in worker run 148e0e02 at commit 7c64bcef1. It corrected the implementation plan against actual flow behavior: headless flows park on interactive design skills, so the greenfield flow must be scaffold -> implement -> run -> gate, with scaffold seeding scratch/<branch>.md from GOAL. Deterministic gate: registry test proves scaffold/run/greenfield resolve and greenfield expands with zero interactive skills. Manual one-shot probe remains outside cargo test. Binary metric: the probe dir's .lf/skills/ stays empty.\\\"\""],"cwd":"/Users/jack/src/loopflow.goals","status":"completed","output":"goals: updated task 1216257471904678\n","exit_code":0,"duration_ms":1186},{"type":"command","id":"call_dTTuy9oEf1I3oPAhYs4W3NSw","command":["/bin/zsh -lc \"lf q worker run goals --flow build --stack 148e0e02-06c3-4657-9040-68c02249dc9f --task \\\"Implement the accepted review-design plan for Asana 1216257471904678 from scratch/prove-the-language.md, building on commit 7c64bcef1. Scope: add first-class builtin scaffold and run prompt skills registered through build.rs; add a greenfield flow that expands to scaffold -> implement -> run -> gate with zero interactive skills; add the deterministic registry test proving scaffold/run/greenfield resolve and greenfield contains no WaitInteractive skills; add scripts/prove_language_cli.sh as a manual one-shot probe from an empty temp dir and a tiny GOAL.md, with the binary success metric that the probe dir's .lf/skills/ remains empty. Do not include the interactive design skill. Keep integrate/server and platform-build/rams/mobile probes deferred. Use lf op pm update --wave goals for write-back.\\\"\""],"cwd":"/Users/jack/src/loopflow.goals","status":"failed","output":"Error: wave 'goals' already at worker capacity (1/1)\n","exit_code":1,"duration_ms":0}],"created_at":"2026-07-04T23:35:42.657239Z","from":null}"#

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
