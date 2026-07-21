// Contract tests: golden JSON fixtures must decode through Swift models.

import Foundation
import Testing
@testable import Loopflow

@Suite("Contract: Golden Fixtures")
struct ContractTests {
    private func fixtureData(_ name: String) throws -> Data {
        // Navigate from this file to tests/fixtures/ at repo root.
        let thisFile = URL(fileURLWithPath: #filePath)
        let repoRoot = thisFile
            .deletingLastPathComponent() // LoopflowTests/
            .deletingLastPathComponent() // swift/
            .deletingLastPathComponent() // repo root
        let path = repoRoot.appendingPathComponent("tests/fixtures/\(name)")
        return try Data(contentsOf: path)
    }

    private func fixtureJSON(_ name: String) throws -> [String: Any] {
        let data = try fixtureData(name)
        guard let json = try JSONSerialization.jsonObject(with: data) as? [String: Any] else {
            throw ContractError.invalidFixture(name)
        }
        return json
    }

    @Test("chat_turn.json decodes through the wave chat models")
    func chatTurnFixtureParses() throws {
        let data = try fixtureData("dto/chat_turn.json")
        let turn = try JSONDecoder().decode(ChatTurn.self, from: data)

        #expect(turn.id == "turn-3")
        #expect(turn.sequence == 3)
        #expect(turn.role == .assistant)
        #expect(turn.status == .running)
        #expect(turn.isInProgress)
        #expect(turn.createdAtDate != nil)
        #expect(turn.items.count == 6)

        guard case let .command(id, command, cwd, status, output, exitCode, durationMs) = turn.items[0] else {
            Issue.record("item 0 should be a command")
            return
        }
        #expect(id == "item-0")
        #expect(command == ["cargo", "test"])
        #expect(cwd == "/home/user/project")
        #expect(status == .completed)
        #expect(output == "test result: ok. 42 passed")
        #expect(exitCode == 0)
        #expect(durationMs == 1234)

        guard case let .file(_, changes, _) = turn.items[1] else {
            Issue.record("item 1 should be a file")
            return
        }
        #expect(changes.first?.path == "src/main.rs")
        #expect(changes.first?.kind == "modified")
        #expect(changes.first?.diff?.contains("+new") == true)

        guard case let .message(_, text, phase) = turn.items[2] else {
            Issue.record("item 2 should be a message")
            return
        }
        #expect(text == "Narrating progress")
        #expect(phase == "progress")

        guard case let .thought(_, thoughtText) = turn.items[3] else {
            Issue.record("item 3 should be a thought")
            return
        }
        #expect(thoughtText.contains("run the tests"))

        guard case let .tool(_, name, _, input, toolOutput) = turn.items[4] else {
            Issue.record("item 4 should be a tool")
            return
        }
        #expect(name == "Grep")
        #expect(input == .string("TODO"))
        #expect(toolOutput == "3 matches")

        // Interrupted state with explicit-null optionals decodes as nils.
        guard case let .command(_, rawCommand, _, interruptedStatus, nilOutput, nilExitCode, nilDurationMs) = turn.items[5] else {
            Issue.record("item 5 should be a command")
            return
        }
        #expect(rawCommand == ["cargo test --workspace"])
        #expect(interruptedStatus == .interrupted)
        #expect(nilOutput == nil)
        #expect(nilExitCode == nil)
        #expect(nilDurationMs == nil)

        // Round-trips: re-encode and decode again yields an identical turn.
        let reencoded = try JSONEncoder().encode(turn)
        let roundTripped = try JSONDecoder().decode(ChatTurn.self, from: reencoded)
        #expect(roundTripped == turn)

    }

    @Test("turn_delta.json decodes and grows a turn through absorbing")
    func turnDeltaFixtureParses() throws {
        let data = try fixtureData("dto/turn_delta.json")
        let delta = try JSONDecoder().decode(TurnDelta.self, from: data)

        #expect(delta.turnId == "turn-3")
        guard case let .message(id, text, phase) = delta.item else {
            Issue.record("turn-delta item should be a message")
            return
        }
        #expect(id == "text-7")
        #expect(phase == "stream")
        #expect(text.contains("edge case"))

        // Applying the delta grows a turn exactly as the listener's fold does: a
        // stream message concatenates into text, never into items.
        let opened = try ChatTurn(
            id: "turn-3", role: .assistant, text: "so ", status: .running, items: [],
            createdAt: "2026-07-03T18:30:00Z", body: nil, activity: nil
        )
        let grown = try opened.absorbing(delta.item)
        #expect(grown.text == "so the parser handles the edge case now.")
        #expect(grown.items.isEmpty)

        // Round-trips: re-encode and decode again yields an identical delta.
        let reencoded = try JSONEncoder().encode(delta)
        let roundTripped = try JSONDecoder().decode(TurnDelta.self, from: reencoded)
        #expect(roundTripped == delta)
    }

    @Test("absorbing keeps commentary as a curatable item, not folded into prose")
    func absorbingKeepsCommentaryAsItem() throws {
        let opened = try ChatTurn(
            id: "turn-10", role: .assistant, text: "", status: .running, items: [],
            createdAt: "2026-07-10T17:52:05Z", body: nil, activity: nil
        )
        // Operational narration the provider tagged `commentary` must survive as
        // a discrete item so `turnPresentation` can fold it behind a disclosure;
        // the conclusion stays the prose.
        let withStep = try opened.absorbing(
            .message(id: "m-0", text: "Using `wave_clarify` to audit the plan.", phase: "commentary")
        )
        let withConclusion = try withStep.absorbing(
            .message(id: "m-1", text: "Clarification complete.", phase: "final_answer")
        )

        #expect(withConclusion.text == "Clarification complete.")
        #expect(withConclusion.items.count == 1)
        guard case let .message(_, stepText, phase) = withConclusion.items[0] else {
            Issue.record("the commentary should ride as a message item")
            return
        }
        #expect(phase == "commentary")
        #expect(stepText.contains("wave_clarify"))

        // And it curates: the conclusion leads, the narration folds into a step.
        let view = turnPresentation(withConclusion)
        #expect(view.conclusion == "Clarification complete.")
        #expect(view.steps.count == 1)
    }

    @Test("child activity cannot masquerade as an ordinary conversation turn")
    func childActivityEnvelopeIsChecked() throws {
        let activity = try fixtureJSON("dto/child_control_activity.json")
        let invalid: [String: Any] = [
            "id": "turn-activity",
            "role": "user",
            "text": "ordinary prose cannot share the activity envelope",
            "status": "completed",
            "items": [],
            "created_at": "2026-07-13T20:00:00Z",
            "body": NSNull(),
            "activity": activity,
        ]
        let data = try JSONSerialization.data(withJSONObject: invalid)

        #expect(throws: ChatTurnError.self) {
            try JSONDecoder().decode(ChatTurn.self, from: data)
        }
    }

