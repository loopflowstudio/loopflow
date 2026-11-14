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
            .appendingPathComponent("rust/loopflow/tests/fixtures/dto/server")
        let reply = try JSONDecoder().decode(JSONValue.self,
            from: Data(contentsOf: fixtures.appendingPathComponent("remote.json")))
        let config = try JSONDecoder().decode(MobileSetup.Configuration.self,
            from: JSONEncoder().encode(reply["result"]["configuration"]))
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
