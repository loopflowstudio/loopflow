import Foundation
import Loopflow
import XCTest
@testable import LoopflowiOS

@MainActor
final class MachinePairingTests: XCTestCase {
    func testLocalMobileSetupCodecMatchesTheBackendFixture() throws {
        let bundle = Bundle(for: Self.self)
        let requestURL = try XCTUnwrap(bundle.url(forResource: "configure_remote", withExtension: "json", subdirectory: "machine"))
        let request = MachineRequest(version: 1, id: "mobile-1", operation: .configureRemote(port: 0))
        XCTAssertEqual(try JSONDecoder().decode(JSONValue.self, from: JSONEncoder().encode(request)),
                       try JSONDecoder().decode(JSONValue.self, from: Data(contentsOf: requestURL)))
        let replyURL = try XCTUnwrap(bundle.url(forResource: "remote", withExtension: "json", subdirectory: "machine"))
        let response = try JSONDecoder().decode(MachineResponse.self, from: Data(contentsOf: replyURL))
        guard case .remote(let port) = response.result else { return XCTFail("Expected listener port") }
        XCTAssertEqual(port, 49250)
    }
    func testTicketPreservesThePrivateEndpointAndRejectsOtherDestinations() throws {
        let token = String(repeating: "a", count: 64)
        let ticket = try PairingTicket.decode(data(endpoint: "https://machine.example.ts.net:8443", token: token))
        let url = try MachineConnection.socketURL(endpoint: ticket.endpoint, path: "/v1/pair")
        XCTAssertEqual(url.absoluteString, "wss://machine.example.ts.net:8443/v1/pair")
        XCTAssertNil(URLComponents(url: url, resolvingAgainstBaseURL: false)?.query)
        for endpoint in ["http://machine.example.ts.net", "https://example.com",
                         "https://machine.example.ts.net.attacker.com", "https://user@machine.example.ts.net",
                         "https://machine.example.ts.net/path", "https://machine.example.ts.net?token=secret",
                         "https://machine.example.ts.net#fragment"] {
            XCTAssertThrowsError(try PairingTicket.decode(data(endpoint: endpoint, token: token)))
        }
    }

    func testTicketsRejectUnsupportedVersionsMissingFieldsAndOversizedImports() throws {
        XCTAssertThrowsError(try PairingTicket.decode(data(version: 2)))
        XCTAssertThrowsError(try PairingTicket.decode(data(token: "too-short")))
        XCTAssertThrowsError(try PairingTicket.decode(data(token: String(repeating: "z", count: 64))))
        XCTAssertThrowsError(try PairingTicket.decode(Data(#"{"version":1}"#.utf8)))
        XCTAssertThrowsError(try PairingTicket.decode(Data(repeating: 32, count: 4097)))
    }

    func testKeychainPreservesNewGrantThenConfirmedMachineIdentity() throws {
        let previous = try MachineKeychain.load()
        defer {
            do {
                if let previous { try MachineKeychain.save(previous) }
                else { try MachineKeychain.forget() }
            } catch { XCTFail("Could not restore the simulator's previous Machine credential") }
        }
        let endpoint = try XCTUnwrap(URL(string: "https://machine.example.ts.net"))
        let pending = MachineCredential(endpoint: endpoint, deviceID: "test-device", name: "Test phone",
            token: String(repeating: "a", count: 64), machineID: nil)
        try MachineKeychain.save(pending)
        let saved = try XCTUnwrap(MachineKeychain.load())
        XCTAssertNil(saved.machineID)
        XCTAssertTrue(saved.token == pending.token, "The issued grant must survive before bootstrap")
        try MachineKeychain.save(MachineCredential(endpoint: endpoint, deviceID: saved.deviceID,
            name: saved.name, token: saved.token, machineID: "test-machine"))
        XCTAssertEqual(try MachineKeychain.load()?.machineID, "test-machine")
        try MachineKeychain.forget()
        XCTAssertNil(try MachineKeychain.load())
    }

    private func data(version: Int = 1, endpoint: String = "https://machine.example.ts.net",
                      token: String = String(repeating: "a", count: 64)) throws -> Data {
        try JSONSerialization.data(withJSONObject: ["version": version, "endpoint": endpoint, "ticket": token])
    }
}
