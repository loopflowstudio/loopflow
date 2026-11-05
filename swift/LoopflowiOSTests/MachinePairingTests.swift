import Foundation
import Loopflow
import SwiftUI
import UIKit
import ViewInspector
import XCTest
@testable import LoopflowiOS

@MainActor
final class MachinePairingTests: XCTestCase {
    func testLocalMobileSetupCodecMatchesTheBackendFixture() throws {
        let bundle = Bundle(for: Self.self)
        let requestURL = try XCTUnwrap(bundle.url(forResource: "configure_remote", withExtension: "json", subdirectory: "server"))
        let configURL = try XCTUnwrap(bundle.url(forResource: "mobile_configuration", withExtension: "json", subdirectory: "server"))
        let config = try JSONDecoder().decode(MachineMobileConfiguration.self, from: Data(contentsOf: configURL))
        let request = ServerClientMessage.request(id: "mobile-1", operation: .configureRemote(configuration: config))
        XCTAssertEqual(try JSONDecoder().decode(JSONValue.self, from: JSONEncoder().encode(request)),
                       try JSONDecoder().decode(JSONValue.self, from: Data(contentsOf: requestURL)))
        let replyURL = try XCTUnwrap(bundle.url(forResource: "remote", withExtension: "json", subdirectory: "server"))
        guard case .response("mobile-1", .remote(let configuration)) = try JSONDecoder().decode(ServerMessage.self, from: Data(contentsOf: replyURL)) else {
            return XCTFail("Expected listener configuration")
        }
        XCTAssertEqual(configuration?.endpoint, "https://host.tail.ts.net:8443")
    }
    func testAddressPreservesThePrivateEndpointAndRejectsOtherDestinations() throws {
        let endpoint = try MachineAddress.decode(" machine.example.ts.net:8443/ ")
        let url = try MachineConnection.socketURL(endpoint: endpoint, path: "/v1/machine")
        XCTAssertEqual(url.absoluteString, "wss://machine.example.ts.net:8443/v1/machine")
        for invalid in ["http://machine.example.ts.net", "https://example.com",
                        "https://machine.example.ts.net.attacker.com", "https://user@machine.example.ts.net",
                        "https://machine.example.ts.net/path", "https://machine.example.ts.net?token=secret",
                        "https://machine.example.ts.net#fragment", "", "{\"ticket\":\"old\"}",
                        "https://machine.example.ts.net:0", "https://machine.example.ts.net:65536",
                        String(repeating: "a", count: 2049)] {
            XCTAssertThrowsError(try MachineAddress.decode(invalid), invalid)
        }
    }

    func testChangeAddressKeepsTheSavedConnectionAndOffersQRFirst() async throws {
        let previous = try MachineKeychain.load()
        let saved = SavedMachine(endpoint: try MachineAddress.decode("macbook.example.ts.net:8443"), machineID: "original-machine")
        let form = MachineAddressView(replacing: saved) { _ in XCTFail("Opening the form cannot change the connection") }
        XCTAssertNoThrow(try form.inspect().find(text: saved.endpoint.absoluteString))
        XCTAssertNoThrow(try form.inspect().find(button: "Scan QR code"))
        XCTAssertNoThrow(try form.inspect().find(button: "Enter address"))
        XCTAssertThrowsError(try form.inspect().find(button: "Update address"), "Verify an entered address before offering to connect")
        for (name, style, size) in [("light", ColorScheme.light, DynamicTypeSize.large),
                                    ("dark", .dark, .large), ("large-text", .light, .accessibility3)] {
            let view = form.environment(\.colorScheme, style).environment(\.dynamicTypeSize, size)
            let controller = UIHostingController(rootView: view)
            let scene = try XCTUnwrap(UIApplication.shared.connectedScenes.compactMap { $0 as? UIWindowScene }.first)
            let previousWindow = scene.keyWindow
            let window = UIWindow(windowScene: scene)
            window.frame = CGRect(x: 0, y: 0, width: 390, height: 844)
            window.rootViewController = controller
            window.makeKeyAndVisible()
            controller.view.frame = window.bounds
            controller.view.layoutIfNeeded()
            try await Task.sleep(for: .milliseconds(250))
            let image = UIGraphicsImageRenderer(bounds: window.bounds).image { _ in
                XCTAssertTrue(window.drawHierarchy(in: window.bounds, afterScreenUpdates: true))
            }
            let attachment = XCTAttachment(image: image)
            attachment.name = "change-machine-address-\(name)"
            attachment.lifetime = .keepAlways
            add(attachment)
            window.isHidden = true
            previousWindow?.makeKey()
        }
        XCTAssertEqual(try MachineKeychain.load()?.endpoint, previous?.endpoint)
        XCTAssertEqual(try MachineKeychain.load()?.machineID, previous?.machineID)
    }

    func testKeychainPreservesTheVerifiedMachineWithoutACredential() throws {
        let previous = try MachineKeychain.load()
        defer {
            do {
                if let previous { try MachineKeychain.save(previous) }
                else { try MachineKeychain.forget() }
            } catch { XCTFail("Could not restore the simulator's previous Machine connection") }
        }
        let saved = SavedMachine(endpoint: try MachineAddress.decode("machine.example.ts.net"), machineID: "test-machine")
        try MachineKeychain.save(saved)
        XCTAssertEqual(try MachineKeychain.load()?.machineID, "test-machine")
        XCTAssertEqual(try MachineKeychain.load()?.endpoint, saved.endpoint)
        let fields = try XCTUnwrap(JSONSerialization.jsonObject(with: JSONEncoder().encode(saved)) as? [String: Any])
        XCTAssertEqual(Set(fields.keys), ["endpoint", "machineID"])
        try MachineKeychain.forget()
        XCTAssertNil(try MachineKeychain.load())
    }
}
