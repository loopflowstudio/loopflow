import Foundation
import XCTest

@testable import LoopflowCore

final class FileTokenProviderTests: XCTestCase {
    func testReadTokenReturnsTrimmedValue() {
        let tempDir = FileManager.default.temporaryDirectory
            .appendingPathComponent(UUID().uuidString, isDirectory: true)
        defer {
            try? FileManager.default.removeItem(at: tempDir)
        }

        try? FileManager.default.createDirectory(at: tempDir, withIntermediateDirectories: true)
        let tokenURL = tempDir.appendingPathComponent("session-token")
        try? " session-token\n".write(to: tokenURL, atomically: true, encoding: .utf8)

        let provider = FileTokenProvider(tokenURL: tokenURL)
        XCTAssertEqual(provider.readToken(), "session-token")
    }

    func testReadTokenReturnsNilWhenFileMissing() {
        let tokenURL = FileManager.default.temporaryDirectory
            .appendingPathComponent(UUID().uuidString)
            .appendingPathComponent("missing-token")

        let provider = FileTokenProvider(tokenURL: tokenURL)
        XCTAssertNil(provider.readToken())
    }

    func testTokenAsyncReturnsValue() async throws {
        let tempDir = FileManager.default.temporaryDirectory
            .appendingPathComponent(UUID().uuidString, isDirectory: true)
        defer {
            try? FileManager.default.removeItem(at: tempDir)
        }

        try FileManager.default.createDirectory(at: tempDir, withIntermediateDirectories: true)
        let tokenURL = tempDir.appendingPathComponent("session-token")
        try "session-token".write(to: tokenURL, atomically: true, encoding: .utf8)

        let provider = FileTokenProvider(tokenURL: tokenURL)
        let token = try await provider.token()
        XCTAssertEqual(token, "session-token")
    }
}
