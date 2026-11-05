import AppKit
import Loopflow
import Testing
import Vision
@testable import LoopflowMac

@MainActor
struct MobilePairingTests {
    @Test func settingsDecodeTheMachinesConfiguration() throws {
        let fixtures = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
            .deletingLastPathComponent().deletingLastPathComponent()
            .appendingPathComponent("tests/fixtures/dto/server")
        let config = try JSONDecoder().decode(MobileSetup.Configuration.self,
            from: Data(contentsOf: fixtures.appendingPathComponent("mobile_configuration.json")))
        #expect(config.enabled && config.owner.node == "node-host")
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
