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

    static func resolveToken(
        connection: ServerConnection,
        tokenProvider: (@Sendable () -> String?)?
    ) -> String? {
        if let token = tokenProvider?(), !token.isEmpty {
            return token
        }
        if let token = connection.staticToken, !token.isEmpty {
            return token
        }
        guard connection.isLocal else {
            return nil
        }
        return FileTokenProvider().readToken()
    }

    public static var defaultTokenURL: URL {
        FileManager.default.homeDirectoryForCurrentUser
            .appendingPathComponent(".lf", isDirectory: true)
            .appendingPathComponent("session-token", isDirectory: false)
    }
}
