#if os(macOS)
import Foundation
import Testing
@testable import LoopflowMac

@Suite("Configuration destinations")
struct ConfigurationSettingsTests {
    @Test func destinationsMatchRustWithoutIndependentScopeOrPath() throws {
        let root = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
        let fixture = try Data(contentsOf: root.appendingPathComponent("tests/fixtures/dto/configuration_destinations.json"))
        let expected = try JSONSerialization.jsonObject(with: fixture) as! [[String: String]]
        let actual = ConfigurationDestination.allCases.map {
            $0.address(repository: "/project/task", wave: "product") as! [String: String]
        }
        #expect(actual == expected)
        #expect(ConfigurationDestination.personalGlobal.address(repository: "/stale", wave: "stale").count == 1)
        #expect(ConfigurationDestination.personalRepository.address(repository: "/project", wave: "stale")["wave"] == nil)
    }
}
#endif
