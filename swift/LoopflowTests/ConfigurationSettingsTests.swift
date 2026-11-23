#if os(macOS)
import Foundation
import Loopflow
import Testing
@testable import LoopflowMac

@Suite("Configuration destinations")
struct ConfigurationSettingsTests {
    @Test func checkoutScopeAndCheckedEditShareTheRustWireShape() throws {
        let fixture = try JSONDecoder().decode([String: JSONValue].self, from:
            loadFixtureData("configuration_scope.json"))
        func json(_ value: some Encodable) throws -> JSONValue {
            try JSONDecoder().decode(JSONValue.self, from: JSONEncoder().encode(value))
        }
        let scope = try JSONDecoder().decode(ConfigurationScope.self, from: JSONEncoder().encode(fixture["scope"]))
        #expect(try json(scope) == fixture["scope"])
        let mine = try #require(scope.files.first { $0.owner == .personalRepository })
        #expect(try json(ServedRead.configScope(directory: "/project", wave: nil, target: .general)) == fixture["read"])
        #expect(try json(ConfigOperation.edit(destination: mine.destination, key: "context.diff", expected: try #require(mine.slots["context.diff"]).entries, change: .reset)) == fixture["write"])
        #expect(try json(ConfigOperation.preview(destination: mine.destination, key: "context.diff", expected: try #require(mine.slots["context.diff"]).entries, change: .reset)) == fixture["preview"])
        let result = try JSONDecoder().decode(MoveResult.self, from: JSONEncoder().encode(fixture["move_result"]))
        guard case .preview(let preview) = result else { Issue.record("Expected a move preview"); return }
        #expect(try json(ConfigOperation.promote(destination: mine.destination, key: "context.diff", wave: nil, action: .confirm(preview))) == fixture["promote"])
        #expect(try json(ConfigOperation.promote(destination: mine.destination, key: "context.diff", wave: nil, action: .dismiss)) == fixture["dismiss"])
        let dismissed = try JSONDecoder().decode(MoveResult.self, from: JSONEncoder().encode(fixture["dismissed"]))
        guard case .dismissed = dismissed else { Issue.record("Expected a dismissed move"); return }
        let receipt = try JSONDecoder().decode(ConfigurationEditReceipt.self, from: JSONEncoder().encode(fixture["receipt"]))
        #expect(try json(receipt) == fixture["receipt"])
    }

    @Test func nullableDeclarationsKeepAbsentNullAndFalseDistinct() throws {
        for (json, expected) in [
            (#"{"kind":"absent"}"#, ConfigurationScope.Authored.absent),
            (#"{"kind":"present","value":null}"#, .present(.null)),
            (#"{"kind":"present","value":false}"#, .present(.bool(false)))
        ] {
            let decoded = try JSONDecoder().decode(ConfigurationScope.Authored.self, from: Data(json.utf8))
            #expect(decoded == expected)
        }
        #expect(throws: DecodingError.self) {
            try JSONDecoder().decode(ConfigurationScope.Authored.self, from: Data(#"{"kind":"present"}"#.utf8))
        }
    }

    @MainActor @Test func dismissIsOfferedOnlyWhenResumingDivergedFiles() {
        let conflict = ServerError(code: "conflict", message: "Declaration changed", data: .null)
        let offline = ServerError(code: "unavailable", message: "Machine offline", data: .null)
        #expect(ScopedSettingsModel.RowError(conflict, move: .resume).recovery == .dismissMove)
        #expect(ScopedSettingsModel.RowError(conflict).recovery == .refresh)
        #expect(ScopedSettingsModel.RowError(offline, move: .resume).recovery == nil)
    }

    @Test func rowsSummarizeMapsAndListsInWords() {
        #expect(JSONValue.object(["name": .null]).settingsText == "None")
        #expect(JSONValue.object(["linear_requests": .bool(false)]).settingsText == "Linear Requests: Off")
        #expect(JSONValue.object([:]).settingsText == "Empty")
        #expect(JSONValue.array([]).settingsText == "Empty list")
        #expect(JSONValue.null.settingsText == "None")
        #expect(JSONValue.array([.string("README.md"), .string("docs/")]).settingsText == "README.md, docs/")
        #expect(JSONValue.object(["groups": .array([.string("a")])]).settingsText == #"{"groups":["a"]}"#)
    }

    @Test func catalogueIncludesFeatureCategory() throws {
        let fixture = try loadFixtureData("configuration_catalog.json")
        let catalogue = try JSONDecoder().decode([ConfigurationDefinition].self, from: fixture)
        #expect(catalogue.map(\.key) == ["theme", "agent", "default_agent"])
        #expect(catalogue.map(\.unset) == [nil, "Use starting preference", "First in fallback order"])
        #expect(catalogue[0].category == "Desktop")
    }

    @Test func destinationKindsMatchTheRustFiles() throws {
        let files = try JSONDecoder().decode([JSONValue].self, from: loadFixtureData("configuration_destinations.json"))
        let owners = files.map { ConfigurationScope.File(destination: $0, path: "", slots: [:]).owner }
        #expect(owners == [.personalGlobal, .personalRepository, .sharedRepository, .personalWave, .sharedWave])
    }
}
#endif
