import Foundation
import XCTest

@testable import LoopflowCore

final class AuthServiceTests: XCTestCase {
    func testDecodeExpiryReadsExpClaim() {
        let exp: TimeInterval = 1_893_456_000
        let token = makeToken(payload: ["exp": exp])
        guard let date = AuthService.decodeExpiry(token) else {
            XCTFail("Expected expiry date")
            return
        }
        XCTAssertEqual(date.timeIntervalSince1970, exp, accuracy: 0.5)
    }

    func testDecodeExpiryReturnsNilForInvalidToken() {
        XCTAssertNil(AuthService.decodeExpiry("not.a.jwt"))
    }

    private func makeToken(payload: [String: Any]) -> String {
        let header: [String: Any] = ["alg": "none", "typ": "JWT"]
        let headerData = try? JSONSerialization.data(withJSONObject: header)
        let payloadData = try? JSONSerialization.data(withJSONObject: payload)
        let headerPart = base64UrlEncode(headerData ?? Data())
        let payloadPart = base64UrlEncode(payloadData ?? Data())
        return "\(headerPart).\(payloadPart).signature"
    }

    private func base64UrlEncode(_ data: Data) -> String {
        let base64 = data.base64EncodedString()
        return base64
            .replacingOccurrences(of: "+", with: "-")
            .replacingOccurrences(of: "/", with: "_")
            .replacingOccurrences(of: "=", with: "")
    }
}
