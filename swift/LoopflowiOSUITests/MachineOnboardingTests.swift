import XCTest

@MainActor
final class MachineOnboardingTests: XCTestCase {
    func testAddressEntryValidatesBeforeOfferingAConnection() throws {
        let app = XCUIApplication()
        app.launchArguments = []
        app.launch()
        defer { app.terminate() }
        // Run in a fresh Simulator; do not forget another installation's Machine.
        XCTAssertTrue(app.buttons["Scan QR code"].waitForExistence(timeout: 10))
        capture("machine-qr-first")
        app.buttons["Scan QR code"].tap()
        let fallback = app.buttons["Enter an address instead"]
        XCTAssertTrue(fallback.waitForExistence(timeout: 5))
        fallback.tap()
        let address = app.textFields["Machine address"]
        XCTAssertTrue(address.waitForExistence(timeout: 5))
        address.tap()
        address.typeText("example.com")
        let continueAddress = app.buttons["machine-address-continue"]
        XCTAssertTrue(continueAddress.isHittable)
        capture("machine-address-keyboard")
        continueAddress.tap()
        let issue = app.staticTexts["Connection issue: Enter the Tailscale address shown in Desktop’s Mobile settings."]
        XCTAssertTrue(issue.waitForExistence(timeout: 5))
        XCTAssertTrue(issue.isHittable)
        capture("machine-address-validation")
        XCTAssertFalse(app.buttons["Connect Machine"].exists)
        address.tap()
        clear(address)
        address.typeText("machine.example.ts.net:8443")
        continueAddress.tap()
        XCTAssertTrue(app.buttons["Connect Machine"].waitForExistence(timeout: 5))
        XCTAssertTrue(app.staticTexts["machine.example.ts.net"].exists)
        XCTAssertFalse(address.exists)
        capture("machine-address-confirmation")

        app.buttons["Enter address"].tap()
        XCTAssertTrue(address.waitForExistence(timeout: 5))
        XCTAssertFalse(app.buttons["Connect Machine"].exists,
                       "Editing the address must not leave a button that connects to the previous address")
        address.tap()
        clear(address)
        address.typeText("other.example.ts.net:9443")
        continueAddress.tap()
        XCTAssertTrue(app.buttons["Connect Machine"].waitForExistence(timeout: 5))
        XCTAssertTrue(app.staticTexts["other.example.ts.net"].exists)
        XCTAssertFalse(app.staticTexts["machine.example.ts.net"].exists)
        capture("machine-corrected-address")
    }

    private func clear(_ field: XCUIElement) {
        field.typeKey("a", modifierFlags: .command)
        field.typeKey(XCUIKeyboardKey.delete.rawValue, modifierFlags: [])
    }

    private func capture(_ name: String) {
        let attachment = XCTAttachment(screenshot: XCUIScreen.main.screenshot())
        attachment.name = name
        attachment.lifetime = .keepAlways
        add(attachment)
    }
}
