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
        #expect(wave.status == .running)
        #expect(wave.iteration == 3)
        #expect(wave.direction == ["ux", "clarity"])
        #expect(wave.area == ["src/"])

        #expect(wave.triggers.count == 2)
        #expect(wave.triggers[0].signal == .repo)
        #expect(wave.triggers[0].flow == "integrate")
        #expect(wave.triggers[1].signal == .ciFailure)
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
