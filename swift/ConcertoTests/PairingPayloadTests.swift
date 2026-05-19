import Foundation
import Testing
@testable import LoopflowCore

@Suite("PairingPayload")
struct PairingPayloadTests {
    @Test("parses loopflow pair URLs into static-token server connections")
    func parsesPairingURL() throws {
        let url = URL(string: "loopflow://pair?host=100.64.1.2&port=2486&tls=false&token=tok%2Ben")!
        let payload = try PairingPayload(url: url)

        #expect(payload.host == "100.64.1.2")
        #expect(payload.port == 2486)
        #expect(!payload.useTLS)
        #expect(payload.token == "tok+en")
        #expect(payload.serverConnection.authMode == .staticToken)
        #expect(payload.serverConnection.wsBaseURL.absoluteString == "ws://100.64.1.2:2486/ws")
    }

    @Test("requires tls query to avoid scattered defaults")
    func requiresTLSField() {
        let url = URL(string: "loopflow://pair?host=lfd.example.com&port=443&token=token")!
        #expect(throws: PairingPayloadError.invalidField("tls")) {
            _ = try PairingPayload(url: url)
        }
    }

    @Test("refuses plaintext outside Tailscale range")
    func refusesPlaintextOutsideTailscale() {
        let url = URL(string: "loopflow://pair?host=192.168.1.2&port=2486&tls=false&token=token")!
        #expect(throws: PairingPayloadError.insecurePlaintextHost("192.168.1.2")) {
            _ = try PairingPayload(url: url)
        }
    }

    @Test("rejects duplicate query fields instead of trapping")
    func rejectsDuplicateFields() {
        let url = URL(string: "loopflow://pair?host=one&host=two&port=443&tls=true&token=token")!
        #expect(throws: PairingPayloadError.invalidField("host")) {
            _ = try PairingPayload(url: url)
        }
    }

    @Test("normalizes QR-provided certificate fingerprint")
    func normalizesFingerprint() throws {
        let fp = String(repeating: "a", count: 64)
        let grouped = fp.chunked(into: 2).joined(separator: ":")
        let url = URL(string: "loopflow://pair?host=lfd.example.com&port=443&tls=true&token=token&fp=\(grouped)")!
        let payload = try PairingPayload(url: url)

        #expect(payload.fingerprint == fp)
    }
}

private extension String {
    func chunked(into size: Int) -> [String] {
        stride(from: 0, to: count, by: size).map { offset in
            let start = index(startIndex, offsetBy: offset)
            let end = index(start, offsetBy: Swift.min(size, distance(from: start, to: endIndex)))
            return String(self[start..<end])
        }
    }
}
