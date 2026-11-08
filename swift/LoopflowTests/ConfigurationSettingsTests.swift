#if os(macOS)
import Foundation
import Loopflow
import Testing
@testable import LoopflowMac

@Suite("Configuration destinations")
struct ConfigurationSettingsTests {
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
