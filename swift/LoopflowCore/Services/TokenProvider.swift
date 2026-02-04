import Foundation

public protocol TokenProvider: Sendable {
    func token() async throws -> String
}

public struct NoAuthProvider: TokenProvider {
    public init() {}
    public func token() async throws -> String { "" }
}

public final class KeychainTokenProvider: TokenProvider, @unchecked Sendable {
    private let authService: AuthService

    public init(authService: AuthService = AuthService()) {
        self.authService = authService
    }

    public func token() async throws -> String {
        guard let token = authService.currentToken() else {
            throw AuthError.notAuthenticated
        }

        if let exp = authService.tokenExpiresAt(), Date() >= exp {
            throw AuthError.tokenExpired
        }

        return token
    }
}
