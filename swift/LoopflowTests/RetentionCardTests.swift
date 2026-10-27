import Foundation
import SwiftUI
import Testing
import ViewInspector
@testable import Loopflow
@testable import LoopflowMac

@MainActor
@Suite("Retention card")
struct RetentionCardTests {
    @Test func fixtureRoundTripsTheRustWireShape() throws {
        let data = try loadFixtureData("retention_summary.json")
        let summary = try JSONDecoder().decode(RetentionSummary.self, from: data)
        #expect(summary.groups[1].state == .facts)
        #expect(summary.groups[1].age == .underQuarter)
        #expect(summary.homes[1].policy == .kept)
        #expect(summary.harnesses[1].nextExpiry == nil)
        #expect(try JSONDecoder().decode(RetentionSummary.self, from: JSONEncoder().encode(summary)) == summary)
        var wire = try #require(JSONSerialization.jsonObject(with: data) as? [String: Any])
        wire.removeValue(forKey: "homes")
        #expect(throws: DecodingError.self) {
            try JSONDecoder().decode(RetentionSummary.self, from: JSONSerialization.data(withJSONObject: wire))
        }
    }

    @Test func cardShowsWhatIsKeptWhatExpiresAndHowToKeepMore() throws {
        let summary = try JSONDecoder().decode(
            RetentionSummary.self, from: loadFixtureData("retention_summary.json"))
        let card = RetentionCard(summary: summary, failure: nil)
        let texts = try card.inspect().findAll(ViewType.Text.self).map { try $0.string() }
        #expect(texts.contains("claude  1 readable · 2 facts only · next removal after 2026-11-15"))
        #expect(texts.contains("codex  1 readable · 0 facts only · no provider expiry"))
        #expect(texts.contains { $0.hasPrefix("after 2026-11-15  claude  Fix the parser") })
        #expect(texts.contains { $0.contains("Raise cleanupPeriodDays") })
    }

    @Test func cardNamesAnUnavailableSummary() throws {
        let card = RetentionCard(summary: nil, failure: "lf history retention failed")
        let texts = try card.inspect().findAll(ViewType.Text.self).map { try $0.string() }
        #expect(texts.contains("lf history retention failed"))
    }
}
