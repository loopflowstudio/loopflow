import Foundation

public struct FileTokenProvider: TokenProvider {
    private let tokenURL: URL

    public init(tokenURL: URL = Self.defaultTokenURL) {
        self.tokenURL = tokenURL
    }

    public func token() async throws -> String {
        guard let token = readToken() else {
            throw AuthError.notAuthenticated
        }
        return token
    }

    public func readToken() -> String? {
        guard let value = try? String(contentsOf: tokenURL, encoding: .utf8) else {
            return nil
        }

        let token = value.trimmingCharacters(in: .whitespacesAndNewlines)
        return token.isEmpty ? nil : token
    }

    public static var defaultTokenURL: URL {
        FileManager.default.homeDirectoryForCurrentUser
            .appendingPathComponent(".lf", isDirectory: true)
            .appendingPathComponent("session-token", isDirectory: false)
    }
}
