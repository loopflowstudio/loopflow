import Foundation
import Loopflow
import XCTest
@testable import LoopflowiOS

@MainActor
final class MachineThemeTests: XCTestCase {
    private let machineID = "machine-theme-\(UUID().uuidString)"

    override func tearDown() {
        UserDefaults.standard.removeObject(forKey: "theme-\(machineID)")
    }

    func testEveryBuiltinThemeDecodesOnThePhone() {
        XCTAssertEqual(ThemeDefinition.builtins.count, 26)
        XCTAssertEqual(ThemeDefinition.builtins.filter(\.isDark).count, 13)
    }

    func testTheMachinesPairDrawsEachSideAndIsRememberedForTheNextLaunch() throws {
        let answer = RawJSON(Data(#"{"path":"/m/.lf/config.yaml","key":"theme","value":"light:Fig Leaf,dark:Fig","lifecycle":"stable"}"#.utf8))
        let theme = MachineTheme()
        theme.show(machineID: machineID)
        XCTAssertEqual(theme.theme(dark: true).name, "Loopflow Dark")
        theme.remember(try MachineTheme.declared(in: answer), machineID: machineID)
        XCTAssertEqual(theme.theme(dark: true).name, "Fig")
        XCTAssertEqual(theme.theme(dark: false).name, "Fig Leaf")

        let relaunched = MachineTheme()
        relaunched.show(machineID: machineID)
        XCTAssertEqual(relaunched.theme(dark: false).name, "Fig Leaf")
        relaunched.show(machineID: nil)
        XCTAssertEqual(relaunched.theme(dark: false).name, "Loopflow Light")
    }

    func testOneNameIsThatThemeInEitherAppearanceAndUnknownNamesDrawLoopflow() {
        let theme = MachineTheme()
        theme.show(machineID: machineID)
        theme.remember("Fig", machineID: machineID)
        XCTAssertEqual(theme.theme(dark: false).name, "Fig")
        theme.remember("light:My Own,dark:Fig", machineID: machineID)
        XCTAssertEqual(theme.theme(dark: false).name, "Loopflow Light")
        XCTAssertEqual(theme.theme(dark: true).name, "Fig")
        theme.remember(nil, machineID: machineID)
        XCTAssertEqual(theme.theme(dark: true).name, "Loopflow Dark")
    }
}