    @Test("post_message_response.json decodes through PostMessageResponse")
    func postMessageResponseFixtureParses() throws {
        // Pins the source-bearing `POST /messages` response for the Mac client.
        let data = try fixtureData("dto/post_message_response.json")
        let posted = try JSONDecoder().decode(PostMessageResponse.self, from: data)

        let message = try #require(posted.message)
        let turn = message.turn
        #expect(turn.id == "turn-4")
        #expect(turn.role == .user)
        #expect(turn.status == .completed)
        #expect(turn.items.isEmpty)
        #expect(message.source == .local(journalSeq: 4))
        #expect(posted.epoch.backing == .local)
        #expect(WaveLoopState(rawValue: posted.state) == .turning)

        // `message` is explicitly Optional: null for a bare interrupt.
        var json = try #require(JSONSerialization.jsonObject(with: data) as? [String: Any])
        json["message"] = NSNull()
        let bareInterrupt = try JSONDecoder().decode(
            PostMessageResponse.self,
            from: JSONSerialization.data(withJSONObject: json)
        )
        #expect(bareInterrupt.message == nil)
    }

    @Test("post_message_error_response.json decodes through PostMessageErrorResponse")
    func postMessageErrorResponseFixtureParses() throws {
        let data = try fixtureData("dto/post_message_error_response.json")
        let rejection = try JSONDecoder().decode(PostMessageErrorResponse.self, from: data)

        #expect(rejection.error.contains("backed by Discord"))
        #expect(rejection.epoch.number == 2)
        if case let .discord(guildId, channelId, open) = rejection.epoch.backing {
            #expect(guildId == "guild-1")
            #expect(channelId == "channel-1")
            #expect(open == .openDiscord(
                label: "Open in Discord",
                url: "https://discord.com/channels/guild-1/channel-1"
            ))
        } else {
            Issue.record("expected Discord epoch")
        }
    }

    @Test("chat_history.json decodes through the durable history models")
    func chatHistoryFixtureParses() throws {
        let data = try fixtureData("dto/chat_history.json")
        let snapshot = try JSONDecoder().decode(ChatHistorySnapshot.self, from: data)

        #expect(snapshot.state == .partial)
        #expect(snapshot.detail?.contains("line 8") == true)
        #expect(snapshot.messages.map(\.turn.id) == ["turn-7"])
        #expect(snapshot.truncated)

        let reencoded = try JSONEncoder().encode(snapshot)
        #expect(try JSONDecoder().decode(ChatHistorySnapshot.self, from: reencoded) == snapshot)
    }

    @Test("wave_loop_states.json pins the shared SSE state vocabulary")
    func loopStateVocabularyPinned() throws {
        // Swift deliberately drops unknown names off the stream (see
        // WaveChatConnectionTests); this pins the vocabulary, not the tolerance.
        let json = try fixtureJSON("dto/wave_loop_states.json")
        let names = try #require(json["states"] as? [String])
        #expect(names.map { WaveLoopState(rawValue: $0) } == [.idle, .turning, .interrupting, .failed])
    }
}

private enum ContractError: Error {
    case invalidFixture(String)
}
