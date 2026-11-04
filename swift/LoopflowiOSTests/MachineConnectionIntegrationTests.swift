import Foundation
import XCTest
@testable import LoopflowiOS

/// Opt-in acceptance against an isolated, empty Machine registry through host
/// Tailscale Serve. The runner transfers a one-use ticket directly to Documents;
/// neither the test nor its assertions print ticket or credential contents.
@MainActor
final class MachineConnectionIntegrationTests: XCTestCase {
    func testPairAndReadThroughHostTailscale() async throws {
        let setup = URL.documentsDirectory.appending(path: "LoopflowIntegration/setup.json")
        guard FileManager.default.fileExists(atPath: setup.path) else {
            throw XCTSkip("No isolated Machine setup ticket supplied")
        }
        let data = try Data(contentsOf: setup)
        try FileManager.default.removeItem(at: setup)
        let ticket = try PairingTicket.decode(data)
        let previous = try MachineKeychain.load()
        defer {
            do {
                if let previous { try MachineKeychain.save(previous) }
                else { try MachineKeychain.forget() }
            } catch { XCTFail("Could not restore the simulator's previous Machine credential") }
        }
        let credential = try await MachinePairing.redeem(ticket)
        XCTAssertNil(credential.machineID)
        let client = RemoteMobileClient(credential: try XCTUnwrap(MachineKeychain.load()))
        let tasks = try await client.workingTasks()
        XCTAssertTrue(tasks.isEmpty, "This acceptance run requires an isolated empty registry")
        let confirmed = try XCTUnwrap(MachineKeychain.load())
        XCTAssertNotNil(confirmed.machineID)
        XCTAssertTrue(confirmed.token == credential.token, "Bootstrap must preserve the issued grant")
        let reopened = RemoteMobileClient(credential: confirmed)
        let refreshed = try await reopened.workingTasks()
        XCTAssertTrue(refreshed.isEmpty)
    }
}
