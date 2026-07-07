import Foundation

public protocol TokenProvider: Sendable {
    func token() async throws -> String
}

public struct NoAuthProvider: TokenProvider {
    public init() {}
    public func token() async throws -> String { "" }
}
