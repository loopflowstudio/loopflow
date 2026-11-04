import AppKit
import Loopflow
import Testing
import Vision
@testable import LoopflowMac

@MainActor
struct MobilePairingTests {
    @Test func settingsDecodeTheMachinesConfigurationAndDeviceList() throws {
        let fixtures = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
            .deletingLastPathComponent().deletingLastPathComponent()
            .appendingPathComponent("tests/fixtures/dto/machine")
        let config = try JSONDecoder().decode(MobileSetup.Configuration.self,
            from: Data(contentsOf: fixtures.appendingPathComponent("mobile_configuration.json")))
        #expect(config.enabled && config.port == 49250)
        let devices = try JSONDecoder().decode([MobileSetup.Device].self,
            from: Data(contentsOf: fixtures.appendingPathComponent("devices.json")))
        #expect(devices.count == 1 && devices[0].name == "My iPhone" && devices[0].revokedAt == nil)
    }

    @Test func desktopCodeScansAsTheSamePrivateMachineTicket() throws {
        // A deterministic, non-credential payload exercises the actual QR image.
        let data = try JSONSerialization.data(withJSONObject: [
            "version": 1, "endpoint": "https://machine.example.ts.net:8443",
            "ticket": String(repeating: "a", count: 64),
        ])
        let image = try MobileSetup.qrCode(data)
        let cgImage = try #require(image.cgImage(forProposedRect: nil, context: nil, hints: nil))
        let request = VNDetectBarcodesRequest()
        request.symbologies = [.qr]
        try VNImageRequestHandler(cgImage: cgImage).perform([request])
        let text = try #require(request.results?.first?.payloadStringValue)
        let ticket = try MachinePairingTicket.decode(Data(text.utf8))
        #expect(ticket.endpoint.absoluteString == "https://machine.example.ts.net:8443")
        #expect(ticket.ticket == String(repeating: "a", count: 64))
        #expect(try MachinePairingTicket.socketURL(endpoint: ticket.endpoint, path: "/v1/pair").query == nil)
    }
}
