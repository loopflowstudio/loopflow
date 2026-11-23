#if os(macOS)
import Foundation
import Loopflow
import Testing
@testable import LoopflowMac

@Suite("Configuration destinations")
struct ConfigurationSettingsTests {
    @Test func checkoutScopeAndCheckedEditShareTheRustWireShape() throws {
        let root = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
        let fixture = try JSONDecoder().decode([String: JSONValue].self, from:
            Data(contentsOf: root.appending(path: "tests/fixtures/dto/configuration_scope.json")))
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
        let receipt = try JSONDecoder().decode(ConfigurationEditReceipt.self, from: JSONEncoder().encode(fixture["receipt"]))
        #expect(try json(receipt) == fixture["receipt"])
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
        let root = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
        let fixture = try Data(contentsOf: root.appendingPathComponent("tests/fixtures/dto/configuration_catalog.json"))
        let catalogue = try JSONDecoder().decode([ConfigurationDefinition].self, from: fixture)
        #expect(catalogue.count == 1)
        #expect(catalogue[0].key == "theme")
        #expect(catalogue[0].category == "Desktop")
    }

    @Test func destinationsMatchRustWithoutIndependentScopeOrPath() throws {
        let root = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
        let fixture = try Data(contentsOf: root.appendingPathComponent("tests/fixtures/dto/configuration_destinations.json"))
        let expected = try JSONDecoder().decode([JSONValue].self, from: fixture)
        let actual = ConfigurationDestination.allCases.map {
            $0.address(repository: "/project/task", wave: "product")
        }
        #expect(actual == expected)
        #expect(ConfigurationDestination.personalGlobal.address(repository: "/stale", wave: "stale")
            == .object(["kind": .string("personal_global")]))
        #expect(ConfigurationDestination.personalRepository.address(repository: "/project", wave: "stale")
            == .object(["kind": .string("personal_repo"), "repo": .string("/project")]))
    }
}
#endif
