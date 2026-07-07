import Foundation
import Testing
@testable import Concerto

@Suite("Credential socket file credential reader")
struct CredentialSocketServerTests {
    @Test("reads Claude credentials from ~/.claude/.credentials.json")
    func readsClaudeCredentialsFile() throws {
        let home = try makeTempHome()
        let claudeDir = home.appendingPathComponent(".claude", isDirectory: true)
        try FileManager.default.createDirectory(at: claudeDir, withIntermediateDirectories: true)
        try """
        {
          "accessToken": "claude-token",
          "email": "jack@example.com",
          "expiresAt": "1893456000000"
        }
        """.write(
            to: claudeDir.appendingPathComponent(".credentials.json"),
            atomically: true,
            encoding: .utf8
        )

        let credential = FileCredentialReader(homeDirectory: home).read(provider: .claude)

        #expect(credential?.token == "claude-token")
        #expect(credential?.login == "jack@example.com")
        #expect(credential?.expires_at == "1893456000000")
    }

    @Test("reads Codex credentials from ~/.codex/auth.json")
    func readsCodexCredentialsFile() throws {
        let home = try makeTempHome()
        let codexDir = home.appendingPathComponent(".codex", isDirectory: true)
        try FileManager.default.createDirectory(at: codexDir, withIntermediateDirectories: true)
        try """
        {
          "access_token": "codex-token",
          "expires_at": "2030-01-01T00:00:00Z"
        }
        """.write(
            to: codexDir.appendingPathComponent("auth.json"),
            atomically: true,
            encoding: .utf8
        )

        let credential = FileCredentialReader(homeDirectory: home).read(provider: .codex)

        #expect(credential?.token == "codex-token")
        #expect(credential?.login == nil)
        #expect(credential?.expires_at == "2030-01-01T00:00:00Z")
    }

    @Test("reads nested Codex access token from ChatGPT auth.json")
    func readsNestedCodexCredentialsFile() throws {
        let home = try makeTempHome()
        let codexDir = home.appendingPathComponent(".codex", isDirectory: true)
        try FileManager.default.createDirectory(at: codexDir, withIntermediateDirectories: true)
        try """
        {
          "auth_mode": "chatgpt",
          "tokens": {
            "access_token": "nested-codex-token"
          }
        }
        """.write(
            to: codexDir.appendingPathComponent("auth.json"),
            atomically: true,
            encoding: .utf8
        )

        let credential = FileCredentialReader(homeDirectory: home).read(provider: .codex)

        #expect(credential?.token == "nested-codex-token")
    }
}

private func makeTempHome() throws -> URL {
    let url = FileManager.default.temporaryDirectory
        .appendingPathComponent(UUID().uuidString, isDirectory: true)
    try FileManager.default.createDirectory(at: url, withIntermediateDirectories: true)
    return url
}
