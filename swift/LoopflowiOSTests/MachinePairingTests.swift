import Foundation
import Loopflow
import XCTest
@testable import LoopflowiOS

@MainActor
final class MachinePairingTests: XCTestCase {
    func testLocalMobileSetupCodecMatchesTheBackendFixture() throws {
        let bundle = Bundle(for: Self.self)
        let requestURL = try XCTUnwrap(bundle.url(forResource: "configure_remote", withExtension: "json", subdirectory: "machine"))
        let configURL = try XCTUnwrap(bundle.url(forResource: "mobile_configuration", withExtension: "json", subdirectory: "machine"))
        let config = try JSONDecoder().decode(MachineMobileConfiguration.self, from: Data(contentsOf: configURL))
        let request = MachineRequest(version: 1, id: "mobile-1", operation: .configureRemote(configuration: config))
        XCTAssertEqual(try JSONDecoder().decode(JSONValue.self, from: JSONEncoder().encode(request)),
                       try JSONDecoder().decode(JSONValue.self, from: Data(contentsOf: requestURL)))
        let replyURL = try XCTUnwrap(bundle.url(forResource: "remote", withExtension: "json", subdirectory: "machine"))
        let response = try JSONDecoder().decode(MachineResponse.self, from: Data(contentsOf: replyURL))
        guard case .remote(let configuration) = response.result else { return XCTFail("Expected listener configuration") }
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
