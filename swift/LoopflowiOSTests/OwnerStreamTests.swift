import Foundation
import Loopflow
import XCTest
@testable import LoopflowiOS

final class OwnerStreamTests: XCTestCase {
    func testHeartbeatUsesOwnerProtocolWithoutTakingControlOrConsumingAttachmentReply() throws {
        var stream = try observed()
        let request = try stream.heartbeatRequest(id: 2)
        XCTAssertEqual(request.first, 0x11)
        XCTAssertEqual(try JSONDecoder().decode(JSONValue.self, from: Data(request.dropFirst(5))),
                       .object(["id": .integer(2), "kind": .string("controlled")]))
        stream.expectAttachment(3)
        let reply = Data(#"{"answers":2,"seq":2,"kind":"controlled","held":true}"#.utf8)
        XCTAssertTrue(try stream.receive(OwnerStream.frame(tag: 1, body: reply)).isEmpty)
        XCTAssertTrue(stream.awaitingAttachment)
        XCTAssertTrue(stream.ready)
        XCTAssertFalse(stream.inputControl, "Controlled reports whether anyone holds control, not this viewer")
        XCTAssertFalse(stream.geometryControl)
        XCTAssertNoThrow(try stream.heartbeatRequest(id: 4))
    }

    func testUnansweredHeartbeatRequiresReconnect() throws {
        var stream = try observed()
        _ = try stream.heartbeatRequest(id: 2)
        XCTAssertThrowsError(try stream.heartbeatRequest(id: 3))
        let unexpected = Data(#"{"answers":2,"seq":2,"kind":"accepted"}"#.utf8)
        XCTAssertThrowsError(try stream.receive(OwnerStream.frame(tag: 1, body: unexpected)))
    }

    func testRemoteDirectoryAdvancesTheStreamWithoutChangingLocalState() throws {
        let url = try XCTUnwrap(Bundle(for: Self.self).url(forResource: "directory", withExtension: "bin", subdirectory: "owner"))
        let wire = try Data(contentsOf: url)
        for size in [1, 7, wire.count] {
            var stream = try observed()
            var events: [MobileTerminalEvent] = []
            for start in stride(from: 0, to: wire.count, by: size) {
                events += try stream.receive(wire.subdata(in: start..<min(start + size, wire.count)))
            }
            XCTAssertEqual(events.count, 1, "The remote path is metadata, not terminal output")
            guard case .control(let input, let geometry) = events.first else {
                return XCTFail("The event following a directory report must still arrive")
            }
            XCTAssertFalse(input)
            XCTAssertFalse(geometry)
            XCTAssertTrue(stream.ready)
        }
    }

    func testRustOwnerSnapshotRendersOnceAfterGeometryAcrossEveryReadBoundary() throws {
        let url = try XCTUnwrap(Bundle(for: Self.self).url(forResource: "observer", withExtension: "bin", subdirectory: "owner"))
        let wire = try Data(contentsOf: url)
        for chunkSize in [1, 3, 64, wire.count] {
            var stream = OwnerStream()
            stream.expectAttachment(1)
            var events: [MobileTerminalEvent] = []
            for start in stride(from: 0, to: wire.count, by: chunkSize) {
                events += try stream.receive(wire.subdata(in: start..<min(start + chunkSize, wire.count)))
            }
            guard case .geometry(let columns, let rows) = events.first else { return XCTFail("Geometry must precede the snapshot") }
            XCTAssertEqual(columns, 120)
            XCTAssertEqual(rows, 40)
            let snapshots = events.compactMap { event -> Data? in
                if case .snapshot(let data) = event { return data }; return nil
            }
            XCTAssertEqual(snapshots, [Data("\u{1b}[?2004h\u{1b}[2J\u{1b}[HHello from a retained Session\r\n> ".utf8)])
            let output = events.reduce(into: Data()) { result, event in
                if case .output(let data) = event { result.append(data) }
            }
            XCTAssertTrue(String(decoding: output, as: UTF8.self).hasPrefix("live\r\n"))
            XCTAssertTrue(String(decoding: output, as: UTF8.self).contains("other-nonce"))
            XCTAssertTrue(stream.ready)
            XCTAssertFalse(stream.inputControl)
            XCTAssertFalse(stream.geometryControl)
        }
    }

    func testGapsAndOversizedFramesRequireReconnect() throws {
        var stream = try observed()
        let gap = Data("\u{1b}_lf1;fixture-nonce;{\"seq\":4,\"type\":\"control\",\"held\":false,\"notice\":null}\u{1b}\\".utf8)
        XCTAssertThrowsError(try stream.receive(OwnerStream.frame(tag: 3, body: gap)))
        var oversized = OwnerStream()
        XCTAssertThrowsError(try oversized.receive(Data([3, 0, 64, 0, 1])))
    }

    func testOwnerReportsAndClearUseTheSharedStatusShapeWithoutChangingClaims() throws {
        let url = try XCTUnwrap(Bundle(for: Self.self).url(forResource: "program-status", withExtension: "bin", subdirectory: "owner"))
        let wire = try Data(contentsOf: url)
        for size in [1, 7, wire.count] {
            var stream = try observed()
            var reports: [ProgramStatusRecords] = []
            for start in stride(from: 0, to: wire.count, by: size) {
                for event in try stream.receive(wire.subdata(in: start..<min(start + size, wire.count))) {
                    if case .programStatus(let status) = event { reports.append(status) }
                }
            }
            XCTAssertEqual(reports.count, 2)
            XCTAssertEqual(reports.first?.summary?.state, .blocked)
            XCTAssertEqual(reports.first?.summary?.kind, .permission)
            XCTAssertEqual(reports.first?.summary?.msg, "Needs a decision")
            XCTAssertTrue(reports.last?.seen == true)
            XCTAssertNil(reports.last?.summary)
            XCTAssertFalse(stream.inputControl)
            XCTAssertFalse(stream.geometryControl)
        }
    }

    func testControlGrantWaitsForTheReplacementScreenAndLossRevokesInput() throws {
        var stream = try observed()
        stream.expectAttachment(2)
        let reply = Data(#"{"answers":2,"seq":2,"kind":"attached","viewer":"mobile-viewer","reconnect":"test-reconnect","nonce":"fixture-nonce","token":"test-claim","terminal":true,"grace_ms":60000,"owner":"mobile-viewer"}"#.utf8)
        XCTAssertTrue(try stream.receive(OwnerStream.frame(tag: 1, body: reply)).isEmpty)
        XCTAssertFalse(stream.ready)
        let resize = Data("\u{1b}_lf1;fixture-nonce;{\"seq\":3,\"type\":\"resize\",\"rows\":20,\"cols\":44}\u{1b}\\".utf8)
        let events = try stream.receive(OwnerStream.frame(tag: 3, body: resize))
        guard case .control(let input, let geometry) = events.last else { return XCTFail("Expected the new grant") }
        XCTAssertTrue(input)
        XCTAssertTrue(geometry)
        XCTAssertTrue(stream.ready)
        let lost = Data("\u{1b}_lf1;fixture-nonce;{\"seq\":4,\"type\":\"control\",\"held\":false,\"notice\":null}\u{1b}\\".utf8)
        _ = try stream.receive(OwnerStream.frame(tag: 3, body: lost))
        XCTAssertFalse(stream.inputControl)
        XCTAssertTrue(stream.geometryControl, "Input and geometry are distinct claims")
    }

    private func observed() throws -> OwnerStream {
        let url = try XCTUnwrap(Bundle(for: Self.self).url(forResource: "observer", withExtension: "bin", subdirectory: "owner"))
        var stream = OwnerStream()
        stream.expectAttachment(1)
        _ = try stream.receive(Data(contentsOf: url))
        return stream
    }
}
