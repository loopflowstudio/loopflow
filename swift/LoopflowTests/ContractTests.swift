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

    @Test("wave.json parses through parseWaveFromJSON")
    func waveFixtureParses() throws {
        let json = try fixtureJSON("wave.json")
        let wave = WaveService.parseWaveFromJSON(json)

        #expect(wave.id == "wave_abc123")
        #expect(wave.name == "engbot")
        #expect(wave.goal == "ship-roadmap")
        #expect(wave.status == .running)
        #expect(wave.direction == ["ux", "clarity"])
        #expect(wave.area == ["src/"])
        #expect(wave.parentWaveId == "wave_parent999")

        #expect(wave.repo == "/home/user/project")
        #expect(wave.iteration == 3)
        #expect(wave.openPRCount == 1)
        #expect(wave.localWorktree == "/home/user/project/.claude/worktrees/engbot")
        #expect(wave.remoteBranch == "engbot/build-3")
        #expect(wave.commits.count == 1)

        // Triggers and crons left the wire in the collapse's organ cut:
        // absent keys parse as empty.
        #expect(wave.triggers.isEmpty)
        #expect(wave.crons.isEmpty)
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
        #expect(turn.from == "worker")
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

        // `from` is explicitly Optional: a payload without the key decodes as
        // nil — no default masking (mirrored in Rust's dto_fixtures).
        var json = try #require(
            JSONSerialization.jsonObject(with: data) as? [String: Any]
        )
        json.removeValue(forKey: "from")
        let stripped = try JSONSerialization.data(withJSONObject: json)
        let unattributed = try JSONDecoder().decode(ChatTurn.self, from: stripped)
        #expect(unattributed.from == nil)
    }

    @Test("post_message_response.json decodes through PostMessageResponse")
    func postMessageResponseFixtureParses() throws {
        // Pins `POST /messages` → `{turn, state}` against Rust's
        // dto_fixtures; a drifted turn shape or state name fails both.
        let data = try fixtureData("dto/post_message_response.json")
        let posted = try JSONDecoder().decode(PostMessageResponse.self, from: data)

        let turn = try #require(posted.turn)
        #expect(turn.id == "turn-4")
        #expect(turn.role == .user)
        #expect(turn.status == .completed)
        #expect(turn.from == nil, "explicit null decodes as absent")
        #expect(turn.items.isEmpty)
        #expect(WaveFlowloopState(rawValue: posted.state) == .turning)

        // `turn` is explicitly Optional: null for a bare interrupt.
        var json = try #require(JSONSerialization.jsonObject(with: data) as? [String: Any])
        json["turn"] = NSNull()
        let bareInterrupt = try JSONDecoder().decode(
            PostMessageResponse.self,
            from: JSONSerialization.data(withJSONObject: json)
        )
        #expect(bareInterrupt.turn == nil)
    }

    @Test("channel_tagged_turn.json decodes through FrameChannelTag + ChatTurn")
    func channelTaggedTurnFixtureParses() throws {
        // A work-line channel's `turn` SSE frame: the ChatTurn JSON plus one
        // extra top-level `channel` key. Pinned against Rust's dto_fixtures —
        // WaveChatConnection peels the tag with FrameChannelTag, then decodes
        // the same bytes as the turn.
        let data = try fixtureData("dto/channel_tagged_turn.json")
        let tag = try JSONDecoder().decode(FrameChannelTag.self, from: data)
        #expect(tag.channel == "ship.148e0e02")

        let turn = try JSONDecoder().decode(ChatTurn.self, from: data)
        #expect(turn.id == "turn-7")
        #expect(turn.role == .assistant)
        #expect(turn.status == .completed)
        #expect(turn.from == "worker")

        // Untagged frame (the primary channel): absent key decodes as nil.
        var json = try #require(JSONSerialization.jsonObject(with: data) as? [String: Any])
        json.removeValue(forKey: "channel")
        let untagged = try JSONDecoder().decode(
            FrameChannelTag.self,
            from: JSONSerialization.data(withJSONObject: json)
        )
        #expect(untagged.channel == nil, "absent channel = the wave's own channel")
    }

    @Test("wave_flowloop_states.json pins the shared SSE state vocabulary")
    func flowloopStateVocabularyPinned() throws {
        // The same fixture Rust's dto_fixtures checks against FlowloopState::name;
        // a renamed state fails both languages. Swift still deliberately drops
        // unknown names off the stream (see WaveChatConnectionTests) — this
        // pins the vocabulary, not the tolerance.
        let json = try fixtureJSON("dto/wave_flowloop_states.json")
        let names = try #require(json["states"] as? [String])
        #expect(names.map { WaveFlowloopState(rawValue: $0) } == [.idle, .turning, .interrupting, .failed])
    }
}

private enum ContractError: Error {
    case invalidFixture(String)
}
