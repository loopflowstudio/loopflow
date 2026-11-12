import Foundation
import Testing
@testable import Loopflow

/// The owner protocol from `rust/loopflow/src/transport/wire.rs`, read from Rust's fixtures.
struct OwnerStreamTests {
    @Test func heartbeatUsesOwnerProtocolWithoutTakingControlOrConsumingAttachmentReply() throws {
        var stream = try observed()
        let request = try stream.heartbeatRequest(id: 2)
        #expect(request.first == OwnerTag.request.rawValue)
        #expect(try JSONDecoder().decode(JSONValue.self, from: Data(request.dropFirst(5)))
                == .object(["id": .integer(2), "kind": .string("controlled")]))
        stream.expectAttachment(3)
        let reply = Data(#"{"answers":2,"seq":2,"kind":"controlled","held":true}"#.utf8)
        #expect(try stream.receive(OwnerStream.frame(.reply, body: reply)).isEmpty)
        #expect(stream.awaitingAttachment)
        #expect(stream.ready)
        #expect(!(stream.inputControl), "Controlled reports whether anyone holds control, not this viewer")
        #expect(!(stream.geometryControl))
        #expect(throws: Never.self) { try stream.heartbeatRequest(id: 4) }
    }

    @Test func unansweredHeartbeatRequiresReconnect() throws {
        var stream = try observed()
        _ = try stream.heartbeatRequest(id: 2)
        #expect(throws: (any Error).self) { try stream.heartbeatRequest(id: 3) }
        let unexpected = Data(#"{"answers":2,"seq":2,"kind":"accepted"}"#.utf8)
        #expect(throws: (any Error).self) { try stream.receive(OwnerStream.frame(.reply, body: unexpected)) }
    }

    @Test func remoteDirectoryAdvancesTheStreamWithoutChangingLocalState() throws {
        let wire = try loadFixtureData("owner/directory.bin")
        for size in [1, 7, wire.count] {
            var stream = try observed()
            var events: [TerminalEvent] = []
            for start in stride(from: 0, to: wire.count, by: size) {
                events += try stream.receive(wire.subdata(in: start..<min(start + size, wire.count)))
            }
            #expect(events.count == 1)
            guard case .control(let input, let geometry) = events.first else {
                Issue.record("The event following a directory report must still arrive"); return
            }
            #expect(!(input))
            #expect(!(geometry))
            #expect(stream.ready)
        }
    }

    @Test func rustOwnerSnapshotRendersOnceAfterGeometryAcrossEveryReadBoundary() throws {
        let wire = try loadFixtureData("owner/observer.bin")
        for chunkSize in [1, 3, 64, wire.count] {
            var stream = OwnerStream()
            stream.expectAttachment(1)
            var events: [TerminalEvent] = []
            for start in stride(from: 0, to: wire.count, by: chunkSize) {
                events += try stream.receive(wire.subdata(in: start..<min(start + chunkSize, wire.count)))
            }
            guard case .geometry(let columns, let rows) = events.first else { Issue.record("Geometry must precede the snapshot"); return }
            #expect(columns == 120)
            #expect(rows == 40)
            let snapshots = events.compactMap { event -> Data? in
                if case .snapshot(let data) = event { return data }; return nil
            }
            #expect(snapshots == [Data("\u{1b}[?2004h\u{1b}[2J\u{1b}[HHello from a retained Session\r\n> ".utf8)])
            let output = events.reduce(into: Data()) { result, event in
                if case .output(let data) = event { result.append(data) }
            }
            #expect(String(decoding: output, as: UTF8.self).hasPrefix("live\r\n"))
            #expect(String(decoding: output, as: UTF8.self).contains("other-nonce"))
            #expect(stream.ready)
            #expect(!(stream.inputControl))
            #expect(!(stream.geometryControl))
        }
    }

    @Test func gapsAndOversizedFramesRequireReconnect() throws {
        var stream = try observed()
        let gap = Data("\u{1b}_lf1;fixture-nonce;{\"seq\":4,\"type\":\"control\",\"held\":false,\"notice\":null}\u{1b}\\".utf8)
        #expect(throws: (any Error).self) { try stream.receive(OwnerStream.frame(.out, body: gap)) }
        var oversized = OwnerStream()
        #expect(throws: (any Error).self) { try oversized.receive(Data([3, 0, 64, 0, 1])) }
    }

    @Test func ownerReportsAndClearUseTheSharedStatusShapeWithoutChangingClaims() throws {
        let wire = try loadFixtureData("owner/program-status.bin")
        for size in [1, 7, wire.count] {
            var stream = try observed()
            var reports: [ProgramStatusRecords] = []
            for start in stride(from: 0, to: wire.count, by: size) {
                for event in try stream.receive(wire.subdata(in: start..<min(start + size, wire.count))) {
                    if case .programStatus(let status) = event { reports.append(status) }
                }
            }
            #expect(reports.count == 2)
            #expect(reports.first?.summary?.state == .blocked)
            #expect(reports.first?.summary?.kind == .permission)
            #expect(reports.first?.summary?.msg == "Needs a decision")
            #expect(reports.last?.seen == true)
            #expect(reports.last?.summary == nil)
            #expect(!(stream.inputControl))
            #expect(!(stream.geometryControl))
        }
    }

    @Test func controlGrantWaitsForTheReplacementScreenAndLossRevokesInput() throws {
        var stream = try observed()
        stream.expectAttachment(2)
        let reply = Data(#"{"answers":2,"seq":2,"kind":"attached","viewer":"mobile-viewer","nonce":"fixture-nonce","token":"test-claim","terminal":true,"owner":"mobile-viewer"}"#.utf8)
        #expect(try stream.receive(OwnerStream.frame(.reply, body: reply)).isEmpty)
        #expect(!(stream.ready))
        let resize = Data("\u{1b}_lf1;fixture-nonce;{\"seq\":3,\"type\":\"resize\",\"rows\":20,\"cols\":44}\u{1b}\\".utf8)
        let events = try stream.receive(OwnerStream.frame(.out, body: resize))
        guard case .control(let input, let geometry) = events.last else { Issue.record("Expected the new grant"); return }
        #expect(input)
        #expect(geometry)
        #expect(stream.ready)
        let lost = Data("\u{1b}_lf1;fixture-nonce;{\"seq\":4,\"type\":\"control\",\"held\":false,\"notice\":null}\u{1b}\\".utf8)
        _ = try stream.receive(OwnerStream.frame(.out, body: lost))
        #expect(!(stream.inputControl))
        #expect(stream.geometryControl, "Input and geometry are distinct claims")
    }

    private func observed() throws -> OwnerStream {
        var stream = OwnerStream()
        stream.expectAttachment(1)
        _ = try stream.receive(loadFixtureData("owner/observer.bin"))
        return stream
    }
}
