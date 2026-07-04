// Contract tests: golden JSON fixtures must decode through Swift models.

import Foundation
import Testing
@testable import LoopflowCore

@Suite("Contract: Golden Fixtures")
struct ContractTests {
    private func fixtureData(_ name: String) throws -> Data {
        // Navigate from this file to tests/fixtures/ at repo root.
        let thisFile = URL(fileURLWithPath: #filePath)
        let repoRoot = thisFile
            .deletingLastPathComponent() // ConcertoTests/
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
        #expect(wave.flow == "build")
        #expect(wave.goal == "ship-roadmap")
        #expect(wave.status == .running)
        #expect(wave.direction == ["ux", "clarity"])
        #expect(wave.area == ["src/"])
        #expect(wave.parentWaveId == "wave_parent999")

        #expect(wave.repos.count == 1)
        #expect(wave.repos.first?.repo == "/home/user/project")
        #expect(wave.repos.first?.status == .running)
        #expect(wave.repos.first?.iteration == 3)
        #expect(wave.repos.first?.openPRCount == 1)
        #expect(wave.repos.first?.localWorktree == "/home/user/project/.claude/worktrees/engbot")
        #expect(wave.repos.first?.remoteBranch == "engbot/build-3")
        #expect(wave.repos.first?.commits.count == 1)

        #expect(wave.triggers.count == 2)
        #expect(wave.triggers[0].signal == .repo)
        #expect(wave.triggers[0].flow == "integrate")
        #expect(wave.triggers[1].signal == .ciFailure)
        #expect(wave.crons.count == 1)
        #expect(wave.crons[0].flow == "wave-polish")
        #expect(wave.crons[0].schedule == "0 0 * * 1")
        #expect(wave.crons[0].lastTriggeredAt != nil)
    }

    @Test("trigger.json decodes signal and optional fields")
    func triggerFixtureParses() throws {
        let json = try fixtureJSON("trigger.json")
        guard let id = json["id"] as? String,
              let signalStr = json["signal"] as? String,
              let signal = Trigger.Signal(rawValue: signalStr) else {
            Issue.record("trigger fixture missing required fields")
            return
        }

        let trigger = Trigger(
            id: id,
            signal: signal,
            flow: json["flow"] as? String,
            sourceWaveId: json["source_wave_id"] as? String
        )

        #expect(trigger.id == "trig_abc123")
        #expect(trigger.signal == .wave)
        #expect(trigger.flow == "build")
        #expect(trigger.sourceWaveId == "wave_upstream")
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
        #expect(input == "TODO")
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

    @Test("activation_log.json has expected shape")
    func activationLogFixtureShape() throws {
        let json = try fixtureJSON("activation_log.json")

        #expect(json["object"] as? String == "activation_log")
        #expect(json["wave_id"] as? String == "wave_abc123")
        #expect(json["trigger_id"] as? String == "trig_001")
        #expect(json["outcome"] as? String == "started")
    }
}

private enum ContractError: Error {
    case invalidFixture(String)
}
