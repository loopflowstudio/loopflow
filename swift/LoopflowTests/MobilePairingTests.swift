import AppKit
import Loopflow
import Testing
import SwiftUI
import ViewInspector
import Vision
@testable import LoopflowMac

@MainActor
struct MobilePairingTests {
    @Test func settingsDecodeTheMachinesConfiguration() throws {
        let fixtures = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
            .deletingLastPathComponent().deletingLastPathComponent()
            .appendingPathComponent("rust/loopflow-execution/tests/fixtures/dto/server")
        let reply = try JSONDecoder().decode(JSONValue.self,
            from: Data(contentsOf: fixtures.appendingPathComponent("remote.json")))
        let config = try JSONDecoder().decode(MobileSetup.Configuration.self,
            from: JSONEncoder().encode(reply["result"]["configuration"]))
        #expect(config.enabled && config.owner.node == "node-host")
    }

    @Test func sidebarAndPairingCodeFollowTheSameFreshStatus() async throws {
        let address = "https://machine.example.ts.net:8443"
        var available = true
        let setup = MobileSetup { _ in
            guard available else { throw RegistryQueryError("Machine unavailable") }
            return #"{"ready":true,"error":null,"configuration":{"enabled":true,"endpoint":"\#(address)","owner":{"node":"node-host","user":1}}}"#
        }
        let window = SettingsWindow(navigation: .constant(SettingsNavigation(pane: .mobile)),
                                    appearance: AppearanceStore(paths: .temporary(), systemDark: false),
                                    checkout: nil, portfolio: PortfolioService(), mobile: setup)
        await setup.refresh()
        _ = try window.inspect().find(viewWithAccessibilityIdentifier: "mobile-ready")
        _ = try window.inspect().find(viewWithAccessibilityLabel: "Machine address QR code")
        _ = try window.inspect().find(text: "Ready to connect")
        available = false
        await setup.refresh()
        #expect((try? window.inspect().find(viewWithAccessibilityIdentifier: "mobile-ready")) == nil)
        #expect((try? window.inspect().find(viewWithAccessibilityLabel: "Machine address QR code")) == nil)
        _ = try window.inspect().find(text: "Mobile connection unavailable")
        _ = try window.inspect().find(text: address)
    }

    @Test func desktopCodeContainsOnlyTheMachineAddress() throws {
        let address = "https://machine.example.ts.net:8443"
        let image = try MobileSetup.qrCode(Data(address.utf8))
        let cgImage = try #require(image.cgImage(forProposedRect: nil, context: nil, hints: nil))
        let request = VNDetectBarcodesRequest()
        request.symbologies = [.qr]
        try VNImageRequestHandler(cgImage: cgImage).perform([request])
        let text = try #require(request.results?.first?.payloadStringValue)
        #expect(text == address)
        #expect(try MachineAddress.decode(text).absoluteString == address)
    }
}
