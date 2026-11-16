import Foundation
import Testing
@testable import Loopflow

private final class InteractionRecords: @unchecked Sendable {
    private let lock = NSLock()
    private var records: [Interaction.Record] = []
    func append(_ record: Interaction.Record) { lock.withLock { records.append(record) } }
    var values: [Interaction.Record] { lock.withLock { records } }
}

@Suite("Interaction journeys")
struct InteractionTests {
    @Test @MainActor
    func eventClockIncludesQueueingAndSeparatesAcknowledgmentFromUsability() {
        let records = InteractionRecords()
        let interaction = Interaction(name: "open_task", scenario: "cached",
            input: InteractionInput(uptime: 100, source: .appKitEvent),
            context: Interaction.Context(thermalState: 0, liveSessions: 3), sink: records.append)
        interaction.acknowledged(at: 100.1, evidence: .presentedFrame)
        interaction.work.cliRead()
        interaction.work.project()
        interaction.work.sql(statements: 12, rows: 25)
        interaction.usable(at: 101.2, evidence: .presentedFrame)
        interaction.finish(.completed)
        let events = records.values
        #expect(events.map(\.phase) == [.input, .acknowledged, .usable, .outcome])
        #expect(abs(events[1].ms! - 100) < 0.00001)
        #expect(abs(events[2].ms! - 1200) < 0.00001)
        #expect(events[3].outcome == .completed)
        #expect(events[3].work.cliReads == 1)
        #expect(events[3].work.projections == 1)
        #expect(events[3].work.sqlStatements == 12)
        #expect(events[3].work.sqlRows == 25)
    }

    @Test @MainActor
    func stateCallbacksCannotBecomePresentedUsability() {
        let records = InteractionRecords()
        let interaction = Interaction(name: "find_work", scenario: "filter",
            input: InteractionInput(uptime: 100, source: .appKitEvent),
            context: Interaction.Context(thermalState: 2, liveSessions: nil), sink: records.append)
        interaction.acknowledged(at: 100.01, evidence: .appState)
        interaction.usable(at: 100.02, evidence: .renderCommit)
        interaction.finish(.completed)
        interaction.finish(.failed)
        #expect(records.values.last?.outcome == .unmeasured)
        #expect(records.values.count == 4)
        #expect(records.values.last?.work.sqlStatements == nil)
    }

    @Test @MainActor
    func presentationCanConfirmAnEarlierStateObservation() {
        let records = InteractionRecords()
        let interaction = Interaction(name: "open_task", scenario: "cached",
            input: InteractionInput(uptime: 100, source: .appKitEvent),
            context: Interaction.Context(thermalState: 0, liveSessions: 1), sink: records.append)
        interaction.acknowledged(at: 100.01, evidence: .appState)
        interaction.acknowledged(at: 100.02, evidence: .capturedFrame)
        interaction.usable(at: 100.03, evidence: .appState)
        interaction.usable(at: 100.04, evidence: .capturedFrame)
        interaction.finish(.completed)
        #expect(records.values.last?.outcome == .completed)
        #expect(records.values.filter { $0.phase == .acknowledged }.count == 2)
    }

    @Test @MainActor
    func missingInputAndSupersededWorkDoNotPass() {
        let records = InteractionRecords()
        let interaction = Interaction(name: "open_task", scenario: "programmatic", input: nil,
            context: Interaction.Context(thermalState: 0, liveSessions: nil), sink: records.append)
        interaction.acknowledged(at: 100, evidence: .presentedFrame)
        interaction.usable(at: 101, evidence: .presentedFrame)
        interaction.finish(.completed)
        #expect(records.values.allSatisfy { $0.ms == nil })
        #expect(records.values.last?.outcome == .unmeasured)
        let other = Interaction(name: "find_work", scenario: "filter",
            input: InteractionInput(uptime: 100, source: .appKitEvent),
            context: Interaction.Context(thermalState: 0, liveSessions: nil), sink: records.append)
        other.finish(.superseded)
        other.usable(at: 102, evidence: .capturedFrame)
        #expect(records.values.last?.outcome == .superseded)
    }

    @Test @MainActor
    func bothPlatformsUseDailyRecordsWithBuildAndLoadContext() throws {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: directory) }
        let journal = InteractionJournal(directory: directory,
            identity: ["platform": "iOS", "app": "1.0", "revision": "candidate"])
        let interaction = Interaction(name: "return_session", scenario: "resume",
            input: InteractionInput(uptime: 100, source: .uiKitEvent),
            context: Interaction.Context(thermalState: 2, liveSessions: 7), sink: journal.append)
        interaction.finish(.failed)
        journal.flush()
        let files = try FileManager.default.contentsOfDirectory(at: directory, includingPropertiesForKeys: nil)
        #expect(files.count == 1)
        let rows = try String(contentsOf: files[0], encoding: .utf8).split(separator: "\n").map {
            try JSONSerialization.jsonObject(with: Data($0.utf8)) as! [String: Any]
        }
        #expect((rows[0]["build"] as? [String: String])?["revision"] == "candidate")
        #expect((rows[0]["context"] as? [String: Any])?["thermalState"] as? Int == 2)
        #expect((rows[0]["context"] as? [String: Any])?["liveSessions"] as? Int == 7)
        #expect(rows[1]["outcome"] as? String == "failed")
    }
}
